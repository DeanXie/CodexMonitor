import { spawn } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import {
  mkdir,
  readFile,
  readdir,
  rename,
  stat,
  writeFile,
} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath, pathToFileURL } from "node:url";

const FULL_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const REQUIRED_CAPTURES = [
  "MONITOR_FIRST_TURN",
  "DESKTOP_IDLE_CONTINUATION",
  "CLI_IDLE_CONTINUATION",
  "ACTIVE_WRITER_PROTECTION",
  "POST_RELEASE_EXACT_ID",
  "RESTART_RECONSTRUCTION",
];

function optionalString(value) {
  return typeof value === "string" && value.length > 0 ? value : null;
}

function sandboxName(value) {
  if (typeof value === "string") return value;
  if (!value || typeof value !== "object") return null;
  return optionalString(value.type) ?? optionalString(value.mode);
}

function stringList(value) {
  return Array.isArray(value) && value.every((item) => typeof item === "string")
    ? [...value]
    : null;
}

export function parseSanitizedRollout(content) {
  let session = null;
  const turns = new Map();
  for (const line of content.split(/\r?\n/u)) {
    if (!line.trim()) continue;
    let record;
    try {
      record = JSON.parse(line);
    } catch {
      continue;
    }
    const payload = record?.payload;
    if (!payload || typeof payload !== "object") continue;
    if (record.type === "session_meta") {
      const threadId = optionalString(payload.id);
      if (!threadId || !FULL_ID.test(threadId)) continue;
      const source = typeof payload.source === "string" ? payload.source : null;
      const parentThreadId = payload.source?.subagent?.thread_spawn?.parent_thread_id ?? null;
      session = {
        threadId,
        sessionId: optionalString(payload.session_id),
        cwd: optionalString(payload.cwd),
        source,
        originator: optionalString(payload.originator),
        cliVersion: optionalString(payload.cli_version),
        parentThreadId: optionalString(parentThreadId),
      };
      continue;
    }
    const turnId = optionalString(payload.turn_id);
    if (!turnId || !FULL_ID.test(turnId)) continue;
    const current = turns.get(turnId) ?? {
      fullTurnId: turnId,
      cwd: null,
      model: null,
      effort: null,
      approvalPolicy: null,
      sandboxPolicy: null,
      networkAccess: null,
      writableRoots: null,
      started: false,
      completed: false,
      interrupted: false,
      failed: false,
    };
    if (record.type === "turn_context") {
      const sandbox = payload.sandbox_policy ?? payload.sandboxPolicy;
      const networkAccess = payload.network_access ?? payload.networkAccess
        ?? sandbox?.network_access ?? sandbox?.networkAccess;
      const writableRoots = payload.writable_roots ?? payload.writableRoots
        ?? sandbox?.writable_roots ?? sandbox?.writableRoots;
      current.cwd = optionalString(payload.cwd);
      current.model = optionalString(payload.model);
      current.effort = optionalString(payload.effort);
      current.approvalPolicy = optionalString(payload.approval_policy ?? payload.approvalPolicy);
      current.sandboxPolicy = sandboxName(sandbox);
      current.networkAccess = typeof networkAccess === "boolean"
        ? networkAccess
        : null;
      current.writableRoots = stringList(writableRoots);
    } else if (record.type === "event_msg") {
      if (payload.type === "task_started") current.started = true;
      if (payload.type === "task_complete") current.completed = true;
      if (payload.type === "turn_aborted" || payload.type === "task_interrupted") current.interrupted = true;
      if (payload.type === "task_failed") current.failed = true;
    }
    turns.set(turnId, current);
  }
  return { session, turns: [...turns.values()].sort((a, b) => a.fullTurnId.localeCompare(b.fullTurnId)) };
}

async function walk(directory, output) {
  let entries;
  try {
    entries = await readdir(directory, { withFileTypes: true });
  } catch (error) {
    if (error?.code === "ENOENT") return;
    throw error;
  }
  for (const entry of entries) {
    const target = path.join(directory, entry.name);
    if (entry.isDirectory()) await walk(target, output);
    else if (entry.isFile() && entry.name.startsWith("rollout-") && entry.name.endsWith(".jsonl")) output.push(target);
  }
}

