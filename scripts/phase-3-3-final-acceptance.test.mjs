import assert from "node:assert/strict";
import test from "node:test";

import {
  buildPreparedRun,
  buildRestartCapture,
  buildSanitizedEvidence,
  classifyActiveWriterError,
  captureSingleTurnDelta,
  discoverSingleNewThread,
  mergeStageCapture,
  parseSanitizedRollout,
  recordActiveWriterEvidence,
  validateFinalizableRun,
  validateSanitizedEvidence,
} from "./phase-3-3-final-acceptance.mjs";

const session = (id = "01a11111-1111-7111-8111-111111111111") =>
  JSON.stringify({
    timestamp: "2026-09-06T01:00:00.000Z",
    type: "session_meta",
    payload: {
      id,
      session_id: id,
      cwd: "F:\\acceptance",
      source: "vscode",
      originator: "Codex Desktop",
      cli_version: "0.147.0",
      prompt: "must never be retained",
    },
  });

const turn = (id, overrides = {}) =>
  JSON.stringify({
    timestamp: "2026-09-06T01:00:01.000Z",
    type: "turn_context",
    payload: {
      turn_id: id,
      cwd: "F:\\acceptance",
      model: "gpt-5.6-terra",
      effort: "medium",
      approval_policy: "on-request",
      sandbox_policy: {
        type: "workspace-write",
        network_access: true,
        writable_roots: ["F:\\acceptance"],
      },
      prompt: "must never be retained",
      ...overrides,
    },
  });

test("sanitizes rollout to identity, lifecycle, workspace, and settings evidence", () => {
  const parsed = parseSanitizedRollout([
    session(),
    turn("01a22222-2222-7222-8222-222222222222"),
    JSON.stringify({
      timestamp: "2026-09-06T01:00:02.000Z",
      type: "event_msg",
      payload: {
        type: "task_complete",
        turn_id: "01a22222-2222-7222-8222-222222222222",
        completed_at: 1,
        duration_ms: 10,
        last_agent_message: "must never be retained",
      },
    }),
    JSON.stringify({
      type: "response_item",
      payload: { type: "message", content: "must never be retained" },
    }),
  ].join("\n"));

  assert.equal(parsed.session.threadId, "01a11111-1111-7111-8111-111111111111");
  assert.deepEqual(parsed.turns, [{
    fullTurnId: "01a22222-2222-7222-8222-222222222222",
    cwd: "F:\\acceptance",
    model: "gpt-5.6-terra",
    effort: "medium",
    approvalPolicy: "on-request",
    sandboxPolicy: "workspace-write",
    networkAccess: true,
    writableRoots: ["F:\\acceptance"],
    started: false,
    completed: true,
    interrupted: false,
    failed: false,
  }]);
  assert.equal(JSON.stringify(parsed).includes("must never be retained"), false);
});

test("discovers exactly one authoritative new main Thread and fails closed otherwise", () => {
  const first = { path: "one.jsonl", parsed: parseSanitizedRollout(session()) };
  assert.equal(discoverSingleNewThread(new Set(), [first]).threadId, first.parsed.session.threadId);
  assert.throws(() => discoverSingleNewThread(new Set(["one.jsonl"]), [first]), /expected exactly one/);
  assert.throws(
    () => discoverSingleNewThread(new Set(), [first, { path: "two.jsonl", parsed: parseSanitizedRollout(session("01a33333-3333-7333-8333-333333333333")) }]),
    /expected exactly one/,
  );
});

test("captures one exact new fullTurnId per user-confirmed stage", () => {
  const before = ["01a00000-0000-7000-8000-000000000000"];
  const after = [...before, "01a22222-2222-7222-8222-222222222222"];
  assert.equal(captureSingleTurnDelta(before, after), after[1]);
  assert.throws(() => captureSingleTurnDelta(before, before), /expected exactly one/);
  assert.throws(
    () => captureSingleTurnDelta(before, [...after, "01a33333-3333-7333-8333-333333333333"]),
    /expected exactly one/,
  );
});

test("repeated identical stage capture is idempotent and conflicting capture fails", () => {
  const initial = { captures: {} };
  const captured = mergeStageCapture(initial, "MONITOR_FIRST_TURN", { fullTurnId: "turn-a" });
  assert.deepEqual(mergeStageCapture(captured, "MONITOR_FIRST_TURN", { fullTurnId: "turn-a" }), captured);
  assert.throws(
    () => mergeStageCapture(captured, "MONITOR_FIRST_TURN", { fullTurnId: "turn-b" }),
    /conflicting capture/,
  );
});

test("normalizes the authoritative app-server active-writer rejection", () => {
  assert.equal(classifyActiveWriterError({ code: -32600, message: "thread already has an active writer" }), true);
  assert.equal(classifyActiveWriterError({ code: -32600, message: "thread not found" }), false);
});

test("records active-writer protection separately from idle CLI continuation", () => {
  const threadId = "01a11111-1111-7111-8111-111111111111";
  const run = recordActiveWriterEvidence({ thread: { fullThreadId: threadId }, captures: {} }, {
    errorCode: -32600,
    message: "thread already has an active writer",
    producerSurface: "DESKTOP",
    consumerSurface: "CLI",
    consumerVersion: "codex-cli 0.153.4",
  });
  assert.deepEqual(run.captures.ACTIVE_WRITER_PROTECTION, {
    result: "BLOCKED_BY_ACTIVE_WRITER",
    fullThreadId: threadId,
    errorCode: -32600,
    errorClass: "ACTIVE_WRITER",
    producerSurface: "DESKTOP",
    consumerSurface: "CLI",
    consumerVersion: "codex-cli 0.153.4",
    provenance: "USER_REPORTED_EXACT_ID_RESUME",
  });
  assert.equal(run.captures.CLI_IDLE_CONTINUATION, undefined);
});

