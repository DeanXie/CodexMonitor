import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const evidenceDir = path.join(root, "docs", "evidence", "phase-3-2-5");
const environment = JSON.parse(await readFile(path.join(evidenceDir, "environment.json"), "utf8"));
const matrix = JSON.parse(await readFile(path.join(evidenceDir, "gate-matrix.json"), "utf8"));
const restart = JSON.parse(await readFile(path.join(evidenceDir, "restart-reconstruction.json"), "utf8"));

for (const document of [environment, matrix, restart]) {
  if (document.phase !== "3.2.5" || document.schemaVersion !== 1) {
    throw new Error("Phase 3.2.5 evidence header is invalid");
  }
}

const expectedGates = "ABCDEFGHIJKL".split("");
if (matrix.gates.length !== expectedGates.length) {
  throw new Error("Gate matrix must contain exactly A-L");
}
for (const [index, gate] of matrix.gates.entries()) {
  if (gate.gate !== expectedGates[index] || gate.result !== "PASS") {
    throw new Error(`Gate ${expectedGates[index]} is missing or not PASS`);
  }
}

const forbiddenKeys = new Set(["prompt", "response", "reasoning", "credential", "cookie"]);
function rejectSensitiveKeys(value) {
  if (Array.isArray(value)) {
    value.forEach(rejectSensitiveKeys);
    return;
  }
  if (value && typeof value === "object") {
    for (const [key, child] of Object.entries(value)) {
      if (forbiddenKeys.has(key.toLowerCase())) {
        throw new Error(`Evidence contains forbidden key: ${key}`);
      }
      rejectSensitiveKeys(child);
    }
  }
}
rejectSensitiveKeys([environment, matrix, restart]);

if (!environment.privateStateReadOnlyBracket.beforeEqualsAfter) {
  throw new Error("Desktop private-state hash bracket did not pass");
}
if (restart.runtimeRelationReconstruction.historicalTurnRelation !== "NOT RECOVERABLE BY CURRENT CONTRACT") {
  throw new Error("Historical Turn reconstruction boundary must remain explicit");
}

process.stdout.write("Phase 3.2.5 evidence validation PASS: Gates A-L, sanitized fields, read-only bracket, and reconstruction boundary verified.\n");
