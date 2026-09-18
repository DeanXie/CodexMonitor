import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  ALLOWED_REMOTE_METHODS,
  assertCredentialCopyPaths,
  assertIsolatedAcceptancePaths,
  loadStaleDeliveryFixture,
  resolveCargoTargetDir,
  sanitizeAcceptanceEvidence,
  validateExistingWorkspaceConfig,
  validateAcceptanceEvidence,
  withTemporaryCredential,
} from "./phase-3-5-final-acceptance.mjs";

test("acceptance artifacts follow the wrapper-provided Cargo target", () => {
  assert.equal(
    resolveCargoTargetDir("F:\\repo", { CARGO_TARGET_DIR: "F:\\external\\agent-target" }),
    "F:\\external\\agent-target",
  );
  assert.equal(resolveCargoTargetDir("F:\\repo", {}), "F:\\repo\\src-tauri\\target");
});

test("acceptance RPC allowlist excludes every remote mutation", () => {
  assert.deepEqual(ALLOWED_REMOTE_METHODS, [
    "auth",
    "daemon_info",
    "list_workspaces",
    "connect_workspace",
    "list_threads",
    "read_thread",
    "thread_live_subscribe",
    "get_authoritative_observation_snapshot",
    "get_projection_freshness",
  ]);
  for (const forbidden of [
    "resume_thread",
    "respond_to_server_request",
    "delete_thread",
    "thread_upstream_unsubscribe",
    "start_thread",
    "send_user_message",
  ]) {
    assert.equal(ALLOWED_REMOTE_METHODS.includes(forbidden), false);
  }
});

test("isolated paths fail closed when any runtime path escapes the run root", () => {
  const runRoot = "F:\\repo\\src-tauri\\target\\phase-3-5-final-acceptance\\run-a";
  assert.doesNotThrow(() =>
    assertIsolatedAcceptancePaths({
      runRoot,
      codexHome: `${runRoot}\\codex-home`,
      daemonDataDir: `${runRoot}\\daemon-data`,
      workspacePath: `${runRoot}\\workspace`,
      evidencePath: `${runRoot}\\raw-evidence.json`,
    }),
  );
  assert.throws(() =>
    assertIsolatedAcceptancePaths({
      runRoot,
      codexHome: "C:\\Users\\Private\\.codex",
      daemonDataDir: `${runRoot}\\daemon-data`,
      workspacePath: `${runRoot}\\workspace`,
      evidencePath: `${runRoot}\\raw-evidence.json`,
    }),
  );
});

test("sanitized evidence drops secrets and private thread payloads", () => {
  const sanitized = sanitizeAcceptanceEvidence({
    token: "do-not-persist",
    prompt: "private prompt",
    thread: { id: "thread-sanitized", preview: "private preview", turns: [{ text: "secret" }] },
    remoteHostIdentity: "host-sanitized",
  });
  const encoded = JSON.stringify(sanitized);
  assert.equal(encoded.includes("do-not-persist"), false);
  assert.equal(encoded.includes("private prompt"), false);
  assert.equal(encoded.includes("private preview"), false);
  assert.equal(encoded.includes("secret"), false);
  assert.equal(sanitized.threadId, "thread-sanitized");
  assert.equal(sanitized.remoteHostIdentity, "host-sanitized");
});

test("evidence requires exact identity, reconnect generation change, hydration, and zero forbidden mutation", () => {
  const evidence = {
    result: "PASS",
    exactThreadIdMatch: true,
    initialTransportGeneration: "transport-a",
    reconnectedTransportGeneration: "transport-b",
    staleOldGenerationRejected: true,
    staleOldGenerationEvidence: {
      classification: "DETERMINISTIC_FIXTURE_CONTRACT",
      realTransportLateNotificationScenario: "NOT_EXECUTED",
    },
    generationTaggedEventObserved: true,
    projectionHydratedCurrent: true,
    projectionRehydratedCurrent: true,
    forbiddenMutationCounts: {
      resumeThread: 0,
      approvalDecision: 0,
      threadDelete: 0,
      upstreamUnsubscribe: 0,
      forceTakeover: 0,
    },
  };
  assert.doesNotThrow(() => validateAcceptanceEvidence(evidence));
  assert.throws(() =>
    validateAcceptanceEvidence({
      ...evidence,
      reconnectedTransportGeneration: "transport-a",
    }),
  );
  assert.throws(() =>
    validateAcceptanceEvidence({
      ...evidence,
      forbiddenMutationCounts: { ...evidence.forbiddenMutationCounts, threadDelete: 1 },
    }),
  );
});

