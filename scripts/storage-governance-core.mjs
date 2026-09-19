import crypto from "node:crypto";
import { lstat, mkdir, opendir, readFile, realpath, rename, rm, stat, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const STATUS = Object.freeze({
  ACTIVE: "ACTIVE",
  CLOSEOUT_ELIGIBLE: "CLOSEOUT-ELIGIBLE",
  ORPHAN: "ORPHAN",
  LEGACY_IN_TREE: "LEGACY-IN-TREE",
  MAIN: "MAIN",
  RELEASE: "RELEASE",
});

export const TARGET_MANIFEST = ".codexmonitor-target.json";
export const BUILD_LEASE = ".codexmonitor-operation-lock";
const LOCK_METADATA = "owner.json";
const GIB = 1024 ** 3;
const DAY_MS = 24 * 60 * 60 * 1000;

function projectRootFromModule() {
  return path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
}

export async function loadPolicy(policyPath = path.join(projectRootFromModule(), "config", "storage-governance.json")) {
  const policy = JSON.parse(await readFile(policyPath, "utf8"));
  if (policy.schemaVersion !== 1) throw new Error(`Unsupported storage policy schema: ${policy.schemaVersion}`);
  return policy;
}

function normalizedAbsolute(value) {
  return path.resolve(value).replace(/[\\/]+$/, "");
}

export function comparisonPath(value, platform = process.platform) {
  const normalized = normalizedAbsolute(value).replaceAll("\\", "/");
  return platform === "win32" ? normalized.toLowerCase() : normalized;
}

function sanitizeSegment(value) {
  const result = value
    .normalize("NFKD")
    .replace(/[^a-zA-Z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 48);
  return result || "worktree";
}

export function resolveBuildRoot({ platform = process.platform, env = process.env, homeDir = os.homedir(), mainRoot }) {
  if (env.CODEXMONITOR_CARGO_TARGET_ROOT?.trim()) {
    return path.resolve(env.CODEXMONITOR_CARGO_TARGET_ROOT.trim());
  }
  if (platform === "win32") {
    return path.resolve(path.dirname(mainRoot), "_cargo-target", path.basename(mainRoot));
  }
  if (platform === "darwin") {
    return path.resolve(homeDir, "Library", "Caches", "CodexMonitor", "cargo-target");
  }
  const cacheRoot = env.XDG_CACHE_HOME?.trim() || path.join(homeDir, ".cache");
  return path.resolve(cacheRoot, "codexmonitor", "cargo-target");
}

export function resolveAgentTarget({ buildRoot, commonDir, worktreePath }) {
  const identity = `${comparisonPath(commonDir)}\n${comparisonPath(worktreePath)}`;
  const hash = crypto.createHash("sha256").update(identity).digest("hex").slice(0, 12);
  const name = sanitizeSegment(path.basename(normalizedAbsolute(worktreePath)));
  return path.resolve(buildRoot, "agents", `${name}-${hash}`);
}

function thresholdEntry(code, actualGiB, thresholdGiB) {
  return { code, actualGiB, thresholdGiB };
}

export function evaluateBudget({ totalBytes, targetBytes, freeBytes }, policy, options = {}) {
  const thresholds = policy.thresholds;
  const totalGiB = totalBytes / GIB;
  const targetGiB = targetBytes / GIB;
  const freeGiB = freeBytes / GIB;
  const warnings = [];
  const reasons = [];

  if (totalGiB > thresholds.totalWarningGiB)
    warnings.push(thresholdEntry("TOTAL_WARNING", totalGiB, thresholds.totalWarningGiB));
  if (targetGiB > thresholds.targetWarningGiB)
    warnings.push(thresholdEntry("TARGET_WARNING", targetGiB, thresholds.targetWarningGiB));
  if (freeGiB < thresholds.freeWarningGiB)
    warnings.push(thresholdEntry("FREE_WARNING", freeGiB, thresholds.freeWarningGiB));
  if (totalGiB > thresholds.totalCriticalGiB)
    reasons.push(thresholdEntry("TOTAL_CRITICAL", totalGiB, thresholds.totalCriticalGiB));
  if (targetGiB > thresholds.targetCriticalGiB)
    reasons.push(thresholdEntry("TARGET_CRITICAL", targetGiB, thresholds.targetCriticalGiB));
  if (freeGiB < thresholds.freeBlockGiB)
    reasons.push(thresholdEntry("FREE_BLOCK", freeGiB, thresholds.freeBlockGiB));

  const overrideApplied = reasons.length > 0 && options.override === true;
  return {
    blocked: reasons.length > 0 && !overrideApplied,
    observed: { freeBytes, targetBytes, totalBytes },
    overrideApplied,
    reasons,
    warnings,
  };
}

export function failClosedOnIncompleteScans(budget, entries, override = false) {
  const incomplete = entries.filter((entry) => entry.truncated || entry.error);
  if (incomplete.length === 0) return budget;
  const reasons = [
    ...budget.reasons,
    {
      actualGiB: 0,
      code: "SCAN_INCOMPLETE",
      targetPaths: incomplete.map((entry) => entry.targetPath),
      thresholdGiB: 0,
    },
  ];
  const overrideApplied = budget.overrideApplied || override;
  return { ...budget, blocked: !overrideApplied, overrideApplied, reasons };
}

export function managedBudgetInputs(entries, currentTargetPath, freeBytes) {
  const sizes = entries.map((entry) => (Number.isFinite(entry.bytes) ? entry.bytes : 0));
  const current = entries.find(
    (entry) => comparisonPath(entry.targetPath) === comparisonPath(currentTargetPath),
  );
  return {
    currentTargetBytes: Number.isFinite(current?.bytes) ? current.bytes : 0,
    freeBytes,
    targetBytes: sizes.reduce((largest, bytes) => Math.max(largest, bytes), 0),
    totalBytes: sizes.reduce((total, bytes) => total + bytes, 0),
  };
}

export function classifyManagedTarget({
  activeBuild = false,
  gitClean = false,
  manifestValid = true,
  mergedIntoMain = false,
  registered,
  targetKind,
}) {
  if (targetKind === STATUS.MAIN || targetKind === STATUS.RELEASE) {
    return { cleanupEligible: false, status: targetKind, targetClass: targetKind };
  }
  if (!registered) {
    return { cleanupEligible: false, status: STATUS.ORPHAN, targetClass: STATUS.ORPHAN };
  }
  const closeoutEligible = manifestValid && gitClean && mergedIntoMain && !activeBuild;
  const targetClass = closeoutEligible ? STATUS.CLOSEOUT_ELIGIBLE : STATUS.ACTIVE;
  return { cleanupEligible: closeoutEligible, status: targetClass, targetClass };
}

export function classifyExternalTarget({
  activeBuild = false,
  manifest,
  registeredWorktrees,
  worktreeStates = new Map(),
  now = Date.now(),
  policy,
}) {
  const key = comparisonPath(manifest.worktreePath);
  const registered = new Set([...registeredWorktrees].map((value) => comparisonPath(value)));
  const states = new Map(
    [...worktreeStates].map(([worktreePath, state]) => [comparisonPath(worktreePath), state]),
  );
  if (registered.has(key)) {
    return classifyManagedTarget({
      activeBuild,
      gitClean: states.get(key)?.gitClean,
      mergedIntoMain: states.get(key)?.mergedIntoMain,
      registered: true,
    });
  }
  const lastUsedAt = Date.parse(manifest.lastUsedAt || manifest.createdAt || 0);
  const ageMs = Number.isFinite(lastUsedAt) ? Math.max(0, now - lastUsedAt) : 0;
  return {
    ageDays: ageMs / DAY_MS,
    cleanupEligible: ageMs >= policy.orphanMinimumAgeDays * DAY_MS,
    status: STATUS.ORPHAN,
    targetClass: STATUS.ORPHAN,
  };
}

export async function scanDirectoryOnce(root, options = {}) {
  const startedAt = performance.now();
  const deadline = startedAt + (options.timeoutMs ?? Number.POSITIVE_INFINITY);
  const maxEntries = options.maxEntries ?? Number.POSITIVE_INFINITY;
  const stack = [path.resolve(root)];
  const skipped = new Set((options.skipPaths || []).map((value) => comparisonPath(value)));
  let bytes = 0;
  let directories = 0;
  let entries = 0;
  let files = 0;
  let reparsePoints = 0;
  let truncated = false;

  while (stack.length > 0) {
    if (performance.now() > deadline || entries >= maxEntries) {
      truncated = true;
      break;
    }
    const current = stack.pop();
    let directory;
    try {
      directory = await opendir(current);
    } catch (error) {
      if (error?.code === "ENOENT") continue;
      throw error;
    }
    directories += 1;
    for await (const entry of directory) {
      entries += 1;
      const entryPath = path.join(current, entry.name);
      options.onVisit?.(entryPath);
      if (skipped.has(comparisonPath(entryPath))) continue;
      if (entry.isSymbolicLink()) {
        reparsePoints += 1;
      } else if (entry.isDirectory()) {
        stack.push(entryPath);
      } else if (entry.isFile()) {
        const info = await stat(entryPath);
        bytes += info.size;
        files += 1;
      }
      if (performance.now() > deadline || entries >= maxEntries) {
        truncated = true;
        break;
      }
    }
  }
  return { bytes, directories, elapsedMs: performance.now() - startedAt, entries, files, reparsePoints, truncated };
}

export async function writeTargetManifest(targetPath, input) {
  await mkdir(targetPath, { recursive: true });
  const manifest = {
    schemaVersion: 1,
    commonDir: normalizedAbsolute(input.commonDir),
    worktreePath: normalizedAbsolute(input.worktreePath),
    targetPath: normalizedAbsolute(input.targetPath || targetPath),
    createdAt: input.createdAt || new Date().toISOString(),
    lastUsedAt: input.lastUsedAt || new Date().toISOString(),
  };
  await writeFile(path.join(targetPath, TARGET_MANIFEST), `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
  return manifest;
}

async function ensureAgentTargetUnlocked({ buildRoot, commonDir, now = new Date().toISOString(), worktreePath }) {
  const targetPath = resolveAgentTarget({ buildRoot, commonDir, worktreePath });
  const agentsRoot = path.resolve(buildRoot, "agents");
  await mkdir(agentsRoot, { recursive: true });
  const agentsInfo = await lstat(agentsRoot);
  if (!agentsInfo.isDirectory() || agentsInfo.isSymbolicLink()) {
    throw new Error("Managed agents root is not a safe directory");
  }
  let existing = null;
  try {
    existing = await readTargetManifest(targetPath);
  } catch (error) {
    if (error?.code !== "ENOENT") throw error;
    let targetInfo = null;
    try {
      targetInfo = await lstat(targetPath);
    } catch (targetError) {
      if (targetError?.code !== "ENOENT") throw targetError;
    }
    if (targetInfo) {
      if (!targetInfo.isDirectory() || targetInfo.isSymbolicLink()) {
        throw new Error("Refusing to initialize a managed target over a non-directory or reparse point");
      }
      const directory = await opendir(targetPath);
      let firstEntry = null;
      try {
        firstEntry = await directory.read();
      } finally {
        await directory.close();
      }
      if (firstEntry) throw new Error("Refusing to certify a non-empty target without a managed manifest");
    } else {
      await mkdir(targetPath);
    }
  }
  if (existing) {
    const validated = await validateManagedTargetIdentity({
      buildRoot,
      candidate: targetPath,
      currentCommonDir: commonDir,
      currentWorktreePath: worktreePath,
    });
    existing = validated.manifest;
  }
  const manifest = await writeTargetManifest(targetPath, {
    commonDir,
    createdAt: existing?.createdAt || now,
    lastUsedAt: now,
    targetPath,
    worktreePath,
  });
  return { manifest, targetPath };
}

export async function ensureAgentTarget({ buildRoot, commonDir, now, operationLock, worktreePath }) {
  const targetPath = resolveAgentTarget({ buildRoot, commonDir, worktreePath });
  if (operationLock && comparisonPath(operationLock.targetPath) !== comparisonPath(targetPath)) {
    throw new Error("Operation lock does not match the managed target");
  }
  const ownedLock = operationLock || await acquireOperationLock({
    buildRoot,
    command: ["ensure", targetPath],
    kind: "prepare",
    targetPath,
  });
  try {
    return await ensureAgentTargetUnlocked({ buildRoot, commonDir, now, worktreePath });
  } finally {
    if (!operationLock) await releaseOperationLock(ownedLock);
  }
}

export async function readTargetManifest(targetPath) {
  const manifest = JSON.parse(await readFile(path.join(targetPath, TARGET_MANIFEST), "utf8"));
  if (manifest.schemaVersion !== 1) throw new Error(`Unsupported target manifest schema: ${manifest.schemaVersion}`);
  return manifest;
}

async function pathExists(value) {
  try {
    await lstat(value);
    return true;
  } catch (error) {
    if (error?.code === "ENOENT") return false;
    throw error;
  }
}

export async function validateManagedTargetIdentity({ buildRoot, candidate, currentCommonDir, currentWorktreePath }) {
  const agentsRoot = path.resolve(buildRoot, "agents");
  const agentsInfo = await lstat(agentsRoot);
  if (agentsInfo.isSymbolicLink()) throw new Error("Managed agents root is a symlink or junction");
  const candidateInfo = await lstat(candidate);
  if (candidateInfo.isSymbolicLink()) throw new Error("Managed target is a symlink or junction");
  const resolvedAgentsRoot = await realpath(agentsRoot);
  const resolvedCandidate = await realpath(candidate);
  if (comparisonPath(path.dirname(resolvedCandidate)) !== comparisonPath(resolvedAgentsRoot)) {
    throw new Error("Managed target is not a direct child of the agents root");
  }
  const manifest = await readTargetManifest(resolvedCandidate);
  if (comparisonPath(manifest.targetPath) !== comparisonPath(resolvedCandidate)) {
    throw new Error("Managed target manifest target identity mismatch");
  }
  if (currentCommonDir && comparisonPath(manifest.commonDir) !== comparisonPath(currentCommonDir)) {
    throw new Error("Managed target manifest repository identity mismatch");
  }
  if (currentWorktreePath && comparisonPath(manifest.worktreePath) !== comparisonPath(currentWorktreePath)) {
    throw new Error("Managed target manifest Worktree identity mismatch");
  }
  const deterministicTarget = resolveAgentTarget({
    buildRoot,
    commonDir: manifest.commonDir,
    worktreePath: manifest.worktreePath,
  });
  if (comparisonPath(deterministicTarget) !== comparisonPath(resolvedCandidate)) {
    throw new Error("Managed target path does not match its deterministic identity");
  }
  for (const field of ["createdAt", "lastUsedAt"]) {
    if (!Number.isFinite(Date.parse(manifest[field]))) throw new Error(`Managed target manifest ${field} is invalid`);
  }
  return { candidate: resolvedCandidate, manifest };
}

export function resolveOperationLockPath({ buildRoot, targetPath }) {
  return path.resolve(buildRoot, "locks", `${path.basename(targetPath)}${BUILD_LEASE}`);
}

function defaultProcessAlive(pid) {
  if (!Number.isSafeInteger(pid) || pid <= 0) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return error?.code === "EPERM";
  }
}

async function readLockMetadata(lockPath) {
  try {
    return JSON.parse(await readFile(path.join(lockPath, LOCK_METADATA), "utf8"));
  } catch (error) {
    if (!["ENOENT", "EACCES", "EPERM", "SyntaxError"].includes(error?.code) && !(error instanceof SyntaxError)) throw error;
    return null;
  }
}

export async function hasActiveBuildLease(targetPath, buildRoot) {
  return await pathExists(resolveOperationLockPath({ buildRoot, targetPath }));
}

export async function inspectOperationLock({
  buildRoot,
  targetPath,
  now = Date.now(),
  staleAfterMs = 5 * 60 * 1000,
  processAlive = defaultProcessAlive,
}) {
  const lockPath = resolveOperationLockPath({ buildRoot, targetPath });
  if (!(await pathExists(lockPath))) return { lockPath, state: "NONE" };
  const owner = await readLockMetadata(lockPath);
  let lockAgeMs;
  try {
    lockAgeMs = now - (await stat(lockPath)).mtimeMs;
  } catch (error) {
    if (error?.code === "ENOENT") return { lockPath, state: "NONE" };
    throw error;
  }
  const ownerCreatedAt = Date.parse(owner?.createdAt);
  const ageMs = Number.isFinite(ownerCreatedAt) ? now - ownerCreatedAt : lockAgeMs;
  const ownerAlive = processAlive(owner?.pid);
  return {
    ageMs,
    lockPath,
    owner,
    state: ageMs >= staleAfterMs && !ownerAlive ? "RECOVERABLE_STALE" : "LIVE",
  };
}

export async function acquireOperationLock({
  buildRoot,
  targetPath,
  kind,
  command = [],
  now = Date.now(),
  pid = process.pid,
  staleAfterMs = 5 * 60 * 1000,
  processAlive = defaultProcessAlive,
}) {
  const locksRoot = path.resolve(buildRoot, "locks");
  await mkdir(locksRoot, { recursive: true });
  const locksInfo = await lstat(locksRoot);
  if (!locksInfo.isDirectory() || locksInfo.isSymbolicLink()) throw new Error("Managed lock root is not a safe directory");
  const lockPath = resolveOperationLockPath({ buildRoot, targetPath });
  const token = crypto.randomUUID();
  const metadata = { command, createdAt: new Date(now).toISOString(), kind, pid, token };

  for (let attempt = 0; attempt < 2; attempt += 1) {
    try {
      await mkdir(lockPath);
      try {
        await writeFile(path.join(lockPath, LOCK_METADATA), `${JSON.stringify(metadata)}\n`, "utf8");
      } catch (writeError) {
        await rm(lockPath, { recursive: true, force: true });
        throw writeError;
      }
      return { lockPath, targetPath, token };
    } catch (error) {
      if (error?.code !== "EEXIST") throw error;
      const lockState = await inspectOperationLock({ buildRoot, now, processAlive, staleAfterMs, targetPath });
      if (lockState.state === "NONE") continue;
      if (lockState.state !== "RECOVERABLE_STALE") {
        throw new Error(`Managed target is locked by ${lockState.owner?.kind || "another operation"}`);
      }
      const recoveryPath = `${lockPath}.stale-${token}`;
      try {
        await rename(lockPath, recoveryPath);
      } catch (renameError) {
        if (["ENOENT", "EEXIST"].includes(renameError?.code)) continue;
        throw renameError;
      }
      await rm(recoveryPath, { recursive: true, force: true });
    }
  }
  throw new Error("Unable to acquire managed target operation lock");
}

export async function releaseOperationLock(lock) {
  if (!lock) return;
  const owner = await readLockMetadata(lock.lockPath);
  if (owner?.token !== lock.token) throw new Error("Refusing to release an operation lock owned by another process");
  const releasePath = `${lock.lockPath}.release-${lock.token}`;
  await rename(lock.lockPath, releasePath);
  await rm(releasePath, { recursive: true, force: true });
}

async function existingRealpath(value) {
  try {
    return await realpath(value);
  } catch (error) {
    if (error?.code === "ENOENT") return normalizedAbsolute(value);
    throw error;
  }
}

function isInside(root, candidate) {
  const relative = path.relative(root, candidate);
  return relative !== "" && !relative.startsWith("..") && !path.isAbsolute(relative);
}

export async function validateCleanupCandidate({ buildRoot, candidate, worktreeRoot }) {
  const root = await existingRealpath(buildRoot);
  const agentsRoot = path.resolve(root, "agents");
  const candidateInfo = await lstat(candidate);
  if (candidateInfo.isSymbolicLink()) throw new Error("Cleanup candidate is a symlink or junction");
  const resolvedCandidate = await realpath(candidate);
  const resolvedWorktree = await existingRealpath(worktreeRoot);
  if (!isInside(root, resolvedCandidate)) throw new Error("Cleanup candidate escapes the controlled build root");
  if (comparisonPath(path.dirname(resolvedCandidate)) !== comparisonPath(agentsRoot))
    throw new Error("Cleanup candidate is not a direct child of the agents root");
  if (comparisonPath(resolvedCandidate) === comparisonPath(root)) throw new Error("Refusing to delete build root");
  if (comparisonPath(resolvedCandidate) === comparisonPath(resolvedWorktree)) throw new Error("Refusing to delete Worktree root");
  const segments = comparisonPath(resolvedCandidate).split("/");
  if (segments.includes(".git")) throw new Error("Refusing to delete a .git path");
  const safetyScan = await scanDirectoryOnce(resolvedCandidate);
  if (safetyScan.reparsePoints > 0) throw new Error("Cleanup candidate contains a symlink or junction");
  return {
    agentsRoot,
    candidate: resolvedCandidate,
    identity: { dev: String(candidateInfo.dev), ino: String(candidateInfo.ino) },
    root,
  };
}

function sameFileIdentity(left, right) {
  return left.dev === right.dev && left.ino === right.ino;
}

export async function planCloseout({
  accepted,
  apply,
  buildRoot,
  commonDir,
  gitClean,
  isLinkedWorktree,
  mergedIntoMain,
  policy,
  targetKind,
  worktreePath,
}) {
  const targetPath = resolveAgentTarget({ buildRoot, commonDir, worktreePath });
  const reasons = [];
  if (!isLinkedWorktree) reasons.push("MAIN_NOT_ELIGIBLE");
  if (!gitClean) reasons.push("WORKTREE_DIRTY");
  if (isLinkedWorktree && !mergedIntoMain) reasons.push("NOT_MERGED_INTO_MAIN");
  if (!accepted) reasons.push("ACCEPTED_REQUIRED");

  let manifest = null;
  let manifestValid = false;
  let activeBuild = false;
  let scan = { bytes: 0, elapsedMs: 0, files: 0, truncated: false };
  try {
    ({ manifest } = await validateManagedTargetIdentity({
      buildRoot,
      candidate: targetPath,
      currentCommonDir: commonDir,
    }));
    manifestValid = true;
    if (comparisonPath(manifest.worktreePath) !== comparisonPath(worktreePath)) reasons.push("MANIFEST_WORKTREE_MISMATCH");
    const lockState = await inspectOperationLock({
      buildRoot,
      staleAfterMs: (policy?.operationLockStaleMinutes ?? 5) * 60 * 1000,
      targetPath,
    });
    activeBuild = lockState.state === "LIVE";
    if (activeBuild) reasons.push("ACTIVE_BUILD_LEASE");
    await validateCleanupCandidate({ buildRoot, candidate: targetPath, worktreeRoot: worktreePath });
    scan = await scanDirectoryOnce(targetPath);
    if (scan.reparsePoints > 0) reasons.push("REPARSE_POINT_PRESENT");
  } catch (error) {
    reasons.push(error?.code === "ENOENT" ? "TARGET_NOT_FOUND" : `TARGET_INVALID: ${error.message}`);
  }

  const classification = classifyManagedTarget({
    activeBuild,
    gitClean,
    manifestValid,
    mergedIntoMain,
    registered: Boolean(isLinkedWorktree),
    targetKind: isLinkedWorktree ? targetKind : STATUS.MAIN,
  });
  if (classification.status === STATUS.MAIN || classification.status === STATUS.RELEASE) {
    reasons.push("PROTECTED_TARGET_CLASS");
  }

  return {
    accepted: Boolean(accepted),
    apply: Boolean(apply),
    buildRoot: normalizedAbsolute(buildRoot),
    bytes: scan.bytes,
    commonDir: normalizedAbsolute(commonDir),
    eligible: reasons.length === 0,
    gitClean: Boolean(gitClean),
    manifest,
    reasons,
    targetClass: classification.targetClass,
    targetPath,
    worktreePath: normalizedAbsolute(worktreePath),
  };
}

export async function applyCleanupPlan(plan, options = {}) {
  if (!plan.apply) return { deleted: false, reason: "DRY_RUN", targetPath: plan.targetPath };
  if (!plan.eligible) throw new Error(`Cleanup plan is not eligible: ${plan.reasons.join(", ")}`);
  const operationLock = await acquireOperationLock({
    buildRoot: plan.buildRoot,
    command: ["cleanup", plan.targetPath],
    kind: "cleanup",
    processAlive: options.processAlive,
    staleAfterMs: options.staleAfterMs,
    targetPath: plan.targetPath,
  });
  try {
    await validateManagedTargetIdentity({
      buildRoot: plan.buildRoot,
      candidate: plan.targetPath,
      currentCommonDir: plan.commonDir,
      currentWorktreePath: plan.worktreePath,
    });
    const initialValidation = await validateCleanupCandidate({
      buildRoot: plan.buildRoot,
      candidate: plan.targetPath,
      worktreeRoot: plan.worktreePath,
    });
    const removeCandidate = options.removeCandidate || ((candidate) =>
      rm(candidate, { recursive: true, force: false, maxRetries: 2, retryDelay: 100 }));
    const platform = options.platform || process.platform;
    try {
      await removeCandidate(plan.targetPath);
    } catch (error) {
      if (!options.repairAcl || platform !== "win32" || !["EACCES", "EPERM"].includes(error?.code)) throw error;
      if (!options.repairAclCandidate) throw new Error("ACL repair handler is unavailable");
      await validateManagedTargetIdentity({
        buildRoot: plan.buildRoot,
        candidate: plan.targetPath,
        currentCommonDir: plan.commonDir,
        currentWorktreePath: plan.worktreePath,
      });
      const beforeRepair = await validateCleanupCandidate({
        buildRoot: plan.buildRoot,
        candidate: plan.targetPath,
        worktreeRoot: plan.worktreePath,
      });
      if (!sameFileIdentity(initialValidation.identity, beforeRepair.identity)) {
        throw new Error("Cleanup candidate identity changed before ACL repair");
      }
      await options.repairAclCandidate(beforeRepair.candidate, beforeRepair.identity);
      await validateManagedTargetIdentity({
        buildRoot: plan.buildRoot,
        candidate: plan.targetPath,
        currentCommonDir: plan.commonDir,
        currentWorktreePath: plan.worktreePath,
      });
      const afterRepair = await validateCleanupCandidate({
        buildRoot: plan.buildRoot,
        candidate: plan.targetPath,
        worktreeRoot: plan.worktreePath,
      });
      if (!sameFileIdentity(beforeRepair.identity, afterRepair.identity)) {
        throw new Error("Cleanup candidate identity changed during ACL repair");
      }
      await removeCandidate(plan.targetPath);
    }
  } finally {
    await releaseOperationLock(operationLock);
  }
  return { bytesFreed: plan.bytes, deleted: true, targetPath: plan.targetPath };
}

export async function planAgentCleanup({
  apply,
  buildRoot,
  commonDir,
  deadlineMs,
  now = Date.now(),
  policy,
  registeredWorktrees,
  worktreeStates = new Map(),
}) {
  const agentsRoot = path.resolve(buildRoot, "agents");
  const entries = [];
  let directory;
  try {
    const agentsInfo = await lstat(agentsRoot);
    if (agentsInfo.isSymbolicLink()) throw new Error("Managed agents root is a symlink or junction");
    directory = await opendir(agentsRoot);
  } catch (error) {
    if (error?.code === "ENOENT") return { apply: Boolean(apply), entries, targetRoot: buildRoot };
    throw error;
  }
  for await (const entry of directory) {
    if (!entry.isDirectory() && !entry.isSymbolicLink()) continue;
    const targetPath = path.join(agentsRoot, entry.name);
    try {
      if (entry.isSymbolicLink()) throw new Error("Managed target is a symlink or junction");
      const { manifest } = await validateManagedTargetIdentity({
        buildRoot,
        candidate: targetPath,
        currentCommonDir: commonDir,
      });
      const scan = await scanDirectoryOnce(targetPath, {
        maxEntries: policy.report.maxEntriesPerScan,
        timeoutMs: Math.max(
          0,
          Math.min(
            policy.report.externalScanTimeoutMs,
            deadlineMs === undefined ? Number.POSITIVE_INFINITY : deadlineMs - performance.now(),
          ),
        ),
      });
      const lockState = await inspectOperationLock({
        buildRoot,
        now,
        staleAfterMs: (policy.operationLockStaleMinutes ?? 5) * 60 * 1000,
        targetPath,
      });
      const activeBuild = lockState.state === "LIVE";
      const classification = classifyExternalTarget({
        activeBuild,
        manifest,
        now,
        policy,
        registeredWorktrees,
        worktreeStates,
      });
      entries.push({
        ...classification,
        activeBuild,
        recoverableStaleLock: lockState.state === "RECOVERABLE_STALE",
        bytes: scan.bytes,
        files: scan.files,
        manifest,
        scanElapsedMs: scan.elapsedMs,
        selected:
          Boolean(apply) &&
          classification.status === STATUS.ORPHAN &&
          classification.cleanupEligible &&
          !activeBuild,
        targetPath,
        truncated: scan.truncated,
      });
    } catch (error) {
      entries.push({
        cleanupEligible: false,
        error: error.message,
        selected: false,
        status: STATUS.ORPHAN,
        targetClass: STATUS.ORPHAN,
        targetPath,
      });
    }
  }
  return { apply: Boolean(apply), entries, targetRoot: buildRoot };
}

export async function buildStorageReport({ entries, freeBytes, policy, startedAt, targetRoot }) {
  const agentStatuses = new Set([STATUS.ACTIVE, STATUS.CLOSEOUT_ELIGIBLE, STATUS.ORPHAN]);
  const totalAgentBytes = entries
    .filter((entry) => agentStatuses.has(entry.status))
    .reduce((total, entry) => total + (Number.isFinite(entry.bytes) ? entry.bytes : 0), 0);
  const largestAgent = entries
    .filter((entry) => agentStatuses.has(entry.status))
    .reduce((largest, entry) => Math.max(largest, Number.isFinite(entry.bytes) ? entry.bytes : 0), 0);
  return {
    budget: evaluateBudget({ totalBytes: totalAgentBytes, targetBytes: largestAgent, freeBytes }, policy),
    entries,
    freeBytes,
    scanElapsedMs: performance.now() - startedAt,
    targetRoot,
    totalAgentBytes,
  };
}
