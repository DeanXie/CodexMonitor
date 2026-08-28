import { mkdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const TOP_LEVEL_FIELDS = new Set([
  "evidenceVersion",
  "phase",
  "scenario",
  "timestamp",
  "fullThreadId",
  "parentThreadId",
  "childThreadIds",
  "sourceLanes",
  "observedModels",
  "authoritativeTokens",
  "lifecycle",
  "freshness",
  "lagMs",
  "cursor",
  "result",
  "notes",
]);

const NESTED_FIELDS = {
  sourceLanes: new Set(["sourceKind", "temporalClass", "sourceInstanceId", "generation"]),
  observedModels: new Set(["threadId", "model", "provenance"]),
  authoritativeTokens: new Set(["threadId", "totalTokens", "provenance"]),
  lagMs: new Set(["minimum", "maximum", "samples"]),
  cursor: new Set(["generation", "committedByteOffset", "recordOrdinal"]),
};
const SOURCE_KINDS = new Set([
  "monitor-app-server",
  "codex-cli-rollout",
  "historical-rollout-scan",
]);
const TEMPORAL_CLASSES = new Set(["LIVE", "NEAR_LIVE", "HISTORICAL"]);

function assertObject(value, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError(`${label} must be an object`);
  }
}

function assertAllowedFields(value, allowed, label) {
  assertObject(value, label);
  for (const key of Object.keys(value)) {
    if (!allowed.has(key)) throw new TypeError(`Unsupported field ${label}.${key}`);
  }
}

function assertString(value, label) {
  if (typeof value !== "string" || value.trim() === "") {
    throw new TypeError(`${label} must be a non-empty string`);
  }
}

export function validateEvidenceSummary(candidate) {
  assertAllowedFields(candidate, TOP_LEVEL_FIELDS, "summary");
  if (candidate.evidenceVersion !== 1) throw new TypeError("evidenceVersion must be 1");
  assertString(candidate.phase, "phase");
  assertString(candidate.scenario, "scenario");
  assertString(candidate.timestamp, "timestamp");
  assertString(candidate.fullThreadId, "fullThreadId");
  if (candidate.parentThreadId !== null && candidate.parentThreadId !== undefined) {
    assertString(candidate.parentThreadId, "parentThreadId");
  }
  for (const field of ["childThreadIds", "sourceLanes", "observedModels", "authoritativeTokens"]) {
    if (!Array.isArray(candidate[field])) throw new TypeError(`${field} must be an array`);
  }
  candidate.childThreadIds.forEach((value, index) => assertString(value, `childThreadIds[${index}]`));
  for (const field of ["sourceLanes", "observedModels", "authoritativeTokens"]) {
    candidate[field].forEach((value, index) => {
      assertAllowedFields(value, NESTED_FIELDS[field], `${field}[${index}]`);
    });
  }
  candidate.sourceLanes.forEach((lane, index) => {
    if (!SOURCE_KINDS.has(lane.sourceKind)) {
      throw new TypeError(`sourceLanes[${index}].sourceKind is unsupported`);
    }
    if (!TEMPORAL_CLASSES.has(lane.temporalClass)) {
      throw new TypeError(`sourceLanes[${index}].temporalClass is unsupported`);
    }
  });
  candidate.observedModels.forEach((observation, index) => {
    assertString(observation.threadId, `observedModels[${index}].threadId`);
    assertString(observation.model, `observedModels[${index}].model`);
    assertString(observation.provenance, `observedModels[${index}].provenance`);
  });
  candidate.authoritativeTokens.forEach((snapshot, index) => {
    assertString(snapshot.threadId, `authoritativeTokens[${index}].threadId`);
    if (!Number.isSafeInteger(snapshot.totalTokens) || snapshot.totalTokens < 0) {
      throw new TypeError(`authoritativeTokens[${index}].totalTokens must be a non-negative integer`);
    }
    assertString(snapshot.provenance, `authoritativeTokens[${index}].provenance`);
  });
  if (candidate.lagMs !== null && candidate.lagMs !== undefined) {
    assertAllowedFields(candidate.lagMs, NESTED_FIELDS.lagMs, "lagMs");
  }
  if (candidate.cursor !== null && candidate.cursor !== undefined) {
    assertAllowedFields(candidate.cursor, NESTED_FIELDS.cursor, "cursor");
  }
  if (candidate.result !== "PASS" && candidate.result !== "FAIL") {
    throw new TypeError("result must be PASS or FAIL");
  }
  return structuredClone(candidate);
}

export function serializeEvidenceSummary(candidate) {
  return `${JSON.stringify(validateEvidenceSummary(candidate), null, 2)}\n`;
}

export async function writeEvidenceSummary(destination, candidate) {
  const target = resolve(destination);
  const temporary = `${target}.tmp-${process.pid}-${Date.now()}`;
  await mkdir(dirname(target), { recursive: true });
  try {
    await writeFile(temporary, serializeEvidenceSummary(candidate), { encoding: "utf8", flag: "wx" });
    await rename(temporary, target);
  } catch (error) {
    await rm(temporary, { force: true });
    throw error;
  }
  return target;
}

async function main() {
  const [input, output] = process.argv.slice(2);
  if (!input || !output) {
    throw new Error("Usage: node scripts/e2e-evidence-summary.mjs <input.json> <output.json>");
  }
  const candidate = JSON.parse(await readFile(input, "utf8"));
  await writeEvidenceSummary(output, candidate);
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
