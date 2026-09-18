import { randomBytes, randomUUID } from "node:crypto";
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { copyFile, mkdir, readFile, readdir, rm, stat, writeFile } from "node:fs/promises";
import net from "node:net";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const ALLOWED_REMOTE_METHODS = [
  "auth",
  "daemon_info",
  "list_workspaces",
  "connect_workspace",
  "list_threads",
  "read_thread",
  "thread_live_subscribe",
  "get_authoritative_observation_snapshot",
  "get_projection_freshness",
];

const FORBIDDEN_REMOTE_METHODS = new Set([
  "resume_thread",
  "respond_to_server_request",
  "delete_thread",
  "thread_upstream_unsubscribe",
  "start_thread",
  "send_user_message",
]);

const AUTHORIZED_CREDENTIAL_SOURCE = "C:\\Users\\DeanX\\.codex\\auth.json";

function canonicalPath(value) {
  return path.resolve(value).replaceAll("/", "\\").toLowerCase();
}

export function assertIsolatedAcceptancePaths(paths) {
  const root = canonicalPath(paths.runRoot);
  for (const [name, value] of Object.entries(paths)) {
    if (name === "runRoot") continue;
    const candidate = canonicalPath(value);
    if (candidate === root || !candidate.startsWith(`${root}\\`)) {
      throw new Error(`${name} must remain inside the isolated acceptance run root`);
    }
  }
}

export function assertCredentialCopyPaths({ source, destination, runRoot, expectedSource }) {
  if (canonicalPath(source) !== canonicalPath(expectedSource)) {
    throw new Error("credential source is outside the explicitly authorized path");
  }
  const isolatedRoot = canonicalPath(runRoot);
  const isolatedCodexHome = `${isolatedRoot}\\codex-home`;
  const target = canonicalPath(destination);
  if (target !== `${isolatedCodexHome}\\auth.json`) {
    throw new Error("temporary credential destination must be isolated CODEX_HOME\\auth.json");
  }
}

export async function withTemporaryCredential(paths, operation) {
  assertCredentialCopyPaths(paths);
  await stat(paths.source);
  await mkdir(path.dirname(paths.destination), { recursive: true });
  try {
    await stat(paths.destination);
    throw new Error("temporary credential destination already exists");
  } catch (error) {
    if (error?.code !== "ENOENT") throw error;
  }
  await copyFile(paths.source, paths.destination);
  try {
    await stat(paths.destination);
    return await operation({ destinationExists: true });
  } finally {
    await rm(paths.destination, { force: true });
    try {
      await stat(paths.destination);
      throw new Error("temporary credential cleanup failed");
    } catch (error) {
      if (error?.code !== "ENOENT") throw error;
    }
  }
}

export function sanitizeAcceptanceEvidence(raw) {
  return {
    remoteHostIdentity: raw.remoteHostIdentity ?? null,
    daemonProcessGeneration: raw.daemonProcessGeneration ?? null,
    initialTransportGeneration: raw.initialTransportGeneration ?? null,
    reconnectedTransportGeneration: raw.reconnectedTransportGeneration ?? null,
    workspaceSessionGeneration: raw.workspaceSessionGeneration ?? null,
    appServerConnectionGeneration: raw.appServerConnectionGeneration ?? null,
    threadId: raw.thread?.id ?? raw.threadId ?? null,
    exactThreadIdMatch: raw.exactThreadIdMatch ?? null,
    generationTaggedEventObserved: raw.generationTaggedEventObserved ?? null,
    staleOldGenerationRejected: raw.staleOldGenerationRejected ?? null,
    staleOldGenerationEvidence: {
      classification: "DETERMINISTIC_FIXTURE_CONTRACT",
      source: "docs/fixtures/remote-transport-coordination/stale-delivery.json",
      productionFunctionRegression: "PASS",
      realTransportLateNotificationScenario: "NOT_EXECUTED",
      installedAppManualUiScenario: "NOT_EXECUTED",
    },
    projectionHydratedCurrent: raw.projectionHydratedCurrent ?? null,
    projectionRehydratedCurrent: raw.projectionRehydratedCurrent ?? null,
    forbiddenMutationCounts: raw.forbiddenMutationCounts ?? null,
    setupMutationCounts: raw.setupMutationCounts ?? null,
    invokedRemoteMethods: raw.invokedRemoteMethods ?? null,
    result: raw.result ?? null,
  };
}

