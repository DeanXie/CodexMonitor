#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { stat, statfs } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  STATUS,
  acquireOperationLock,
  applyCleanupPlan,
  buildStorageReport,
  comparisonPath,
  ensureAgentTarget,
  evaluateBudget,
  failClosedOnIncompleteScans,
  managedBudgetInputs,
  loadPolicy,
  planAgentCleanup,
  planCloseout,
  resolveAgentTarget,
  resolveBuildRoot,
  scanDirectoryOnce,
  releaseOperationLock,
} from "./storage-governance-core.mjs";

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function parseArguments(argv) {
  const command = argv[0] || "report";
  const separator = argv.indexOf("--");
  const allArgs = argv.slice(1);
  const rawForwarded = separator === -1 ? [] : argv.slice(separator + 1);
  const flags = new Set(allArgs.filter((value) => value.startsWith("--") && !value.includes("=")));
  const value = (name) => {
    const index = allArgs.indexOf(name);
    return index >= 0 ? allArgs[index + 1] : undefined;
  };
  const governanceFlags = new Set([
    "--accepted",
    "--allow-low-space",
    "--apply",
    "--ensure-only",
    "--json",
    "--repair-acl",
  ]);
  const forwarded = [];
  for (let index = 0; index < rawForwarded.length; index += 1) {
    const item = rawForwarded[index];
    if (governanceFlags.has(item)) continue;
    if (item === "--simulate-free-bytes") {
      index += 1;
      continue;
    }
    forwarded.push(item);
  }
  return {
    accepted: flags.has("--accepted"),
    allowLowSpace: flags.has("--allow-low-space") || process.env.CODEXMONITOR_ALLOW_LOW_SPACE === "1",
    apply: flags.has("--apply"),
    command,
    ensureOnly: flags.has("--ensure-only"),
    forwarded,
    json: flags.has("--json"),
    repairAcl: flags.has("--repair-acl"),
    simulateFreeBytes: value("--simulate-free-bytes"),
  };
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd,
    encoding: "utf8",
    env: options.env || process.env,
    stdio: options.stdio || ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });
  if (result.error) throw result.error;
  return result;
}

function git(cwd, args, options = {}) {
  const result = run("git", args, { cwd });
  if (result.status !== 0 && !options.allowFailure) {
    throw new Error(result.stderr.trim() || `git ${args.join(" ")} failed with ${result.status}`);
  }
  return { status: result.status, stderr: result.stderr.trim(), stdout: result.stdout.trim() };
}

function resolveGitPath(cwd, value) {
  return path.resolve(cwd, value);
}

function resolveTopology(cwd = process.cwd()) {
  const worktreePath = path.resolve(git(cwd, ["rev-parse", "--show-toplevel"]).stdout);
  const gitDir = resolveGitPath(cwd, git(cwd, ["rev-parse", "--git-dir"]).stdout);
  const commonDir = resolveGitPath(cwd, git(cwd, ["rev-parse", "--git-common-dir"]).stdout);
  const isLinkedWorktree = comparisonPath(gitDir) !== comparisonPath(commonDir);
  const mainRoot = path.basename(commonDir).toLowerCase() === ".git" ? path.dirname(commonDir) : worktreePath;
  return { commonDir, gitDir, isLinkedWorktree, mainRoot, worktreePath };
}

function listRegisteredWorktrees(mainRoot) {
  return git(mainRoot, ["worktree", "list", "--porcelain"]).stdout
    .split(/\r?\n/)
    .filter((line) => line.startsWith("worktree "))
    .map((line) => path.resolve(line.slice("worktree ".length)));
}

function isGitClean(worktreePath) {
  const result = git(worktreePath, ["-c", `safe.directory=${worktreePath}`, "status", "--porcelain"], {
    allowFailure: true,
  });
  return result.status === 0 && result.stdout === "";
}