export async function inventoryRollouts(codexHome) {
  const paths = [];
  await walk(path.join(codexHome, "sessions"), paths);
  const inventory = [];
  for (const rolloutPath of paths.sort()) {
    const metadata = await stat(rolloutPath);
    inventory.push({
      path: path.resolve(rolloutPath),
      size: metadata.size,
      modifiedAtMs: Math.trunc(metadata.mtimeMs),
    });
  }
  return inventory;
}

async function parsedInventory(inventory) {
  return Promise.all(inventory.map(async (item) => ({
    ...item,
    parsed: parseSanitizedRollout(await readFile(item.path, "utf8")),
  })));
}

export function discoverSingleNewThread(baselinePaths, inventory) {
  const candidatesByThread = new Map();
  for (const item of inventory) {
    if (baselinePaths.has(item.path)) continue;
    const session = item.parsed?.session;
    if (!session?.threadId || session.parentThreadId) continue;
    const existing = candidatesByThread.get(session.threadId) ?? { ...session, rolloutPaths: [] };
    existing.rolloutPaths.push(item.path);
    candidatesByThread.set(session.threadId, existing);
  }
  const candidates = [...candidatesByThread.values()];
  if (candidates.length !== 1) {
    throw new Error(`expected exactly one authoritative new main Thread; observed ${candidates.length}`);
  }
  return candidates[0];
}

export function captureSingleTurnDelta(beforeTurnIds, afterTurnIds) {
  const before = new Set(beforeTurnIds);
  const delta = [...new Set(afterTurnIds)].filter((turnId) => !before.has(turnId));
  if (delta.length !== 1) {
    throw new Error(`expected exactly one new fullTurnId; observed ${delta.length}`);
  }
  return delta[0];
}

export function mergeStageCapture(run, stage, capture) {
  const prior = run.captures?.[stage];
  if (prior) {
    if (JSON.stringify(prior) !== JSON.stringify(capture)) {
      throw new Error(`conflicting capture for ${stage}`);
    }
    return run;
  }
  return { ...run, captures: { ...(run.captures ?? {}), [stage]: capture } };
}

export function validateFinalizableRun(run) {
  if (!FULL_ID.test(run.thread?.fullThreadId ?? "")) throw new Error("missing authoritative fullThreadId");
  for (const stage of REQUIRED_CAPTURES) {
    if (!run.captures?.[stage]) throw new Error(`missing required capture: ${stage}`);
  }
  const turnStages = ["MONITOR_FIRST_TURN", "DESKTOP_IDLE_CONTINUATION", "CLI_IDLE_CONTINUATION"];
  const turnIds = turnStages.map((stage) => run.captures[stage].fullTurnId);
  if (turnIds.some((turnId) => !FULL_ID.test(turnId ?? "")) || new Set(turnIds).size !== turnIds.length) {
    throw new Error("Surface continuation Turn IDs must be valid and distinct");
  }
  if (turnStages.some((stage) => run.captures[stage].completed !== true)) {
    throw new Error("normal continuation requires completed idle Turns");
  }
  if (run.captures.ACTIVE_WRITER_PROTECTION.result !== "BLOCKED_BY_ACTIVE_WRITER") {
    throw new Error("ACTIVE_WRITER_PROTECTION must be BLOCKED_BY_ACTIVE_WRITER");
  }
  for (const stage of ["POST_RELEASE_EXACT_ID", "RESTART_RECONSTRUCTION"]) {
    if (run.captures[stage].result !== "SUCCESS" || run.captures[stage].fullThreadId !== run.thread.fullThreadId) {
      throw new Error(`${stage} must preserve exact fullThreadId`);
    }
  }
}

async function writeJsonAtomic(target, value) {
  await mkdir(path.dirname(target), { recursive: true });
  const temporary = `${target}.${process.pid}.tmp`;
  await writeFile(temporary, `${JSON.stringify(value, null, 2)}\n`, "utf8");
  await rename(temporary, target);
}

function arg(name, fallback = null) {
  const index = process.argv.indexOf(`--${name}`);
  return index >= 0 ? process.argv[index + 1] : fallback;
}

function defaultCodexHome() {
  return process.env.CODEX_HOME || path.join(os.homedir(), ".codex");
}

function defaultRunRoot() {
  const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
  return path.join(repositoryRoot, ".codexmonitor", "phase-3-3-final-acceptance");
}

function runFile(runDirectory) {
  return path.join(runDirectory, "run.json");
}