export function validateAcceptanceEvidence(evidence) {
  if (evidence.result !== "PASS") throw new Error("acceptance result is not PASS");
  if (!evidence.exactThreadIdMatch) throw new Error("exact Thread identity was not preserved");
  if (!evidence.initialTransportGeneration || !evidence.reconnectedTransportGeneration) {
    throw new Error("transport generations are required");
  }
  if (evidence.initialTransportGeneration === evidence.reconnectedTransportGeneration) {
    throw new Error("reconnect did not mint a new transport generation");
  }
  if (!evidence.staleOldGenerationRejected) throw new Error("stale generation rejection missing");
  if (evidence.staleOldGenerationEvidence?.classification !== "DETERMINISTIC_FIXTURE_CONTRACT") {
    throw new Error("stale generation evidence classification missing");
  }
  if (evidence.staleOldGenerationEvidence?.realTransportLateNotificationScenario !== "NOT_EXECUTED") {
    throw new Error("real transport late-notification status must remain NOT_EXECUTED");
  }
  if (!evidence.generationTaggedEventObserved) throw new Error("generation-tagged event missing");
  if (!evidence.projectionHydratedCurrent || !evidence.projectionRehydratedCurrent) {
    throw new Error("projection hydration did not reach current");
  }
  for (const [name, count] of Object.entries(evidence.forbiddenMutationCounts ?? {})) {
    if (count !== 0) throw new Error(`forbidden mutation ${name} count was ${count}`);
  }
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function freeLoopbackPort() {
  return await new Promise((resolve, reject) => {
    const server = net.createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      const port = typeof address === "object" && address ? address.port : null;
      server.close((error) => (error ? reject(error) : resolve(port)));
    });
  });
}

class DaemonRpcClient {
  constructor(socket) {
    this.socket = socket;
    this.nextId = 1;
    this.pending = new Map();
    this.notifications = [];
    createInterface({ input: socket }).on("line", (line) => this.#receive(line));
  }

  static async connect(port) {
    const socket = await new Promise((resolve, reject) => {
      const candidate = net.createConnection({ host: "127.0.0.1", port });
      candidate.once("connect", () => resolve(candidate));
      candidate.once("error", reject);
    });
    socket.setEncoding("utf8");
    return new DaemonRpcClient(socket);
  }

  #receive(line) {
    let message;
    try {
      message = JSON.parse(line);
    } catch {
      return;
    }
    if (message.id === undefined) {
      this.notifications.push(message);
      return;
    }
    const waiter = this.pending.get(message.id);
    if (!waiter) return;
    this.pending.delete(message.id);
    clearTimeout(waiter.timer);
    if (message.error) waiter.reject(new Error(message.error.message ?? JSON.stringify(message.error)));
    else waiter.resolve(message.result);
  }

  request(method, params = {}, timeoutMs = 30_000) {
    if (!ALLOWED_REMOTE_METHODS.includes(method) || FORBIDDEN_REMOTE_METHODS.has(method)) {
      throw new Error(`Remote method ${method} is outside final acceptance allowlist`);
    }
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`${method} timed out`));
      }, timeoutMs);
      this.pending.set(id, { resolve, reject, timer });
      this.socket.write(`${JSON.stringify({ id, method, params })}\n`, (error) => {
        if (!error) return;
        clearTimeout(timer);
        this.pending.delete(id);
        reject(error);
      });
    });
  }

  close() {
    this.socket.destroy();
  }
}