function isMergedIntoMain(worktreePath) {
  const result = git(
    worktreePath,
    ["-c", `safe.directory=${worktreePath}`, "merge-base", "--is-ancestor", "HEAD", "main"],
    { allowFailure: true },
  );
  return result.status === 0;
}

async function nearestExistingPath(value) {
  let current = path.resolve(value);
  while (true) {
    try {
      await stat(current);
      return current;
    } catch (error) {
      if (error?.code !== "ENOENT") throw error;
      const parent = path.dirname(current);
      if (parent === current) throw error;
      current = parent;
    }
  }
}

async function freeBytesFor(value) {
  const filesystem = await statfs(await nearestExistingPath(value));
  return Number(filesystem.bavail) * Number(filesystem.bsize);
}

async function scanIfPresent(value, options = {}) {
  try {
    const info = await stat(value);
    if (!info.isDirectory()) return null;
    return await scanDirectoryOnce(value, options);
  } catch (error) {
    if (error?.code === "ENOENT") return null;
    throw error;
  }
}

async function prepareContext(options = {}) {
  const topology = resolveTopology();
  const policy = await loadPolicy();
  const targetRoot = resolveBuildRoot({ mainRoot: topology.mainRoot });
  if (!topology.isLinkedWorktree) {
    return {
      environment: {},
      isLinkedWorktree: false,
      policy,
      targetPath: path.join(topology.worktreePath, "src-tauri", "target"),
      targetRoot,
      topology,
    };
  }
  const ensured = options.ensure === false
    ? {
        manifest: null,
        targetPath: resolveAgentTarget({
          buildRoot: targetRoot,
          commonDir: topology.commonDir,
          worktreePath: topology.worktreePath,
        }),
      }
    : await ensureAgentTarget({
        buildRoot: targetRoot,
        commonDir: topology.commonDir,
        worktreePath: topology.worktreePath,
      });
  const context = {
    environment: { CARGO_INCREMENTAL: "0", CARGO_TARGET_DIR: ensured.targetPath },
    isLinkedWorktree: true,
    manifest: ensured.manifest,
    policy,
    targetPath: ensured.targetPath,
    targetRoot,
    topology,
  };
  if (options.guard !== false) context.guard = await evaluateCurrentBudget(context, options);
  return context;
}

async function evaluateCurrentBudget(context, options = {}) {
  if (!context.isLinkedWorktree) {
    return { blocked: false, overrideApplied: false, reasons: [], warnings: [] };
  }
  const registeredWorktrees = new Set(listRegisteredWorktrees(context.topology.mainRoot));
  const cleanupPlan = await planAgentCleanup({
    apply: false,
    buildRoot: context.targetRoot,
    commonDir: context.topology.commonDir,
    deadlineMs: performance.now() + context.policy.report.totalTimeoutMs,
    policy: context.policy,
    registeredWorktrees,
  });
  let freeBytes;
  if (options.simulateFreeBytes !== undefined) {
    if (process.env.CODEXMONITOR_STORAGE_TEST_MODE !== "1") {
      throw new Error("--simulate-free-bytes requires CODEXMONITOR_STORAGE_TEST_MODE=1");
    }
    freeBytes = Number(options.simulateFreeBytes);
    if (!Number.isFinite(freeBytes) || freeBytes < 0) throw new Error("Invalid simulated free-space value");
  } else {
    freeBytes = await freeBytesFor(context.targetRoot);
  }
  const inputs = managedBudgetInputs(cleanupPlan.entries, context.targetPath, freeBytes);
  const budget = failClosedOnIncompleteScans(
    evaluateBudget(
      inputs,
      context.policy,
      { override: options.allowLowSpace },
    ),
    cleanupPlan.entries,
    options.allowLowSpace,
  );
  budget.observed.currentTargetBytes = inputs.currentTargetBytes;
  return budget;
}

function publicContext(context) {
  return {
    budget: context.guard,
    environment: context.environment,
    isLinkedWorktree: context.isLinkedWorktree,
    targetPath: context.targetPath,
    targetRoot: context.targetRoot,
    worktreePath: context.topology.worktreePath,
  };
}