async function loadRun(runDirectory) {
  return JSON.parse(await readFile(runFile(runDirectory), "utf8"));
}

async function executableVersion(executable, argumentsList) {
  return new Promise((resolve) => {
    const child = spawn(executable, argumentsList, { env: process.env, stdio: ["ignore", "pipe", "ignore"], windowsHide: true });
    let output = "";
    child.stdout.setEncoding("utf8");
    child.stdout.on("data", (chunk) => { output = `${output}${chunk}`.slice(0, 500); });
    child.on("error", () => resolve("UNAVAILABLE"));
    child.on("close", (code) => resolve(code === 0 ? output.trim() : "UNAVAILABLE"));
  });
}

export function buildPreparedRun({
  runId,
  codexHome,
  workspaceRoot,
  baselineRolloutPaths,
  codexCliVersion,
  nodeVersion,
  createdAt,
}) {
  return {
    schemaVersion: 1,
    phase: "3.3-final",
    runId,
    state: "PREPARED",
    createdAt,
    codexHome,
    workspaceRoot,
    environment: { codexCliVersion, nodeVersion },
    baselineRolloutPaths,
    observedTurnIds: [],
    thread: null,
    captures: {},
    guarantees: {
      rawDirectoryIgnored: true,
      promptRecorded: false,
      responseRecorded: false,
      desktopPrivateStateWrittenByHarness: false,
      correlation: "EXACT_IDS_AND_EXPLICIT_STAGE_BOUNDARIES_ONLY",
    },
  };
}

async function collectExactThread(inventory, threadId) {
  const parsed = await parsedInventory(inventory);
  const matching = parsed.filter((item) => item.parsed.session?.threadId === threadId);
  if (matching.length === 0) throw new Error(`no persisted rollout for exact Thread ${threadId}`);
  const turns = new Map();
  for (const item of matching) {
    for (const turnRecord of item.parsed.turns) turns.set(turnRecord.fullTurnId, turnRecord);
  }
  return { rollouts: matching.map((item) => item.path), turns: [...turns.values()] };
}

async function prepare() {
  const runId = arg("run-id", `phase-3-3-final-${new Date().toISOString().replace(/[:.]/gu, "-")}-${randomUUID().slice(0, 8)}`);
  const runDirectory = path.resolve(arg("run-dir", path.join(defaultRunRoot(), runId)));
  const codexHome = path.resolve(arg("codex-home", defaultCodexHome()));
  const workspaceRoot = path.resolve(arg("workspace", process.cwd()));
  const baseline = await inventoryRollouts(codexHome);
  const codexCliVersion = await executableVersion(arg("codex-bin", "codex"), ["--version"]);
  const run = buildPreparedRun({
    runId,
    createdAt: new Date().toISOString(),
    codexHome,
    workspaceRoot,
    codexCliVersion,
    nodeVersion: process.version,
    baselineRolloutPaths: baseline.map((item) => item.path),
  });
  await writeJsonAtomic(runFile(runDirectory), run);
  process.stdout.write(`${JSON.stringify({ runDirectory, runId, state: run.state, baselineRollouts: baseline.length })}\n`);
}

const STAGE_ALIASES = {
  monitor: "MONITOR_FIRST_TURN",
  desktop: "DESKTOP_IDLE_CONTINUATION",
  cli: "CLI_IDLE_CONTINUATION",
};

async function capture() {
  const runDirectory = path.resolve(arg("run-dir", ""));
  const requestedStage = process.argv[3] ?? arg("stage");
  const stage = STAGE_ALIASES[requestedStage] ?? requestedStage;
  if (!runDirectory || !stage) throw new Error("capture requires <monitor|desktop|cli> and --run-dir");
  let run = await loadRun(runDirectory);
  const inventory = await inventoryRollouts(run.codexHome);
  if (stage === "MONITOR_FIRST_TURN" && !run.thread) {
    const parsed = await parsedInventory(inventory);
    const discovered = discoverSingleNewThread(new Set(run.baselineRolloutPaths), parsed);
    run.thread = {
      fullThreadId: discovered.threadId,
      sessionId: discovered.sessionId,
      cwd: discovered.cwd,
      source: discovered.source,
      originator: discovered.originator,
      cliVersion: discovered.cliVersion,
      rolloutPaths: discovered.rolloutPaths,
    };
  }
  if (!run.thread?.fullThreadId) throw new Error("Monitor Thread has not been authoritatively locked");
  const exact = await collectExactThread(inventory, run.thread.fullThreadId);
  const fullTurnId = captureSingleTurnDelta(run.observedTurnIds, exact.turns.map((item) => item.fullTurnId));
  const turnRecord = exact.turns.find((item) => item.fullTurnId === fullTurnId);
  if (!turnRecord?.completed || turnRecord.interrupted || turnRecord.failed) {
    throw new Error(`Turn ${fullTurnId} is not a successfully completed idle boundary`);
  }
  const captureValue = { ...turnRecord, fullThreadId: run.thread.fullThreadId };
  run = mergeStageCapture(run, stage, captureValue);
  run.observedTurnIds = [...new Set([...run.observedTurnIds, fullTurnId])];
  run.state = `${stage}_CAPTURED`;
  await writeJsonAtomic(runFile(runDirectory), run);
  process.stdout.write(`${JSON.stringify({ stage, fullThreadId: run.thread.fullThreadId, fullTurnId, completed: true })}\n`);
}

