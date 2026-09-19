import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, realpath, rm, stat, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  STATUS,
  acquireOperationLock,
  applyCleanupPlan,
  buildStorageReport,
  classifyExternalTarget,
  ensureAgentTarget,
  failClosedOnIncompleteScans,
  evaluateBudget,
  managedBudgetInputs,
  planAgentCleanup,
  planCloseout,
  resolveAgentTarget,
  resolveBuildRoot,
  releaseOperationLock,
  scanDirectoryOnce,
  validateCleanupCandidate,
  validateManagedTargetIdentity,
  writeTargetManifest,
} from "./storage-governance-core.mjs";

const GiB = 1024 ** 3;

const policy = {
  schemaVersion: 1,
  thresholds: {
    totalWarningGiB: 80,
    totalCriticalGiB: 120,
    targetWarningGiB: 25,
    targetCriticalGiB: 40,
    freeWarningGiB: 100,
    freeBlockGiB: 50,
  },
  orphanMinimumAgeDays: 7,
  operationLockStaleMinutes: 5,
  report: { externalScanTimeoutMs: 30_000, legacyScanTimeoutMs: 5_000 },
};

async function withTempDir(fn) {
  const root = await mkdtemp(path.join(os.tmpdir(), "codexmonitor-storage-test-"));
  try {
    return await fn(root);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

test("build root override wins and Windows default stays beside the main checkout", () => {
  assert.equal(
    resolveBuildRoot({
      platform: "win32",
      env: { CODEXMONITOR_CARGO_TARGET_ROOT: "D:\\custom target" },
      homeDir: "C:\\Users\\Dev",
      mainRoot: "F:\\AI\\CodexMonitor",
    }),
    path.resolve("D:\\custom target"),
  );

  assert.equal(
    resolveBuildRoot({
      platform: "win32",
      env: {},
      homeDir: "C:\\Users\\Dev",
      mainRoot: "F:\\AI\\CodexMonitor",
    }),
    path.resolve("F:\\AI\\_cargo-target\\CodexMonitor"),
  );
});

test("two Worktrees receive different external target directories", () => {
  const first = resolveAgentTarget({
    buildRoot: "F:\\AI\\_cargo-target\\CodexMonitor",
    commonDir: "F:\\AI\\CodexMonitor\\.git",
    worktreePath: "F:\\AI\\CodexMonitor\\.worktrees\\one",
  });
  const second = resolveAgentTarget({
    buildRoot: "F:\\AI\\_cargo-target\\CodexMonitor",
    commonDir: "F:\\AI\\CodexMonitor\\.git",
    worktreePath: "F:\\AI\\CodexMonitor\\.worktrees\\two",
  });

  assert.notEqual(first, second);
  assert.equal(path.dirname(first), path.resolve("F:\\AI\\_cargo-target\\CodexMonitor\\agents"));
  assert.equal(path.dirname(second), path.dirname(first));
});

test("budget guard warns early and blocks critical ephemeral builds", () => {
  const warning = evaluateBudget(
    { totalBytes: 81 * GiB, targetBytes: 26 * GiB, freeBytes: 99 * GiB },
    policy,
  );
  assert.equal(warning.blocked, false);
  assert.deepEqual(warning.warnings.map((entry) => entry.code), [
    "TOTAL_WARNING",
    "TARGET_WARNING",
    "FREE_WARNING",
  ]);

  const critical = evaluateBudget(
    { totalBytes: 121 * GiB, targetBytes: 41 * GiB, freeBytes: 49 * GiB },
    policy,
  );
  assert.equal(critical.blocked, true);
  assert.deepEqual(critical.reasons.map((entry) => entry.code), [
    "TOTAL_CRITICAL",
    "TARGET_CRITICAL",
    "FREE_BLOCK",
  ]);

  const overridden = evaluateBudget(
    { totalBytes: 121 * GiB, targetBytes: 41 * GiB, freeBytes: 49 * GiB },
    policy,
    { override: true },
  );
  assert.equal(overridden.blocked, false);
  assert.equal(overridden.overrideApplied, true);
});

test("budget guard fails closed when a managed target scan is incomplete", () => {
  const budget = evaluateBudget({ totalBytes: 1, targetBytes: 1, freeBytes: 500 * GiB }, policy);
  const blocked = failClosedOnIncompleteScans(budget, [{ targetPath: "F:\\target", truncated: true }]);
  assert.equal(blocked.blocked, true);
  assert.equal(blocked.reasons.at(-1).code, "SCAN_INCOMPLETE");

  const overridden = failClosedOnIncompleteScans(budget, [{ error: "access denied", targetPath: "F:\\target" }], true);
  assert.equal(overridden.blocked, false);
  assert.equal(overridden.overrideApplied, true);
});

test("budget guard evaluates the largest managed target, not only the current target", () => {
  const inputs = managedBudgetInputs(
    [
      { bytes: 2 * GiB, targetPath: "F:\\current" },
      { bytes: 41 * GiB, targetPath: "F:\\other" },
    ],
    "F:\\current",
    200 * GiB,
  );
  assert.equal(inputs.targetBytes, 41 * GiB);
  assert.equal(inputs.currentTargetBytes, 2 * GiB);
});

test("external target classification distinguishes active, closeout, and orphan TTL", () => {
  const now = Date.parse("2026-09-19T00:00:00Z");
  const activeManifest = {
    worktreePath: "F:\\repo\\.worktrees\\active",
    lastUsedAt: "2026-09-18T00:00:00Z",
  };
  assert.equal(
    classifyExternalTarget({
      manifest: activeManifest,
      registeredWorktrees: new Set([activeManifest.worktreePath.toLowerCase()]),
      worktreeStates: new Map([[activeManifest.worktreePath, { gitClean: false, mergedIntoMain: true }]]),
      now,
      policy,
    }).status,
    STATUS.ACTIVE,
  );
  assert.equal(
    classifyExternalTarget({
      manifest: activeManifest,
      registeredWorktrees: new Set([activeManifest.worktreePath.toLowerCase()]),
      worktreeStates: new Map([[activeManifest.worktreePath, { gitClean: true, mergedIntoMain: true }]]),
      now,
      policy,
    }).status,
    STATUS.CLOSEOUT_ELIGIBLE,
  );

  const newOrphan = classifyExternalTarget({
    manifest: activeManifest,
    registeredWorktrees: new Set(),
    now,
    policy,
  });
  assert.equal(newOrphan.status, STATUS.ORPHAN);
  assert.equal(newOrphan.cleanupEligible, false);

  const oldOrphan = classifyExternalTarget({
    manifest: { ...activeManifest, lastUsedAt: "2026-09-01T00:00:00Z" },
    registeredWorktrees: new Set(),
    now,
    policy,
  });
  assert.equal(oldOrphan.status, STATUS.ORPHAN);
  assert.equal(oldOrphan.cleanupEligible, true);
});

test("single-pass scan reports size, count, elapsed time, and truncation state", async () => {
  await withTempDir(async (root) => {
    await mkdir(path.join(root, "nested"));
    await writeFile(path.join(root, "a.bin"), Buffer.alloc(7));
    await writeFile(path.join(root, "nested", "b.bin"), Buffer.alloc(11));
    const visited = [];
    const result = await scanDirectoryOnce(root, { onVisit: (entry) => visited.push(entry) });
    assert.equal(result.bytes, 18);
    assert.equal(result.files, 2);
    assert.equal(result.truncated, false);
    assert.ok(result.elapsedMs >= 0);
    assert.equal(new Set(visited).size, visited.length);
  });
});

test("cleanup validation rejects the controlled root, Worktree root, .git, and symlink escapes", async (t) => {
  await withTempDir(async (root) => {
    const buildRoot = path.join(root, "build");
    const agentsRoot = path.join(buildRoot, "agents");
    const candidate = path.join(agentsRoot, "feature-a-1234");
    const worktreeRoot = path.join(root, "worktree");
    await mkdir(candidate, { recursive: true });
    await mkdir(path.join(worktreeRoot, ".git"), { recursive: true });

    await assert.rejects(() => validateCleanupCandidate({ buildRoot, candidate: buildRoot, worktreeRoot }));
    await assert.rejects(() => validateCleanupCandidate({ buildRoot, candidate: worktreeRoot, worktreeRoot }));
    await assert.rejects(() =>
      validateCleanupCandidate({ buildRoot, candidate: path.join(worktreeRoot, ".git"), worktreeRoot }),
    );
    await validateCleanupCandidate({ buildRoot, candidate, worktreeRoot });

    const link = path.join(agentsRoot, "feature-link-1234");
    try {
      await import("node:fs/promises").then(({ symlink }) =>
        symlink(candidate, link, process.platform === "win32" ? "junction" : "dir"),
      );
      await assert.rejects(() => validateCleanupCandidate({ buildRoot, candidate: link, worktreeRoot }));
    } catch (error) {
      if (error?.code === "EPERM") t.diagnostic("symlink fixture unavailable under current Windows token");
      else throw error;
    }
  });
});

test("closeout apply deletes only a verified external target and preserves Worktree source", async () => {
  await withTempDir(async (root) => {
    const buildRoot = path.join(root, "build");
    const worktreeRoot = path.join(root, "worktree");
    const target = resolveAgentTarget({
      buildRoot,
      commonDir: path.join(root, "repo", ".git"),
      worktreePath: worktreeRoot,
    });
    await mkdir(target, { recursive: true });
    await mkdir(worktreeRoot, { recursive: true });
    await writeFile(path.join(worktreeRoot, "source.txt"), "keep");
    await writeFile(path.join(target, "artifact.bin"), Buffer.alloc(13));
    await writeTargetManifest(target, {
      commonDir: path.join(root, "repo", ".git"),
      worktreePath: worktreeRoot,
      targetPath: target,
      createdAt: "2026-09-19T00:00:00Z",
      lastUsedAt: "2026-09-19T00:00:00Z",
    });

    const dryRun = await planCloseout({
      accepted: true,
      apply: false,
      buildRoot,
      commonDir: path.join(root, "repo", ".git"),
      gitClean: true,
      isLinkedWorktree: true,
      mergedIntoMain: true,
      worktreePath: worktreeRoot,
    });
    assert.equal(dryRun.targetClass, STATUS.CLOSEOUT_ELIGIBLE);
    assert.equal(dryRun.eligible, true);
    assert.equal(dryRun.apply, false);
    assert.ok((await stat(target)).isDirectory());

    const result = await applyCleanupPlan({ ...dryRun, apply: true });
    assert.equal(result.deleted, true);
    await assert.rejects(() => stat(target), { code: "ENOENT" });
    assert.equal(await readFile(path.join(worktreeRoot, "source.txt"), "utf8"), "keep");
    assert.equal(await realpath(worktreeRoot), await realpath(worktreeRoot));
  });
});

test("managed target classification is shared by report planning and closeout", async () => {
  await withTempDir(async (root) => {
    const buildRoot = path.join(root, "build");
    const commonDir = path.join(root, "repo", ".git");
    const worktreePath = path.join(root, "worktree");
    await ensureAgentTarget({ buildRoot, commonDir, worktreePath });

    const reportPlan = await planAgentCleanup({
      apply: false,
      buildRoot,
      commonDir,
      policy,
      registeredWorktrees: new Set([worktreePath]),
      worktreeStates: new Map([[worktreePath, { gitClean: true, mergedIntoMain: true }]]),
    });
    const closeoutPlan = await planCloseout({
      accepted: true,
      apply: false,
      buildRoot,
      commonDir,
      gitClean: true,
      isLinkedWorktree: true,
      mergedIntoMain: true,
      policy,
      worktreePath,
    });

    assert.equal(reportPlan.entries[0].targetClass, STATUS.CLOSEOUT_ELIGIBLE);
    assert.equal(reportPlan.entries[0].targetClass, closeoutPlan.targetClass);

    const unacceptedPlan = await planCloseout({
      accepted: false,
      apply: false,
      buildRoot,
      commonDir,
      gitClean: true,
      isLinkedWorktree: true,
      mergedIntoMain: true,
      policy,
      worktreePath,
    });
    assert.equal(unacceptedPlan.targetClass, STATUS.CLOSEOUT_ELIGIBLE);
    assert.equal(unacceptedPlan.eligible, false);
    assert.ok(unacceptedPlan.reasons.includes("ACCEPTED_REQUIRED"));
  });
});

test("closeout keeps dirty and unmerged registered targets active", async () => {
  await withTempDir(async (root) => {
    const buildRoot = path.join(root, "build");
    const commonDir = path.join(root, "repo", ".git");
    const worktreePath = path.join(root, "worktree");
    await ensureAgentTarget({ buildRoot, commonDir, worktreePath });
    const base = {
      accepted: true,
      apply: true,
      buildRoot,
      commonDir,
      isLinkedWorktree: true,
      policy,
      worktreePath,
    };

    const dirty = await planCloseout({ ...base, gitClean: false, mergedIntoMain: true });
    assert.equal(dirty.targetClass, STATUS.ACTIVE);
    assert.equal(dirty.eligible, false);

    const unmerged = await planCloseout({ ...base, gitClean: true, mergedIntoMain: false });
    assert.equal(unmerged.targetClass, STATUS.ACTIVE);
    assert.equal(unmerged.eligible, false);
    assert.ok(unmerged.reasons.includes("NOT_MERGED_INTO_MAIN"));
  });
});

test("closeout refuses missing acceptance, dirty source, main, and release targets", async () => {
  await withTempDir(async (root) => {
    const base = {
      apply: true,
      buildRoot: path.join(root, "build"),
      commonDir: path.join(root, "repo", ".git"),
      gitClean: true,
      isLinkedWorktree: true,
      mergedIntoMain: true,
      worktreePath: path.join(root, "worktree"),
    };
    assert.equal((await planCloseout({ ...base, accepted: false })).eligible, false);
    assert.equal((await planCloseout({ ...base, accepted: true, gitClean: false })).eligible, false);
    const main = await planCloseout({ ...base, accepted: true, isLinkedWorktree: false });
    assert.equal(main.targetClass, STATUS.MAIN);
    assert.equal(main.eligible, false);
    const release = await planCloseout({ ...base, accepted: true, targetKind: STATUS.RELEASE });
    assert.equal(release.targetClass, STATUS.RELEASE);
    assert.equal(release.eligible, false);
  });
});

test("storage report exposes all required target classes without deleting data", async () => {
  await withTempDir(async (root) => {
    const report = await buildStorageReport({
      entries: [
        { path: path.join(root, "main"), status: STATUS.MAIN, bytes: 1 },
        { path: path.join(root, "agent"), status: STATUS.ACTIVE, bytes: 2 },
        { path: path.join(root, "eligible"), status: STATUS.CLOSEOUT_ELIGIBLE, bytes: 3 },
        { path: path.join(root, "orphan"), status: STATUS.ORPHAN, bytes: 4 },
        { path: path.join(root, "legacy"), status: STATUS.LEGACY_IN_TREE, bytes: 5 },
        { path: path.join(root, "release"), status: STATUS.RELEASE, bytes: 6 },
      ],
      freeBytes: 200 * GiB,
      policy,
      startedAt: performance.now(),
      targetRoot: path.join(root, "build"),
    });
    assert.deepEqual(
      report.entries.map((entry) => entry.status),
      [
        STATUS.MAIN,
        STATUS.ACTIVE,
        STATUS.CLOSEOUT_ELIGIBLE,
        STATUS.ORPHAN,
        STATUS.LEGACY_IN_TREE,
        STATUS.RELEASE,
      ],
    );
    assert.equal(report.totalAgentBytes, 9);
    assert.ok(report.scanElapsedMs >= 0);

    const reportWithError = await buildStorageReport({
      entries: [{ error: "unreadable", status: STATUS.ORPHAN }],
      freeBytes: 200 * GiB,
      policy,
      startedAt: performance.now(),
      targetRoot: path.join(root, "build"),
    });
    assert.equal(reportWithError.totalAgentBytes, 0);
    assert.equal(Number.isNaN(reportWithError.totalAgentBytes), false);
  });
});

test("ensure creates and refreshes a schema-versioned target manifest", async () => {
  await withTempDir(async (root) => {
    const input = {
      buildRoot: path.join(root, "build"),
      commonDir: path.join(root, "repo", ".git"),
      worktreePath: path.join(root, "repo", ".worktrees", "feature"),
    };
    const first = await ensureAgentTarget({ ...input, now: "2026-09-18T00:00:00Z" });
    const second = await ensureAgentTarget({ ...input, now: "2026-09-19T00:00:00Z" });
    assert.equal(first.targetPath, second.targetPath);
    assert.equal(second.manifest.createdAt, "2026-09-18T00:00:00Z");
    assert.equal(second.manifest.lastUsedAt, "2026-09-19T00:00:00Z");
  });
});

test("ensure refuses to certify pre-existing content or overwrite a forged manifest", async () => {
  await withTempDir(async (root) => {
    const input = {
      buildRoot: path.join(root, "build"),
      commonDir: path.join(root, "repo", ".git"),
      worktreePath: path.join(root, "repo", ".worktrees", "feature"),
    };
    const targetPath = resolveAgentTarget(input);
    await mkdir(targetPath, { recursive: true });
    await writeFile(path.join(targetPath, "pre-existing.bin"), "do not manage");
    await assert.rejects(() => ensureAgentTarget(input), /non-empty target/i);

    await rm(targetPath, { recursive: true, force: true });
    await writeTargetManifest(targetPath, {
      commonDir: path.join(root, "other", ".git"),
      targetPath,
      worktreePath: input.worktreePath,
    });
    await assert.rejects(() => ensureAgentTarget(input), /identity|repository|deterministic/i);
  });
});

test("cleanup validation rejects a junction nested anywhere inside a candidate", async (t) => {
  await withTempDir(async (root) => {
    const buildRoot = path.join(root, "build");
    const candidate = path.join(buildRoot, "agents", "feature-a-1234");
    const outside = path.join(root, "outside");
    await mkdir(candidate, { recursive: true });
    await mkdir(outside, { recursive: true });
    const link = path.join(candidate, "escape");
    try {
      await import("node:fs/promises").then(({ symlink }) =>
        symlink(outside, link, process.platform === "win32" ? "junction" : "dir"),
      );
    } catch (error) {
      if (error?.code === "EPERM") {
        t.diagnostic("nested junction fixture unavailable under current Windows token");
        return;
      }
      throw error;
    }
    await assert.rejects(() =>
      validateCleanupCandidate({ buildRoot, candidate, worktreeRoot: path.join(root, "worktree") }),
    );
  });
});

test("orphan cleanup plans mark new orphans but select only targets older than TTL", async () => {
  await withTempDir(async (root) => {
    const buildRoot = path.join(root, "build");
    const commonDir = path.join(root, "repo", ".git");
    const recent = await ensureAgentTarget({
      buildRoot,
      commonDir,
      now: "2026-09-18T00:00:00Z",
      worktreePath: path.join(root, "recent"),
    });
    const old = await ensureAgentTarget({
      buildRoot,
      commonDir,
      now: "2026-09-01T00:00:00Z",
      worktreePath: path.join(root, "old"),
    });
    const plan = await planAgentCleanup({
      apply: true,
      buildRoot,
      now: Date.parse("2026-09-19T00:00:00Z"),
      policy,
      registeredWorktrees: new Set(),
    });
    const recentEntry = plan.entries.find((entry) => entry.targetPath === recent.targetPath);
    const oldEntry = plan.entries.find((entry) => entry.targetPath === old.targetPath);
    assert.equal(recentEntry.status, STATUS.ORPHAN);
    assert.equal(recentEntry.cleanupEligible, false);
    assert.equal(recentEntry.selected, false);
    assert.equal(oldEntry.cleanupEligible, true);
    assert.equal(oldEntry.selected, true);
  });
});

test("managed target identity rejects copied manifests and apply re-reads identity", async () => {
  await withTempDir(async (root) => {
    const buildRoot = path.join(root, "build");
    const commonDir = path.join(root, "repo", ".git");
    const worktreePath = path.join(root, "worktree");
    const ensured = await ensureAgentTarget({ buildRoot, commonDir, worktreePath });
    await validateManagedTargetIdentity({
      buildRoot,
      candidate: ensured.targetPath,
      currentCommonDir: commonDir,
    });
    const forgedPath = path.join(buildRoot, "agents", "forged");
    await mkdir(forgedPath, { recursive: true });
    await writeFile(
      path.join(forgedPath, ".codexmonitor-target.json"),
      `${JSON.stringify({ ...ensured.manifest, targetPath: forgedPath })}\n`,
    );
    await assert.rejects(() =>
      validateManagedTargetIdentity({ buildRoot, candidate: forgedPath, currentCommonDir: commonDir }),
    );

    const plan = await planCloseout({
      accepted: true,
      apply: true,
      buildRoot,
      commonDir,
      gitClean: true,
      isLinkedWorktree: true,
      mergedIntoMain: true,
      worktreePath,
    });
    await writeTargetManifest(ensured.targetPath, {
      commonDir: path.join(root, "other", ".git"),
      worktreePath,
      targetPath: ensured.targetPath,
    });
    await assert.rejects(() => applyCleanupPlan(plan), /identity|repository|deterministic/i);
  });
});

test("active build lease blocks closeout and orphan cleanup selection", async () => {
  await withTempDir(async (root) => {
    const buildRoot = path.join(root, "build");
    const commonDir = path.join(root, "repo", ".git");
    const worktreePath = path.join(root, "worktree");
    const ensured = await ensureAgentTarget({
      buildRoot,
      commonDir,
      now: "2026-09-01T00:00:00Z",
      worktreePath,
    });
    const lock = await acquireOperationLock({
      buildRoot,
      command: ["cargo", "check"],
      kind: "build",
      targetPath: ensured.targetPath,
    });
    const closeout = await planCloseout({
      accepted: true,
      apply: true,
      buildRoot,
      commonDir,
      gitClean: true,
      isLinkedWorktree: true,
      mergedIntoMain: true,
      worktreePath,
    });
    assert.equal(closeout.eligible, false);
    assert.equal(closeout.targetClass, STATUS.ACTIVE);
    assert.ok(closeout.reasons.includes("ACTIVE_BUILD_LEASE"));

    const cleanup = await planAgentCleanup({
      apply: true,
      buildRoot,
      commonDir,
      now: Date.parse("2026-09-19T00:00:00Z"),
      policy,
      registeredWorktrees: new Set(),
    });
    assert.equal(cleanup.entries[0].activeBuild, true);
    assert.equal(cleanup.entries[0].selected, false);
    await releaseOperationLock(lock);
  });
});

test("build and cleanup atomically contend for one operation lock", async () => {
  await withTempDir(async (root) => {
    const buildRoot = path.join(root, "build");
    const commonDir = path.join(root, "repo", ".git");
    const worktreePath = path.join(root, "worktree");
    const ensured = await ensureAgentTarget({ buildRoot, commonDir, worktreePath });
    const cleanupLock = await acquireOperationLock({
      buildRoot,
      command: ["cleanup"],
      kind: "cleanup",
      targetPath: ensured.targetPath,
    });
    await assert.rejects(
      () =>
        acquireOperationLock({
          buildRoot,
          command: ["cargo", "check"],
          kind: "build",
          targetPath: ensured.targetPath,
        }),
      /locked/i,
    );
    await assert.rejects(
      () => ensureAgentTarget({ buildRoot, commonDir, worktreePath }),
      /locked/i,
    );
    await releaseOperationLock(cleanupLock);
  });
});

test("a dead stale operation lock is recovered without taking a live lock", async () => {
  await withTempDir(async (root) => {
    const buildRoot = path.join(root, "build");
    const targetPath = path.join(buildRoot, "agents", "feature-123");
    const stale = await acquireOperationLock({
      buildRoot,
      kind: "build",
      now: Date.parse("2026-09-19T00:00:00Z"),
      pid: 111,
      targetPath,
    });
    await assert.rejects(
      () =>
        acquireOperationLock({
          buildRoot,
          kind: "cleanup",
          now: Date.parse("2026-09-19T00:10:00Z"),
          processAlive: () => true,
          staleAfterMs: 5 * 60 * 1000,
          targetPath,
        }),
      /locked/i,
    );
    const recovered = await acquireOperationLock({
      buildRoot,
      kind: "cleanup",
      now: Date.parse("2026-09-19T00:10:00Z"),
      processAlive: () => false,
      staleAfterMs: 5 * 60 * 1000,
      targetPath,
    });
    await assert.rejects(() => releaseOperationLock(stale), /another process/i);
    await releaseOperationLock(recovered);
  });
});

test("closeout and orphan cleanup recover dead stale locks but reject live owners", async () => {
  await withTempDir(async (root) => {
    const buildRoot = path.join(root, "build");
    const commonDir = path.join(root, "repo", ".git");
    const staleWorktree = path.join(root, "stale-worktree");
    const staleTarget = await ensureAgentTarget({
      buildRoot,
      commonDir,
      now: "2026-09-01T00:00:00Z",
      worktreePath: staleWorktree,
    });
    await acquireOperationLock({
      buildRoot,
      kind: "build",
      now: Date.parse("2000-01-01T00:00:00Z"),
      pid: 999_999_999,
      targetPath: staleTarget.targetPath,
    });
    const closeout = await planCloseout({
      accepted: true,
      apply: true,
      buildRoot,
      commonDir,
      gitClean: true,
      isLinkedWorktree: true,
      mergedIntoMain: true,
      policy,
      worktreePath: staleWorktree,
    });
    assert.equal(closeout.eligible, true);
    assert.equal((await applyCleanupPlan(closeout, { staleAfterMs: 1 })).deleted, true);

    const orphanWorktree = path.join(root, "orphan-worktree");
    const orphanTarget = await ensureAgentTarget({
      buildRoot,
      commonDir,
      now: "2026-09-01T00:00:00Z",
      worktreePath: orphanWorktree,
    });
    const liveLock = await acquireOperationLock({
      buildRoot,
      kind: "build",
      now: Date.parse("2000-01-01T00:00:00Z"),
      pid: process.pid,
      targetPath: orphanTarget.targetPath,
    });
    const livePlan = await planAgentCleanup({
      apply: true,
      buildRoot,
      commonDir,
      now: Date.parse("2026-09-19T00:00:00Z"),
      policy,
      registeredWorktrees: new Set(),
    });
    assert.equal(livePlan.entries[0].activeBuild, true);
    assert.equal(livePlan.entries[0].selected, false);
    await releaseOperationLock(liveLock);
    await acquireOperationLock({
      buildRoot,
      kind: "build",
      now: Date.parse("2000-01-01T00:00:00Z"),
      pid: 999_999_999,
      targetPath: orphanTarget.targetPath,
    });
    const staleOrphanPlan = await planAgentCleanup({
      apply: true,
      buildRoot,
      commonDir,
      now: Date.parse("2026-09-19T00:00:00Z"),
      policy,
      registeredWorktrees: new Set(),
    });
    assert.equal(staleOrphanPlan.entries[0].recoverableStaleLock, true);
    assert.equal(staleOrphanPlan.entries[0].selected, true);
    assert.equal(
      (
        await applyCleanupPlan(
          {
            apply: true,
            buildRoot,
            bytes: staleOrphanPlan.entries[0].bytes,
            commonDir,
            eligible: true,
            reasons: [],
            targetPath: orphanTarget.targetPath,
            worktreePath: orphanWorktree,
          },
          { staleAfterMs: 1 },
        )
      ).deleted,
      true,
    );
  });
});

test("ACL repair retries only the already validated cleanup candidate", async () => {
  await withTempDir(async (root) => {
    const buildRoot = path.join(root, "build");
    const worktreeRoot = path.join(root, "worktree");
    const commonDir = path.join(root, "repo", ".git");
    const targetPath = resolveAgentTarget({ buildRoot, commonDir, worktreePath: worktreeRoot });
    await writeTargetManifest(targetPath, { commonDir, targetPath, worktreePath: worktreeRoot });
    let attempts = 0;
    const repaired = [];
    const result = await applyCleanupPlan(
      {
        apply: true,
        buildRoot,
        bytes: 1,
        commonDir,
        eligible: true,
        reasons: [],
        targetPath,
        worktreePath: worktreeRoot,
      },
      {
        platform: "win32",
        repairAcl: true,
        repairAclCandidate: async (candidate) => repaired.push(candidate),
        removeCandidate: async (candidate) => {
          attempts += 1;
          if (attempts === 1) throw Object.assign(new Error("denied"), { code: "EPERM" });
          await rm(candidate, { recursive: true, force: false });
        },
      },
    );
    assert.equal(result.deleted, true);
    assert.deepEqual(repaired, [path.resolve(targetPath)]);
    assert.equal(attempts, 2);
  });
});

test("ACL repair is not invoked after a candidate path swap", async (t) => {
  await withTempDir(async (root) => {
    const buildRoot = path.join(root, "build");
    const commonDir = path.join(root, "repo", ".git");
    const worktreeRoot = path.join(root, "worktree");
    const outside = path.join(root, "outside");
    const targetPath = resolveAgentTarget({ buildRoot, commonDir, worktreePath: worktreeRoot });
    await mkdir(outside, { recursive: true });
    await writeTargetManifest(targetPath, { commonDir, targetPath, worktreePath: worktreeRoot });
    let repaired = false;
    try {
      await assert.rejects(() =>
        applyCleanupPlan(
          {
            apply: true,
            buildRoot,
            bytes: 1,
            commonDir,
            eligible: true,
            reasons: [],
            targetPath,
            worktreePath: worktreeRoot,
          },
          {
            platform: "win32",
            repairAcl: true,
            repairAclCandidate: async () => {
              repaired = true;
            },
            removeCandidate: async (candidate) => {
              await rm(candidate, { recursive: true, force: false });
              await import("node:fs/promises").then(({ symlink }) =>
                symlink(outside, candidate, process.platform === "win32" ? "junction" : "dir"),
              );
              throw Object.assign(new Error("denied"), { code: "EPERM" });
            },
          },
        ),
      );
    } catch (error) {
      if (error?.code === "EPERM") {
        t.diagnostic("junction swap fixture unavailable under current Windows token");
        return;
      }
      throw error;
    }
    assert.equal(repaired, false);
  });
});