test("credential copy is restricted to the isolated CODEX_HOME", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "phase-3-5-credential-paths-"));
  try {
    const source = path.join(root, "authorized-source", "auth.json");
    const runRoot = path.join(root, "run");
    const destination = path.join(runRoot, "codex-home", "auth.json");
    assert.doesNotThrow(() =>
      assertCredentialCopyPaths({ source, destination, runRoot, expectedSource: source }),
    );
    assert.throws(() =>
      assertCredentialCopyPaths({
        source,
        destination: path.join(root, "outside", "auth.json"),
        runRoot,
        expectedSource: source,
      }),
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("temporary credential is deleted after success and failure without reading its contents", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "phase-3-5-credential-lifecycle-"));
  try {
    const source = path.join(root, "authorized-source", "auth.json");
    const runRoot = path.join(root, "run");
    const destination = path.join(runRoot, "codex-home", "auth.json");
    await writeFile(source, "test-only-credential", { encoding: "utf8", flag: "wx" }).catch(async () => {
      await import("node:fs/promises").then(({ mkdir }) => mkdir(path.dirname(source), { recursive: true }));
      await writeFile(source, "test-only-credential", { encoding: "utf8", flag: "wx" });
    });
    await withTemporaryCredential(
      { source, destination, runRoot, expectedSource: source },
      async ({ destinationExists }) => assert.equal(destinationExists, true),
    );
    await assert.rejects(
      withTemporaryCredential(
        { source, destination, runRoot, expectedSource: source },
        async () => {
          throw new Error("fixture failure");
        },
      ),
      /fixture failure/,
    );
    await assert.rejects(readFile(destination), { code: "ENOENT" });
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("real acceptance path reuses an existing disposable Thread and performs no setup mutation", async () => {
  const source = await readFile(new URL("./phase-3-5-final-acceptance.mjs", import.meta.url), "utf8");
  assert.equal(source.includes('peer.request("thread/start"'), false);
  assert.equal(source.includes('peer.request("turn/start"'), false);
  assert.match(source, /setupMutationCounts:\s*\{\s*disposableThreadStart:\s*0,\s*disposableTurnStart:\s*0/);
});

test("reused acceptance state validates workspace config without rewriting it", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "phase-3-5-workspace-config-"));
  try {
    const workspacePath = path.join(root, "workspace");
    const configPath = path.join(root, "daemon-data", "workspaces.json");
    await mkdir(path.dirname(configPath), { recursive: true });
    await mkdir(workspacePath, { recursive: true });
    const config = [{
      id: "phase-3-5-final-disposable-workspace",
      name: "Phase 3.5 disposable acceptance",
      path: workspacePath,
      kind: "main",
      settings: {},
    }];
    await writeFile(configPath, `${JSON.stringify(config, null, 2)}\n`, "utf8");
    const before = await stat(configPath);
    await validateExistingWorkspaceConfig({
      configPath,
      workspaceId: config[0].id,
      workspacePath,
    });
    const after = await stat(configPath);
    assert.equal(after.mtimeMs, before.mtimeMs);
    await assert.rejects(
      validateExistingWorkspaceConfig({
        configPath,
        workspaceId: config[0].id,
        workspacePath: path.join(root, "wrong-workspace"),
      }),
      /does not match the isolated run/,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("stale delivery fixture proves old-generation evidence cannot reach or change current state", async () => {
  const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
  assert.equal(await loadStaleDeliveryFixture(repositoryRoot), true);
});

test("final acceptance labels stale delivery as fixture evidence rather than real transport E2E", async () => {
  const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
  const evidence = JSON.parse(
    await readFile(
      path.join(repositoryRoot, "docs", "evidence", "phase-3-5-final", "final-acceptance.json"),
      "utf8",
    ),
  );
  assert.equal(evidence.isolatedE2e.staleOldGenerationRejected, true);
  assert.equal(
    evidence.isolatedE2e.staleOldGenerationEvidence.classification,
    "DETERMINISTIC_FIXTURE_CONTRACT",
  );
  assert.equal(
    evidence.isolatedE2e.staleOldGenerationEvidence.realTransportLateNotificationScenario,
    "NOT_EXECUTED",
  );
  assert.equal(evidence.evidenceCorrection.preservesOriginalResult, true);
});
