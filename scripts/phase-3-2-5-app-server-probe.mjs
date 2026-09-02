import { spawn } from "node:child_process";
import { createInterface } from "node:readline";

const codexBin = process.env.PHASE_3_2_5_CODEX_BIN;
const cwd = process.env.PHASE_3_2_5_CWD;

if (!codexBin || !cwd) {
  throw new Error("PHASE_3_2_5_CODEX_BIN and PHASE_3_2_5_CWD are required");
}

const child = spawn(codexBin, ["app-server"], {
  cwd,
  env: process.env,
  stdio: ["pipe", "pipe", "pipe"],
  windowsHide: true,
});

let nextId = 1;
const pending = new Map();
const observedMethods = new Set();
let stderr = "";

child.stderr.setEncoding("utf8");
child.stderr.on("data", (chunk) => {
  stderr = `${stderr}${chunk}`.slice(-4000);
});

const lines = createInterface({ input: child.stdout });
lines.on("line", (line) => {
  let message;
  try {
    message = JSON.parse(line);
  } catch {
    return;
  }
  if (typeof message.method === "string") {
    observedMethods.add(message.method);
  }
  if (message.id !== undefined && pending.has(message.id)) {
    const { resolve, reject } = pending.get(message.id);
    pending.delete(message.id);
    if (message.error) {
      reject(new Error(JSON.stringify(message.error)));
    } else {
      resolve(message.result);
    }
  }
});

function sendNotification(method, params) {
  const message = params === undefined ? { method } : { method, params };
  child.stdin.write(`${JSON.stringify(message)}\n`);
}

function sendRequest(method, params) {
  const id = nextId++;
  child.stdin.write(`${JSON.stringify({ id, method, params })}\n`);
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
  });
}

function waitForMethod(method, timeoutMs = 120000) {
  return new Promise((resolve, reject) => {
    const started = Date.now();
    const timer = setInterval(() => {
      if (observedMethods.has(method)) {
        clearInterval(timer);
        resolve();
      } else if (Date.now() - started >= timeoutMs) {
        clearInterval(timer);
        reject(new Error(`timed out waiting for ${method}`));
      }
    }, 50);
  });
}

const timeout = setTimeout(() => {
  child.kill();
  throw new Error(`app-server probe timed out: ${stderr}`);
}, 150000);

try {
  await sendRequest("initialize", {
    clientInfo: {
      name: "codex-monitor-phase-3-2-5-probe",
      title: "CodexMonitor Phase 3.2.5",
      version: "1.0.0",
    },
    capabilities: { experimentalApi: true },
  });
  sendNotification("initialized");

  const threadResult = await sendRequest("thread/start", {
    cwd,
    approvalPolicy: "on-request",
  });
  const threadId = threadResult?.thread?.id;
  if (!threadId) {
    throw new Error("thread/start response omitted result.thread.id");
  }

  const turnResult = await sendRequest("turn/start", {
    threadId,
    input: [
      {
        type: "text",
        text: "Phase 3.2.5 isolated app-server E2E. Return only PHASE_3_2_5_APP_SERVER_OK.",
      },
    ],
    cwd,
    approvalPolicy: "on-request",
    sandboxPolicy: { type: "readOnly" },
    model: null,
    effort: null,
  });
  const turnId = turnResult?.turn?.id;
  if (!turnId) {
    throw new Error("turn/start response omitted result.turn.id");
  }
  await waitForMethod("turn/completed");

  process.stdout.write(
    `${JSON.stringify({
      phase: "3.2.5",
      probe: "isolated-app-server",
      threadStart: { cwd, fullThreadId: threadId },
      turnStart: { cwd, fullTurnId: turnId, sandbox: "readOnly" },
      observedLifecycleMethods: [...observedMethods]
        .filter((method) =>
          ["thread/started", "turn/started", "turn/completed"].includes(method),
        )
        .sort(),
      promptPersistedInEvidence: false,
      responsePersistedInEvidence: false,
      result: "PASS",
    })}\n`,
  );
} catch (error) {
  process.stderr.write(`${error.message}\n${stderr}\n`);
  process.exitCode = 1;
} finally {
  clearTimeout(timeout);
  child.kill();
}