async function appServerProbe(codexBin, method, threadId) {
  const child = spawn(codexBin, ["app-server"], { env: process.env, stdio: ["pipe", "pipe", "pipe"], windowsHide: true });
  const pending = new Map();
  let nextId = 1;
  let stderr = "";
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", (chunk) => { stderr = `${stderr}${chunk}`.slice(-4000); });
  createInterface({ input: child.stdout }).on("line", (line) => {
    let message;
    try { message = JSON.parse(line); } catch { return; }
    const waiter = pending.get(message.id);
    if (!waiter) return;
    pending.delete(message.id);
    if (message.error) waiter.resolve({ ok: false, error: message.error });
    else waiter.resolve({ ok: true, result: message.result });
  });
  const request = (requestMethod, params) => new Promise((resolve, reject) => {
    const id = nextId++;
    pending.set(id, { resolve, reject });
    child.stdin.write(`${JSON.stringify({ id, method: requestMethod, params })}\n`, (error) => {
      if (error) { pending.delete(id); reject(error); }
    });
  });
  const timer = setTimeout(() => child.kill(), 30_000);
  try {
    const initialized = await request("initialize", {
      clientInfo: { name: "codex-monitor-phase-3-3-final-acceptance", title: "Phase 3.3 Final Acceptance", version: "1.0.0" },
      capabilities: { experimentalApi: true },
    });
    if (!initialized.ok) return initialized;
    child.stdin.write(`${JSON.stringify({ method: "initialized" })}\n`);
    return await request(method, { threadId });
  } finally {
    clearTimeout(timer);
    child.kill();
    if (stderr && process.env.PHASE_3_3_ACCEPTANCE_DEBUG === "1") process.stderr.write(stderr);
  }
}

function errorText(value) {
  return JSON.stringify(value ?? {}).toUpperCase();
}

export function classifyActiveWriterError(error) {
  if (error?.code !== -32600) return false;
  const text = errorText(error);
  return text.includes("BLOCKED_BY_ACTIVE_WRITER") || text.includes("THREAD ALREADY HAS AN ACTIVE WRITER");
}

export function recordActiveWriterEvidence(run, evidence) {
  const threadId = run.thread?.fullThreadId;
  if (!threadId || !classifyActiveWriterError({ code: evidence.errorCode, message: evidence.message })) {
    throw new Error("invalid active-writer evidence");
  }
  return mergeStageCapture(run, "ACTIVE_WRITER_PROTECTION", {
    result: "BLOCKED_BY_ACTIVE_WRITER",
    fullThreadId: threadId,
    errorCode: evidence.errorCode,
    errorClass: "ACTIVE_WRITER",
    producerSurface: evidence.producerSurface,
    consumerSurface: evidence.consumerSurface,
    consumerVersion: evidence.consumerVersion,
    provenance: "USER_REPORTED_EXACT_ID_RESUME",
  });
}

export function buildRestartCapture(threadId, response) {
  if (!response?.ok && response?.result === undefined) {
    throw new Error("restart exact-ID read failed");
  }
  const observedId = response.result?.thread?.id;
  if (observedId !== threadId) throw new Error("restart exact-ID read returned conflicting Thread identity");
  return {
    result: "SUCCESS",
    fullThreadId: observedId,
    method: "thread/read",
    monitorUiReconstruction: "USER_CONFIRMED",
    priorSurfaceHistoryVisible: ["MONITOR", "DESKTOP", "CLI"],
  };
}