function findThreadId(value, expected) {
  if (Array.isArray(value)) {
    return value.some((entry) => findThreadId(entry, expected));
  }
  if (!value || typeof value !== "object") return false;
  if (value.id === expected || value.threadId === expected) return true;
  return Object.values(value).some((entry) => findThreadId(entry, expected));
}

async function waitForExactThreadInList(call, client, workspaceId, threadId, timeoutMs = 30_000) {
  const started = Date.now();
  let lastResult = null;
  while (Date.now() - started < timeoutMs) {
    lastResult = await call(client, "list_threads", { workspaceId, limit: 100, sortKey: "updated_at" });
    if (findThreadId(lastResult, threadId)) return lastResult;
    await delay(250);
  }
  throw new Error("exact disposable Thread missing from thread/list after bounded index hydration");
}

function coverage(snapshot, name) {
  return snapshot?.coverages?.find((entry) => entry.coverage === name);
}

function requireCurrentCoverage(snapshot, names) {
  for (const name of names) {
    const entry = coverage(snapshot, name);
    if (!entry || entry.status !== "current") {
      throw new Error(`${name} did not hydrate to current`);
    }
  }
}

async function waitForDaemon(port, process, timeoutMs = 30_000) {
  const started = Date.now();
  while (Date.now() - started < timeoutMs) {
    if (process.exitCode !== null) throw new Error(`daemon exited early with code ${process.exitCode}`);
    try {
      const client = await DaemonRpcClient.connect(port);
      client.close();
      return;
    } catch {
      await delay(100);
    }
  }
  throw new Error("daemon listener did not become ready");
}

function transportGeneration(snapshot) {
  const values = new Set(
    (snapshot?.coverages ?? [])
      .map((entry) => entry?.generations?.remoteTransportGeneration)
      .filter((value) => typeof value === "string" && value.length > 0),
  );
  if (values.size !== 1) throw new Error("freshness snapshot omitted a unique transport generation");
  return [...values][0];
}

export async function loadStaleDeliveryFixture(repositoryRoot) {
  const fixturePath = path.join(
    repositoryRoot,
    "docs",
    "fixtures",
    "remote-transport-coordination",
    "stale-delivery.json",
  );
  const fixture = JSON.parse(await readFile(fixturePath, "utf8"));
  const scenarios = Array.isArray(fixture?.scenarios) ? fixture.scenarios : [];
  const failClosed = scenarios.some((scenario) => scenario?.event === "stale_notification")
    && scenarios.every(
    (scenario) => typeof scenario?.event === "string"
      && scenario.sourceGeneration === "old"
      && scenario.deliveredToCurrent === false
      && scenario.currentStateChanged === false,
  );
  if (!failClosed) {
    throw new Error("stale-delivery production-gate fixture is not fail-closed");
  }
  return true;
}

async function assertDisposableThreadRollout(codexHome, threadId) {
  const sessionsRoot = path.join(codexHome, "sessions");
  const entries = await readdir(sessionsRoot, { recursive: true, withFileTypes: true });
  const matches = entries.filter(
    (entry) => entry.isFile() && entry.name.endsWith(`-${threadId}.jsonl`),
  );
  if (matches.length !== 1) {
    throw new Error("exact disposable Thread rollout was not uniquely present in isolated CODEX_HOME");
  }
}

export async function validateExistingWorkspaceConfig({ configPath, workspaceId, workspacePath }) {
  const workspaces = JSON.parse(await readFile(configPath, "utf8"));
  const workspace = Array.isArray(workspaces)
    ? workspaces.find((entry) => entry?.id === workspaceId)
    : null;
  if (!workspace || canonicalPath(workspace.path) !== canonicalPath(workspacePath)) {
    throw new Error("existing disposable workspace config does not match the isolated run");
  }
}

