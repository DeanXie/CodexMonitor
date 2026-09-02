import { spawn } from "node:child_process";
import { createInterface } from "node:readline";

const codexBin = process.env.PHASE_3_2_5_CODEX_BIN;
const threadId = process.env.PHASE_3_2_5_THREAD_ID;

if (!codexBin || !threadId) {
  throw new Error("PHASE_3_2_5_CODEX_BIN and PHASE_3_2_5_THREAD_ID are required");
}

const child = spawn(codexBin, ["app-server"], {
  env: process.env,
  stdio: ["pipe", "pipe", "pipe"],
  windowsHide: true,
});

let nextId = 1;
const pending = new Map();
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

const timeout = setTimeout(() => {
  child.kill();
  throw new Error(`app-server reconstruction probe timed out: ${stderr}`);
}, 30000);

try {
  await sendRequest("initialize", {
    clientInfo: {
      name: "codex-monitor-phase-3-2-5-reconstruction-probe",
      title: "CodexMonitor Phase 3.2.5 Reconstruction",
      version: "1.0.0",
    },
    capabilities: { experimentalApi: true },
  });
  sendNotification("initialized");

  const resumeResult = await sendRequest("thread/resume", { threadId });
  if (resumeResult?.thread?.id !== threadId) {
    throw new Error("thread/resume did not preserve the requested exact fullThreadId");
  }

  const readResult = await sendRequest("thread/read", { threadId });
  const reconstructed = readResult?.thread;
  if (reconstructed?.id !== threadId) {
    throw new Error("thread/read did not preserve the requested exact fullThreadId");
  }

  process.stdout.write(
    `${JSON.stringify({
      phase: "3.2.5",
      probe: "fresh-process-thread-read",
      fullThreadId: reconstructed.id,
      cwd: reconstructed.cwd ?? null,
      existingTurnIds: Array.isArray(reconstructed.turns)
        ? reconstructed.turns.map((turn) => turn.id).filter(Boolean)
        : [],
      resumeCreatedTurn: (resumeResult.thread.turns?.length ?? 0) !==
        (reconstructed.turns?.length ?? 0),
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