async function collectReport() {
  const startedAt = performance.now();
  const topology = resolveTopology();
  const policy = await loadPolicy();
  const deadlineMs = startedAt + policy.report.totalTimeoutMs;
  const targetRoot = resolveBuildRoot({ mainRoot: topology.mainRoot });
  const registered = listRegisteredWorktrees(topology.mainRoot);
  const registeredSet = new Set(registered);
  const worktreeStates = new Map(
    registered.map((worktreePath) => [
      worktreePath,
      {
        gitClean: isGitClean(worktreePath),
        mergedIntoMain: isMergedIntoMain(worktreePath),
      },
    ]),
  );
  const external = await planAgentCleanup({
    apply: false,
    buildRoot: targetRoot,
    commonDir: topology.commonDir,
    deadlineMs,
    policy,
    registeredWorktrees: registeredSet,
    worktreeStates,
  });
  const entries = [...external.entries];

  const mainTarget = path.join(topology.mainRoot, "src-tauri", "target");
  const releaseTarget = path.join(mainTarget, "release");
  const mainScan = await scanIfPresent(mainTarget, {
    maxEntries: policy.report.maxEntriesPerScan,
    skipPaths: [releaseTarget],
    timeoutMs: Math.max(0, Math.min(policy.report.legacyScanTimeoutMs, deadlineMs - performance.now())),
  });
  if (mainScan) entries.push({ ...mainScan, path: mainTarget, status: STATUS.MAIN });
  const releaseScan = await scanIfPresent(releaseTarget, {
    maxEntries: policy.report.maxEntriesPerScan,
    timeoutMs: Math.max(0, Math.min(policy.report.legacyScanTimeoutMs, deadlineMs - performance.now())),
  });
  if (releaseScan) entries.push({ ...releaseScan, path: releaseTarget, status: STATUS.RELEASE });

  for (const worktreePath of registered) {
    if (comparisonPath(worktreePath) === comparisonPath(topology.mainRoot)) continue;
    const legacyTarget = path.join(worktreePath, "src-tauri", "target");
    const legacyScan = await scanIfPresent(legacyTarget, {
      maxEntries: policy.report.maxEntriesPerScan,
      timeoutMs: Math.max(0, Math.min(policy.report.legacyScanTimeoutMs, deadlineMs - performance.now())),
    });
    if (legacyScan) entries.push({ ...legacyScan, path: legacyTarget, status: STATUS.LEGACY_IN_TREE });
  }

  const auxiliary = [];
  for (const [kind, value] of [
    ["node_modules", path.join(topology.mainRoot, "node_modules")],
    ["dist", path.join(topology.mainRoot, "dist")],
  ]) {
    const scan = await scanIfPresent(value, {
      maxEntries: policy.report.maxEntriesPerScan,
      timeoutMs: Math.max(0, Math.min(policy.report.legacyScanTimeoutMs, deadlineMs - performance.now())),
    });
    if (scan) auxiliary.push({ ...scan, kind, path: value });
  }
  const freeBytes = await freeBytesFor(targetRoot);
  return {
    ...(await buildStorageReport({ entries, freeBytes, policy, startedAt, targetRoot })),
    auxiliary,
    policy,
  };
}

function formatBytes(bytes = 0) {
  return `${(bytes / 1024 ** 3).toFixed(2)} GiB`;
}

