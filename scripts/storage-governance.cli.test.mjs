import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdtemp, mkdir, readFile, rm, stat, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const cli = path.join(projectRoot, "scripts", "storage-governance.mjs");

function git(cwd, ...args) {
  return execFileSync("git", args, { cwd, encoding: "utf8" }).trim();
}

function runCli(cwd, buildRoot, ...args) {
  return spawnSync(process.execPath, [cli, ...args], {
    cwd,
    encoding: "utf8",
    env: { ...process.env, CODEXMONITOR_CARGO_TARGET_ROOT: buildRoot },
  });
}

async function fixtureRepository(fn) {
  const root = await mkdtemp(path.join(os.tmpdir(), "codexmonitor-storage-cli-"));
  const repo = path.join(root, "repo");
  const worktree = path.join(root, "worktree");
  const buildRoot = path.join(root, "external-build");
  try {
    await mkdir(repo);
    git(repo, "init", "-b", "main");
    git(repo, "config", "user.name", "Storage Test");
    git(repo, "config", "user.email", "storage-test@example.invalid");
    await writeFile(path.join(repo, "README.md"), "fixture\n");
    git(repo, "add", "README.md");
    git(repo, "commit", "-m", "fixture");
    git(repo, "worktree", "add", worktree, "-b", "feature");
    return await fn({ buildRoot, repo, root, worktree });
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

test("prepare resolves an external target and closeout preserves the linked Worktree", async () => {
  await fixtureRepository(async ({ buildRoot, repo, worktree }) => {
    const preparedRun = runCli(worktree, buildRoot, "prepare", "--allow-low-space", "--json");
    assert.equal(preparedRun.status, 0, preparedRun.stderr);
    const prepared = JSON.parse(preparedRun.stdout);
    assert.equal(prepared.isLinkedWorktree, true);
    assert.equal(path.dirname(prepared.targetPath), path.join(buildRoot, "agents"));
    assert.equal(prepared.environment.CARGO_INCREMENTAL, "0");
    assert.equal(prepared.environment.CARGO_TARGET_DIR, prepared.targetPath);
    assert.equal(JSON.parse(await readFile(path.join(prepared.targetPath, ".codexmonitor-target.json"), "utf8")).schemaVersion, 1);

    const report = JSON.parse(runCli(repo, buildRoot, "report", "--json").stdout);
    assert.equal(report.entries.some((entry) => entry.status === "LEGACY-IN-TREE"), false);
    const reportTarget = report.entries.find((entry) => entry.targetPath === prepared.targetPath);
    assert.equal(reportTarget.targetClass, "CLOSEOUT-ELIGIBLE");

    const dryRun = runCli(worktree, buildRoot, "closeout", "--accepted", "--json");
    assert.equal(dryRun.status, 0, dryRun.stderr);
    const dryRunPayload = JSON.parse(dryRun.stdout);
    assert.equal(dryRunPayload.targetClass, reportTarget.targetClass);
    assert.equal(dryRunPayload.eligible, true);
    assert.ok((await stat(prepared.targetPath)).isDirectory());

    const appliedRun = runCli(worktree, buildRoot, "closeout", "--accepted", "--apply", "--json");
    assert.equal(appliedRun.status, 0, appliedRun.stderr);
    assert.equal(JSON.parse(appliedRun.stdout).deleted, true);
    await assert.rejects(() => stat(prepared.targetPath), { code: "ENOENT" });
    assert.ok((await stat(worktree)).isDirectory());
    assert.match(git(repo, "branch", "--list", "feature"), /^\+?\s*feature$/);
  });
});

test("guard fails closed on simulated low space and records an explicit override", async () => {
  await fixtureRepository(async ({ buildRoot, worktree }) => {
    const blocked = spawnSync(process.execPath, [cli, "guard", "--json", "--simulate-free-bytes", "1"], {
      cwd: worktree,
      encoding: "utf8",
      env: {
        ...process.env,
        CODEXMONITOR_CARGO_TARGET_ROOT: buildRoot,
        CODEXMONITOR_STORAGE_TEST_MODE: "1",
      },
    });
    assert.equal(blocked.status, 2);
    const blockedPayload = JSON.parse(blocked.stdout);
    assert.equal(blockedPayload.blocked, true);
    assert.equal(blockedPayload.targetRoot, path.resolve(buildRoot));
    assert.equal(blockedPayload.budget.reasons[0].code, "FREE_BLOCK");

    const overridden = spawnSync(
      process.execPath,
      [cli, "guard", "--", "--json", "--simulate-free-bytes", "1", "--allow-low-space"],
      {
        cwd: worktree,
        encoding: "utf8",
        env: {
          ...process.env,
          CODEXMONITOR_CARGO_TARGET_ROOT: buildRoot,
          CODEXMONITOR_STORAGE_TEST_MODE: "1",
        },
      },
    );
    assert.equal(overridden.status, 0, overridden.stderr);
    assert.equal(JSON.parse(overridden.stdout).budget.overrideApplied, true);
  });
});

test("report labels an under-TTL orphan without selecting it for deletion", async () => {
  await fixtureRepository(async ({ buildRoot, repo, worktree }) => {
    const prepared = JSON.parse(runCli(worktree, buildRoot, "prepare", "--json").stdout);
    git(repo, "worktree", "remove", "--force", worktree);
    const reportRun = runCli(repo, buildRoot, "report", "--json");
    assert.equal(reportRun.status, 0, reportRun.stderr);
    const orphan = JSON.parse(reportRun.stdout).entries.find((entry) => entry.targetPath === prepared.targetPath);
    assert.equal(orphan.status, "ORPHAN");
    assert.equal(orphan.cleanupEligible, false);

    const cleanupRun = runCli(repo, buildRoot, "clean-agents", "--apply", "--json");
    assert.equal(cleanupRun.status, 0, cleanupRun.stderr);
    const cleanupEntry = JSON.parse(cleanupRun.stdout).entries.find((entry) => entry.targetPath === prepared.targetPath);
    assert.equal(cleanupEntry.selected, false);
    assert.ok((await stat(prepared.targetPath)).isDirectory());
  });
});