test("prepared run records environment versions without claiming a Thread", () => {
  const run = buildPreparedRun({
    runId: "run-a",
    codexHome: "C:\\codex-home",
    workspaceRoot: "F:\\workspace",
    baselineRolloutPaths: ["rollout-a"],
    codexCliVersion: "codex-cli 0.147.0",
    nodeVersion: "v24.16.0",
    createdAt: "2026-09-06T00:00:00.000Z",
  });
  assert.equal(run.environment.codexCliVersion, "codex-cli 0.147.0");
  assert.equal(run.environment.nodeVersion, "v24.16.0");
  assert.equal(run.thread, null);
  assert.equal(run.state, "PREPARED");
});

test("restart capture keeps backend exact-ID evidence separate from user-observed Monitor reconstruction", () => {
  assert.deepEqual(buildRestartCapture("01a11111-1111-7111-8111-111111111111", {
    result: { thread: { id: "01a11111-1111-7111-8111-111111111111" } },
  }), {
    result: "SUCCESS",
    fullThreadId: "01a11111-1111-7111-8111-111111111111",
    method: "thread/read",
    monitorUiReconstruction: "USER_CONFIRMED",
    priorSurfaceHistoryVisible: ["MONITOR", "DESKTOP", "CLI"],
  });
});

test("finalization requires separate idle continuation and active-writer evidence", () => {
  const threadId = "01a11111-1111-7111-8111-111111111111";
  const run = {
    thread: { fullThreadId: threadId },
    captures: {
      MONITOR_FIRST_TURN: { fullTurnId: "01a22222-2222-7222-8222-222222222222", completed: true },
      DESKTOP_IDLE_CONTINUATION: { fullTurnId: "01a33333-3333-7333-8333-333333333333", completed: true },
      CLI_IDLE_CONTINUATION: { fullTurnId: "01a44444-4444-7444-8444-444444444444", completed: true },
      ACTIVE_WRITER_PROTECTION: { result: "BLOCKED_BY_ACTIVE_WRITER" },
      POST_RELEASE_EXACT_ID: { result: "SUCCESS", fullThreadId: threadId },
      RESTART_RECONSTRUCTION: { result: "SUCCESS", fullThreadId: threadId },
    },
  };
  assert.doesNotThrow(() => validateFinalizableRun(run));
  const missingOccupancy = structuredClone(run);
  delete missingOccupancy.captures.ACTIVE_WRITER_PROTECTION;
  assert.throws(() => validateFinalizableRun(missingOccupancy), /ACTIVE_WRITER_PROTECTION/);
});

test("final evidence excludes raw paths and keeps independent acceptance facts", () => {
  const threadId = "01a11111-1111-7111-8111-111111111111";
  const completeRun = {
    runId: "private-run-name",
    createdAt: "2026-09-06T00:00:00.000Z",
    codexHome: "C:\\Users\\Private\\.codex",
    workspaceRoot: "F:\\workspace",
    environment: { codexCliVersion: "codex-cli 0.153.3", nodeVersion: "v24" },
    thread: {
      fullThreadId: threadId,
      cwd: "F:\\workspace",
      source: "vscode",
      originator: "codex_monitor",
      rolloutPaths: ["C:\\Users\\Private\\.codex\\sessions\\rollout.jsonl"],
    },
    captures: {
      MONITOR_FIRST_TURN: { fullTurnId: "01a22222-2222-7222-8222-222222222222", completed: true, cwd: "F:\\workspace" },
      DESKTOP_IDLE_CONTINUATION: { fullTurnId: "01a33333-3333-7333-8333-333333333333", completed: true, cwd: "F:\\workspace" },
      CLI_IDLE_CONTINUATION: { fullTurnId: "01a44444-4444-7444-8444-444444444444", completed: true, cwd: "F:\\workspace" },
      ACTIVE_WRITER_PROTECTION: { result: "BLOCKED_BY_ACTIVE_WRITER", fullThreadId: threadId },
      POST_RELEASE_EXACT_ID: { result: "SUCCESS", fullThreadId: threadId },
      RESTART_RECONSTRUCTION: {
        result: "SUCCESS",
        fullThreadId: threadId,
        monitorUiReconstruction: "USER_CONFIRMED",
        priorSurfaceHistoryVisible: ["MONITOR", "DESKTOP", "CLI"],
      },
    },
    guarantees: { correlation: "EXACT_IDS_AND_EXPLICIT_STAGE_BOUNDARIES_ONLY" },
  };
  const evidence = buildSanitizedEvidence(completeRun, "codex-cli 0.153.4");
  const serialized = JSON.stringify(evidence);
  assert.equal(serialized.includes("C:\\Users\\Private"), false);
  assert.equal(serialized.includes("rollout.jsonl"), false);
  assert.equal(evidence.creation.threadAcknowledged, "CONFIRMED");
  assert.equal(evidence.creation.persistenceConfirmed, "CONFIRMED");
  assert.equal(evidence.creation.firstTurnOutcome, "COMPLETED");
  assert.equal(evidence.creation.ephemeral, "UNKNOWN");
  assert.equal(evidence.identity.duplicateCanonicalThreadCount, 0);
  assert.equal(evidence.workspace.origin.workspacePath, "<workspace>");
  assert.doesNotThrow(() => validateSanitizedEvidence(evidence));
  const invalid = structuredClone(evidence);
  invalid.occupancy.result = "PASS";
  assert.throws(() => validateSanitizedEvidence(invalid), /occupancy protection/);
});