function print(payload, json) {
  if (json) {
    process.stdout.write(`${JSON.stringify(payload)}\n`);
    return;
  }
  if (payload.entries) {
    console.log(`Storage target root: ${payload.targetRoot}`);
    for (const entry of payload.entries) {
      const scan = entry.truncated ? "truncated" : `${(entry.scanElapsedMs ?? entry.elapsedMs ?? 0).toFixed(0)}ms`;
      console.log(`${entry.status.padEnd(18)} ${formatBytes(entry.bytes)}  ${scan.padEnd(10)} ${entry.targetPath || entry.path}`);
    }
    for (const entry of payload.auxiliary || []) {
      console.log(`${entry.kind.padEnd(18)} ${formatBytes(entry.bytes)}  ${entry.path}`);
    }
    console.log(`Free space: ${formatBytes(payload.freeBytes)}`);
    console.log(`Agent targets: ${formatBytes(payload.totalAgentBytes)}`);
    console.log(`Scan elapsed: ${payload.scanElapsedMs.toFixed(0)} ms`);
    for (const warning of payload.budget.warnings) {
      console.log(`WARNING ${warning.code}: ${warning.actualGiB.toFixed(2)} GiB (threshold ${warning.thresholdGiB} GiB)`);
    }
    for (const reason of payload.budget.reasons) {
      console.log(`CRITICAL ${reason.code}: ${reason.actualGiB.toFixed(2)} GiB (threshold ${reason.thresholdGiB} GiB)`);
    }
    return;
  }
  console.log(JSON.stringify(payload, null, 2));
}

function childEnvironment(context) {
  return context.isLinkedWorktree ? { ...process.env, ...context.environment } : process.env;
}

function formatGuardFinding(entry) {
  if (entry.code === "SCAN_INCOMPLETE") return `${entry.code}:${entry.targetPaths.join(",")}`;
  return `${entry.code}:${entry.actualGiB.toFixed(2)}GiB/${entry.thresholdGiB}GiB`;
}

function logGuardMeasurements(context, label, findings) {
  console.error(
    `${label}: override=${context.guard.overrideApplied}; free=${formatBytes(context.guard.observed.freeBytes)}; ` +
      `largestTarget=${formatBytes(context.guard.observed.targetBytes)}; ` +
      `currentTarget=${formatBytes(context.guard.observed.currentTargetBytes)}; ` +
      `agents=${formatBytes(context.guard.observed.totalBytes)}; targetRoot=${context.targetRoot}; ` +
      `thresholds=${findings.map(formatGuardFinding).join(",")}`,
  );
}

async function acquireBuildLease(context, kind, forwarded) {
  if (!context.isLinkedWorktree) return null;
  return await acquireOperationLock({
    buildRoot: context.targetRoot,
    command: [kind, ...forwarded],
    kind: "build",
    staleAfterMs: context.policy.operationLockStaleMinutes * 60 * 1000,
    targetPath: context.targetPath,
  });
}

async function executeHeavyCommand(kind, forwarded, context) {
  for (const warning of context.guard?.warnings || []) {
    logGuardMeasurements(context, `Storage guard warning ${warning.code}`, [warning]);
  }
  if (context.guard?.blocked) {
    logGuardMeasurements(context, "Storage guard blocked ephemeral build", context.guard.reasons);
    console.error("Use --allow-low-space to override explicitly.");
    return 2;
  }
  if (context.guard?.overrideApplied) {
    logGuardMeasurements(context, "Storage guard override applied", context.guard.reasons);
  }
  const leasePath = await acquireBuildLease(context, kind, forwarded);
  try {
    if (context.isLinkedWorktree) {
      await ensureAgentTarget({
        buildRoot: context.targetRoot,
        commonDir: context.topology.commonDir,
        operationLock: leasePath,
        worktreePath: context.topology.worktreePath,
      });
    }
    if (kind === "cargo") {
      const result = spawnSync("cargo", forwarded.length > 0 ? forwarded : ["check", "--all-targets"], {
        cwd: path.join(context.topology.worktreePath, "src-tauri"),
        env: childEnvironment(context),
        stdio: "inherit",
        windowsHide: true,
      });
      if (result.error) throw result.error;
      return result.status ?? 1;
    }
    const tauriCli = path.join(projectRoot, "node_modules", "@tauri-apps", "cli", "tauri.js");
    const result = spawnSync(process.execPath, [tauriCli, ...forwarded], {
      cwd: context.topology.worktreePath,
      env: childEnvironment(context),
      stdio: "inherit",
      windowsHide: true,
    });
    if (result.error) throw result.error;
    return result.status ?? 1;
  } finally {
    await releaseOperationLock(leasePath);
  }
}