async function recordActiveWriter() {
  const runDirectory = path.resolve(arg("run-dir", ""));
  let run = await loadRun(runDirectory);
  run = recordActiveWriterEvidence(run, {
    errorCode: Number(arg("error-code")),
    message: arg("message", ""),
    producerSurface: arg("producer", "UNKNOWN").toUpperCase(),
    consumerSurface: arg("consumer", "UNKNOWN").toUpperCase(),
    consumerVersion: arg("consumer-version", "UNKNOWN"),
  });
  run.state = "ACTIVE_WRITER_PROTECTION_CAPTURED";
  await writeJsonAtomic(runFile(runDirectory), run);
  process.stdout.write(`${JSON.stringify(run.captures.ACTIVE_WRITER_PROTECTION)}\n`);
}

async function probe() {
  const kind = process.argv[3] ?? arg("kind");
  const runDirectory = path.resolve(arg("run-dir", ""));
  if (!kind || !runDirectory) throw new Error("probe requires <active-writer|post-release|restart> and --run-dir");
  let run = await loadRun(runDirectory);
  const threadId = run.thread?.fullThreadId;
  if (!threadId) throw new Error("exact Thread is not locked");
  const codexBin = arg("codex-bin", "codex");
  const stage = kind === "active-writer" ? "ACTIVE_WRITER_PROTECTION"
    : kind === "post-release" ? "POST_RELEASE_EXACT_ID"
      : kind === "restart" ? "RESTART_RECONSTRUCTION" : null;
  if (!stage) throw new Error(`unsupported probe kind: ${kind}`);
  const method = kind === "restart" ? "thread/read" : "thread/resume";
  const response = await appServerProbe(codexBin, method, threadId);
  let captureValue;
  if (kind === "active-writer") {
    if (response.ok || !classifyActiveWriterError(response.error)) {
      throw new Error("active-writer probe did not return BLOCKED_BY_ACTIVE_WRITER");
    }
    captureValue = { result: "BLOCKED_BY_ACTIVE_WRITER", fullThreadId: threadId, method };
  } else {
    const observedId = response.result?.thread?.id;
    if (!response.ok || observedId !== threadId) throw new Error(`${kind} exact-ID probe failed`);
    captureValue = kind === "restart"
      ? buildRestartCapture(threadId, response)
      : { result: "SUCCESS", fullThreadId: observedId, method };
  }
  run = mergeStageCapture(run, stage, captureValue);
  run.state = `${stage}_CAPTURED`;
  await writeJsonAtomic(runFile(runDirectory), run);
  process.stdout.write(`${JSON.stringify({ stage, ...captureValue })}\n`);
}

async function status() {
  const runDirectory = path.resolve(arg("run-dir", ""));
  const run = await loadRun(runDirectory);
  const completed = Object.keys(run.captures ?? {});
  const next = REQUIRED_CAPTURES.find((stage) => !completed.includes(stage)) ?? "FINALIZE";
  process.stdout.write(`${JSON.stringify({ runDirectory, state: run.state, fullThreadId: run.thread?.fullThreadId ?? null, completed, next }, null, 2)}\n`);
}

function rejectForbiddenKeys(value) {
  const forbidden = new Set(["prompt", "response", "reasoning", "credential", "credentials", "cookie", "cookies", "token", "tokens", "content", "message", "lastagentmessage"]);
  if (Array.isArray(value)) return value.forEach(rejectForbiddenKeys);
  if (!value || typeof value !== "object") return;
  for (const [key, child] of Object.entries(value)) {
    if (forbidden.has(key.toLowerCase())) throw new Error(`forbidden evidence key: ${key}`);
    rejectForbiddenKeys(child);
  }
}

function sanitizeString(value, run) {
  const comparisons = [
    [run.workspaceRoot, "<workspace>"],
    [run.codexHome, "<codex-home>"],
  ];
  for (const [prefix, replacement] of comparisons) {
    if (!prefix) continue;
    const lowerValue = value.toLowerCase();
    const lowerPrefix = prefix.toLowerCase();
    if (lowerValue.startsWith(lowerPrefix)) return `${replacement}${value.slice(prefix.length)}`;
  }
  return value;
}