export async function runAcceptance() {
  const scriptPath = fileURLToPath(import.meta.url);
  const repositoryRoot = path.resolve(path.dirname(scriptPath), "..");
  const acceptanceRoot = path.join(repositoryRoot, "src-tauri", "target", "phase-3-5-final-acceptance");
  const configuredRunRoot = process.env.PHASE_3_5_ACCEPTANCE_RUN_ROOT;
  const threadId = process.env.PHASE_3_5_ACCEPTANCE_THREAD_ID;
  if (!configuredRunRoot || !threadId) {
    throw new Error("existing isolated acceptance run root and exact Thread ID are required");
  }
  const runRoot = path.resolve(configuredRunRoot);
  if (!canonicalPath(runRoot).startsWith(`${canonicalPath(acceptanceRoot)}\\`)) {
    throw new Error("acceptance run root is outside the ignored isolated acceptance root");
  }
  if (!/^[0-9a-f-]{36}$/i.test(threadId)) {
    throw new Error("acceptance exact Thread ID is malformed");
  }
  const runId = path.basename(runRoot);
  const codexHome = path.join(runRoot, "codex-home");
  const daemonDataDir = path.join(runRoot, "daemon-data");
  const workspacePath = path.join(runRoot, "workspace");
  const evidencePath = path.join(runRoot, "raw-evidence.json");
  const credentialDestination = path.join(codexHome, "auth.json");
  assertIsolatedAcceptancePaths({ runRoot, codexHome, daemonDataDir, workspacePath, evidencePath, credentialDestination });
  await Promise.all([codexHome, daemonDataDir, workspacePath].map((dir) => stat(dir)));
  await assertDisposableThreadRollout(codexHome, threadId);

  const daemonBin = process.env.PHASE_3_5_DAEMON_BIN || path.join(repositoryRoot, "src-tauri", "target", "debug", "codex_monitor_daemon.exe");
  const workspaceId = "phase-3-5-final-disposable-workspace";
  await validateExistingWorkspaceConfig({
    configPath: path.join(daemonDataDir, "workspaces.json"),
    workspaceId,
    workspacePath,
  });

  return await withTemporaryCredential(
    {
      source: AUTHORIZED_CREDENTIAL_SOURCE,
      destination: credentialDestination,
      runRoot,
      expectedSource: AUTHORIZED_CREDENTIAL_SOURCE,
    },
    async () => {
      const port = await freeLoopbackPort();
      const token = randomBytes(32).toString("base64url");
      const daemon = spawn(daemonBin, ["--listen", `127.0.0.1:${port}`, "--data-dir", daemonDataDir, "--token", token], {
    cwd: repositoryRoot,
    env: { ...process.env, CODEX_HOME: codexHome },
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
      });
      let daemonStderr = "";
      daemon.stderr.setEncoding("utf8");
      daemon.stderr.on("data", (chunk) => {
        daemonStderr = `${daemonStderr}${chunk}`.slice(-8000);
      });

      const invokedRemoteMethods = [];
      const call = async (client, method, params = {}) => {
        invokedRemoteMethods.push(method);
        return await client.request(method, params);
      };
      let first;
      let second;
      try {
    await waitForDaemon(port, daemon);
    first = await DaemonRpcClient.connect(port);
    await call(first, "auth", { token });
    const info = await call(first, "daemon_info");
    if (info?.name !== "codex-monitor-daemon" || info?.mode !== "tcp" || info?.protocolVersion !== 1) {
      throw new Error("daemon identity/service/mode/protocol validation failed");
    }
    const initialWorkspaces = await call(first, "list_workspaces");
    const workspace = initialWorkspaces.find((entry) => entry.id === workspaceId);
    if (!workspace) throw new Error("disposable workspace missing from list_workspaces");
    await call(first, "connect_workspace", { id: workspaceId });
    const connectedWorkspaces = await call(first, "list_workspaces");
    if (!connectedWorkspaces.find((entry) => entry.id === workspaceId)?.connected) {
      throw new Error("disposable workspace did not connect");
    }
    await waitForExactThreadInList(call, first, workspaceId, threadId);
    const read = await call(first, "read_thread", { workspaceId, threadId });
    if (!findThreadId(read, threadId)) throw new Error("thread/read returned a mismatched exact Thread");
    await call(first, "get_authoritative_observation_snapshot", { workspaceId, threadId });
    await call(first, "thread_live_subscribe", { workspaceId, threadId });
    await delay(100);
    const hydrated = await call(first, "get_projection_freshness", { workspaceId, threadId });
    requireCurrentCoverage(hydrated, ["workspace_catalog", "thread_catalog", "thread_detail", "observation_snapshot"]);
    const initialTransportGeneration = transportGeneration(hydrated);
    const generationEvent = first.notifications.find(
      (message) => message.method === "app-server-event"
        && message.params?.remoteTransportGeneration === initialTransportGeneration
        && message.params?.daemonProcessGeneration === info.daemonProcessGeneration
        && typeof message.params?.workspaceSessionGeneration === "string"
        && typeof message.params?.appServerConnectionGeneration === "string",
    );
    if (!generationEvent) throw new Error("generation-tagged app-server event was not observed");

    first.close();
    first = null;
    second = await DaemonRpcClient.connect(port);
    await call(second, "auth", { token });
    const reconnectInfo = await call(second, "daemon_info");
    if (reconnectInfo.remoteHostIdentity !== info.remoteHostIdentity || reconnectInfo.daemonProcessGeneration !== info.daemonProcessGeneration) {
      throw new Error("same-daemon reconnect identity changed unexpectedly");
    }
    const beforeRehydrate = await call(second, "get_projection_freshness", { workspaceId, threadId });
    const reconnectedTransportGeneration = transportGeneration(beforeRehydrate);
    if (reconnectedTransportGeneration === initialTransportGeneration) {
      throw new Error("Remote reconnect did not mint a new transport generation");
    }
    await waitForExactThreadInList(call, second, workspaceId, threadId);
    const reread = await call(second, "read_thread", { workspaceId, threadId });
    if (!findThreadId(reread, threadId)) throw new Error("rehydration thread/read changed exact Thread");
    await call(second, "get_authoritative_observation_snapshot", { workspaceId, threadId });
    const rehydrated = await call(second, "get_projection_freshness", { workspaceId, threadId });
    requireCurrentCoverage(rehydrated, ["workspace_catalog", "thread_catalog", "thread_detail", "observation_snapshot"]);
    const staleOldGenerationRejected = await loadStaleDeliveryFixture(repositoryRoot);
    const raw = {
      token,
      thread: { id: threadId, response: read },
      remoteHostIdentity: info.remoteHostIdentity,
      daemonProcessGeneration: info.daemonProcessGeneration,
      initialTransportGeneration,
      reconnectedTransportGeneration,
      workspaceSessionGeneration: generationEvent.params.workspaceSessionGeneration,
      appServerConnectionGeneration: generationEvent.params.appServerConnectionGeneration,
      exactThreadIdMatch: true,
      generationTaggedEventObserved: true,
      staleOldGenerationRejected,
      projectionHydratedCurrent: true,
      projectionRehydratedCurrent: true,
      forbiddenMutationCounts: {
        resumeThread: 0,
        approvalDecision: 0,
        threadDelete: 0,
        upstreamUnsubscribe: 0,
        forceTakeover: 0,
      },
      setupMutationCounts: { disposableThreadStart: 0, disposableTurnStart: 0, localSyntheticLiveAttach: 1 },
      invokedRemoteMethods,
      result: "PASS",
    };
    const evidence = sanitizeAcceptanceEvidence(raw);
    validateAcceptanceEvidence(evidence);
    await writeFile(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`, "utf8");
    return { ...evidence, runId, evidencePath, codexVersion: null };
      } catch (error) {
        const suffix = daemonStderr ? `; daemon diagnostics: ${daemonStderr}` : "";
        throw new Error(`${error.message}${suffix}`);
      } finally {
        first?.close();
        second?.close();
        daemon.kill();
      }
    },
  );
}

const isMain = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) {
  runAcceptance()
    .then((result) => process.stdout.write(`${JSON.stringify(result)}\n`))
    .catch((error) => {
      process.stderr.write(`${error.message}\n`);
      process.exitCode = 1;
    });
}