async function repairAclCandidate(candidate) {
  const who = run("whoami", [], { cwd: projectRoot });
  if (who.status !== 0) throw new Error("Unable to resolve current Windows identity for ACL repair");
  const identity = who.stdout.trim();
  const result = run("icacls", [candidate, "/grant:r", `${identity}:(OI)(CI)F`, "/T", "/C", "/Q"], {
    cwd: projectRoot,
  });
  if (result.status !== 0) throw new Error(`Scoped ACL repair failed for ${candidate}: ${result.stderr}`);
  console.error(`Scoped ACL repair applied to verified target: ${candidate}`);
}

async function main(argv = process.argv.slice(2)) {
  const options = parseArguments(argv);
  if (["prepare", "guard", "cargo", "tauri"].includes(options.command)) {
    const context = await prepareContext({
      ...options,
      ensure: !["cargo", "tauri"].includes(options.command),
      guard: !(options.command === "prepare" && options.ensureOnly),
    });
    if (options.command === "prepare" || options.command === "guard") {
      const payload = { ...publicContext(context), blocked: Boolean(context.guard?.blocked) };
      print(payload, options.json);
      return context.guard?.blocked ? 2 : 0;
    }
    return await executeHeavyCommand(options.command, options.forwarded, context);
  }

  if (options.command === "report") {
    print(await collectReport(), options.json);
    return 0;
  }

  if (options.command === "closeout") {
    const topology = resolveTopology();
    const policy = await loadPolicy();
    const buildRoot = resolveBuildRoot({ mainRoot: topology.mainRoot });
    const plan = await planCloseout({
      accepted: options.accepted,
      apply: options.apply,
      buildRoot,
      commonDir: topology.commonDir,
      gitClean: isGitClean(topology.worktreePath),
      isLinkedWorktree: topology.isLinkedWorktree,
      mergedIntoMain: isMergedIntoMain(topology.worktreePath),
      policy,
      worktreePath: topology.worktreePath,
    });
    if (!options.apply) {
      print(plan, options.json);
      return 0;
    }
    const result = await applyCleanupPlan(plan, {
      repairAcl: options.repairAcl,
      repairAclCandidate,
      staleAfterMs: policy.operationLockStaleMinutes * 60 * 1000,
    });
    print({ ...plan, ...result }, options.json);
    return 0;
  }

  if (options.command === "clean-agents") {
    const topology = resolveTopology();
    const policy = await loadPolicy();
    const buildRoot = resolveBuildRoot({ mainRoot: topology.mainRoot });
    const plan = await planAgentCleanup({
      apply: options.apply,
      buildRoot,
      commonDir: topology.commonDir,
      policy,
      registeredWorktrees: new Set(listRegisteredWorktrees(topology.mainRoot)),
    });
    const deleted = [];
    for (const entry of plan.entries.filter((candidate) => candidate.selected)) {
      deleted.push(
        await applyCleanupPlan(
          {
            apply: true,
            buildRoot,
            bytes: entry.bytes,
            commonDir: topology.commonDir,
            eligible: true,
            reasons: [],
            targetPath: entry.targetPath,
            worktreePath: entry.manifest.worktreePath,
          },
          {
            repairAcl: options.repairAcl,
            repairAclCandidate,
            staleAfterMs: policy.operationLockStaleMinutes * 60 * 1000,
          },
        ),
      );
    }
    print({ ...plan, deleted }, options.json);
    return 0;
  }

  throw new Error(`Unknown storage governance command: ${options.command}`);
}

main()
  .then((status) => {
    process.exitCode = status;
  })
  .catch((error) => {
    console.error(`Storage governance failed: ${error.message}`);
    process.exitCode = 1;
  });