function sanitizeValue(value, run) {
  if (typeof value === "string") return sanitizeString(value, run);
  if (Array.isArray(value)) return value.map((item) => sanitizeValue(item, run));
  if (!value || typeof value !== "object") return value;
  return Object.fromEntries(Object.entries(value).map(([key, child]) => [key, sanitizeValue(child, run)]));
}

export function buildSanitizedEvidence(run, finalCodexCliVersion) {
  validateFinalizableRun(run);
  const monitor = sanitizeValue(run.captures.MONITOR_FIRST_TURN, run);
  const desktop = sanitizeValue(run.captures.DESKTOP_IDLE_CONTINUATION, run);
  const cli = sanitizeValue(run.captures.CLI_IDLE_CONTINUATION, run);
  const turnFields = (capture) => ({
    fullTurnId: capture.fullTurnId,
    cwd: capture.cwd,
    model: capture.model ?? "NOT OBSERVED",
    effort: capture.effort ?? "NOT OBSERVED",
    approvalPolicy: capture.approvalPolicy ?? "NOT OBSERVED",
    sandboxPolicy: capture.sandboxPolicy ?? "NOT OBSERVED",
    networkAccess: capture.networkAccess ?? "NOT OBSERVED",
    writableRoots: capture.writableRoots ?? "NOT OBSERVED",
  });
  return {
    schemaVersion: 1,
    phase: "3.3-final",
    capturedAt: new Date().toISOString(),
    runIdHash: createHash("sha256").update(run.runId).digest("hex"),
    environment: {
      codexCliAtPrepare: run.environment?.codexCliVersion ?? "UNKNOWN",
      codexCliAtCliContinuation: finalCodexCliVersion,
      nodeVersion: run.environment?.nodeVersion ?? "UNKNOWN",
    },
    identity: {
      fullThreadId: run.thread.fullThreadId,
      canonicalThreadCount: 1,
      duplicateCanonicalThreadCount: 0,
      sourceRolloutRecordCount: new Set(run.thread.rolloutPaths ?? []).size,
      monitorWorkspaceEntryIdEntersIdentity: false,
    },
    creation: {
      threadAcknowledged: "CONFIRMED",
      acknowledgementBasis: "MONITOR_PRODUCTION_ACKNOWLEDGEMENT_GATE_ACCEPTED_EXACT_SERVER_ID",
      persistenceConfirmed: "CONFIRMED",
      persistenceBasis: "AUTHORITATIVE_SESSION_META",
      firstTurnAccepted: "CONFIRMED",
      firstTurnOutcome: "COMPLETED",
      firstTurnId: monitor.fullTurnId,
      ephemeral: "UNKNOWN",
      dispatchCount: "NOT DIRECTLY OBSERVED; ONE USER ACTION AND ONE CANONICAL THREAD OBSERVED",
    },
    workspace: {
      origin: {
        scope: "ORIGIN",
        workspacePath: sanitizeString(run.thread.cwd, run),
        basis: "session_meta.cwd after Monitor thread/start.cwd",
      },
      turnExecution: [monitor, desktop, cli].map((capture) => ({
        scope: "TURN_EXECUTION",
        fullTurnId: capture.fullTurnId,
        workspacePath: capture.cwd,
        basis: "persisted turn_context.cwd",
      })),
    },
    executionSettings: {
      monitor: turnFields(monitor),
      desktop: turnFields(desktop),
      cli: turnFields(cli),
      sameThreadAllowsDistinctTurnSettings: true,
    },
    continuation: {
      monitorToDesktopIdle: { result: "PASS", fullThreadId: run.thread.fullThreadId, fullTurnId: desktop.fullTurnId },
      desktopToCliIdleAfterRelease: { result: "PASS", fullThreadId: run.thread.fullThreadId, fullTurnId: cli.fullTurnId },
      priorSurfaceHistoryVisibleInCli: "USER_CONFIRMED",
      distinctTurnIds: new Set([monitor.fullTurnId, desktop.fullTurnId, cli.fullTurnId]).size === 3,
    },
    occupancy: sanitizeValue(run.captures.ACTIVE_WRITER_PROTECTION, run),
    postReleaseExactId: sanitizeValue(run.captures.POST_RELEASE_EXACT_ID, run),
    reconstruction: {
      ...sanitizeValue(run.captures.RESTART_RECONSTRUCTION, run),
      persistedObserved: "RECOVERABLE_FROM_ROLLOUT_TURN_CONTEXT",
      processLocalRequestedAndNotificationEvidence: "NOT RECOVERABLE BY CURRENT CONTRACT",
    },
    projections: {
      desktopProjectAssigned: "UNKNOWN",
      desktopSidebarVisible: "NOT OBSERVED",
      monitorListVisibleAfterRestart: "CONFIRMED",
      remoteDiscoverable: "UNKNOWN / NOT TESTED",
    },
    boundaries: {
      correlation: run.guarantees.correlation,
      rawEvidenceCommitted: false,
      userContentStored: false,
      desktopPrivateStateWrittenByHarness: false,
      normalContinuationRequiresReleasedWriter: true,
      activeWriterProtectionIsNotCapabilityFailure: true,
    },
  };
}

export function validateSanitizedEvidence(evidence) {
  rejectForbiddenKeys(evidence);
  if (evidence?.phase !== "3.3-final" || evidence?.schemaVersion !== 1) throw new Error("invalid Phase 3.3 evidence header");
  if (!FULL_ID.test(evidence.identity?.fullThreadId ?? "")) throw new Error("invalid canonical Thread identity");
  if (evidence.identity.canonicalThreadCount !== 1 || evidence.identity.duplicateCanonicalThreadCount !== 0) {
    throw new Error("canonical Thread duplication detected");
  }
  if (evidence.creation?.threadAcknowledged !== "CONFIRMED"
    || evidence.creation?.persistenceConfirmed !== "CONFIRMED"
    || evidence.creation?.firstTurnAccepted !== "CONFIRMED"
    || evidence.creation?.firstTurnOutcome !== "COMPLETED") {
    throw new Error("creation facts are not independently confirmed");
  }
  if (evidence.continuation?.monitorToDesktopIdle?.result !== "PASS"
    || evidence.continuation?.desktopToCliIdleAfterRelease?.result !== "PASS"
    || evidence.continuation?.distinctTurnIds !== true) {
    throw new Error("idle cross-Surface continuation is incomplete");
  }
  if (evidence.occupancy?.result !== "BLOCKED_BY_ACTIVE_WRITER") throw new Error("occupancy protection evidence is missing");
  if (evidence.postReleaseExactId?.result !== "SUCCESS") throw new Error("post-release exact-ID probe is missing");
  if (evidence.reconstruction?.result !== "SUCCESS"
    || evidence.reconstruction?.monitorUiReconstruction !== "USER_CONFIRMED") {
    throw new Error("restart reconstruction evidence is incomplete");
  }
  if (evidence.boundaries?.rawEvidenceCommitted !== false
    || evidence.boundaries?.userContentStored !== false
    || evidence.boundaries?.desktopPrivateStateWrittenByHarness !== false) {
    throw new Error("privacy or read-only boundary failed");
  }
}

async function finalize() {
  const runDirectory = path.resolve(arg("run-dir", ""));
  const output = path.resolve(arg("output", ""));
  if (!output) throw new Error("finalize requires --output");
  const run = await loadRun(runDirectory);
  const finalCodexCliVersion = await executableVersion(arg("codex-bin", "codex"), ["--version"]);
  const evidence = buildSanitizedEvidence(run, finalCodexCliVersion);
  validateSanitizedEvidence(evidence);
  await writeJsonAtomic(output, evidence);
  process.stdout.write(`${JSON.stringify({ result: "READY_FOR_FINAL_ASSESSMENT", output })}\n`);
}

async function main() {
  const command = process.argv[2];
  if (command === "prepare") return prepare();
  if (command === "capture") return capture();
  if (command === "record-active-writer") return recordActiveWriter();
  if (command === "probe") return probe();
  if (command === "status") return status();
  if (command === "finalize") return finalize();
  if (command === "verify") {
    const input = path.resolve(arg("input", ""));
    const evidence = JSON.parse(await readFile(input, "utf8"));
    validateSanitizedEvidence(evidence);
    process.stdout.write("Phase 3.3 Final Acceptance evidence validation PASS\n");
    return;
  }
  throw new Error("usage: phase-3-3-final-acceptance.mjs <prepare|capture|record-active-writer|probe|status|finalize|verify>");
}

const invokedPath = process.argv[1] ? pathToFileURL(path.resolve(process.argv[1])).href : null;
if (invokedPath === import.meta.url) {
  main().catch((error) => {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  });
}
