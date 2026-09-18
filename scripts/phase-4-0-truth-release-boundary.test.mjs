import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

async function read(relativePath) {
  return readFile(path.join(repositoryRoot, relativePath), "utf8");
}

test("root roadmap points to P4.1 and keeps Phase 3.5 frozen", async () => {
  const roadmap = await read("CodexMonitor_四阶段开发路线图.md");
  const currentState = roadmap.slice(roadmap.indexOf("# 当前进度"));
  assert.doesNotMatch(currentState, /Phase 3\.3\.3b/);
  assert.match(currentState, /Phase 3\.5 PASS \/ COMPLETE \/ FROZEN/);
  assert.match(currentState, /Phase 4 — Productization[\s\S]*IN PROGRESS/);
  assert.match(currentState, /P4\.1 — Release Identity \/ Version \/ Migration \/ Update Safety/);
});

test("Phase 4 authority freezes eleven approved decisions and P4.0 through P4.7", async () => {
  const authority = await read("docs/phase-4-0-truth-release-boundary.md");
  const decisionHeadings = authority.match(/^\d+\. \*\*/gm) ?? [];
  assert.equal(decisionHeadings.length, 11);
  for (let phase = 0; phase <= 7; phase += 1) {
    assert.match(authority, new RegExp(`### P4\\.${phase} —`));
  }
  assert.match(authority, /Windows Daily-use Milestone/);
  assert.match(authority, /Adaptive Model Router remains an \*\*Advanced Phase\*\*/);
});

test("Phase 3.5 evidence classification remains explicit and JSON is parseable", async () => {
  const evidence = JSON.parse(
    await read("docs/evidence/phase-3-5-final/final-acceptance.json"),
  );
  assert.equal(evidence.status, "PASS_COMPLETE_FROZEN");
  assert.equal(
    evidence.isolatedE2e.staleOldGenerationEvidence.classification,
    "DETERMINISTIC_FIXTURE_CONTRACT",
  );
  assert.equal(
    evidence.isolatedE2e.staleOldGenerationEvidence.realTransportLateNotificationScenario,
    "NOT_EXECUTED",
  );
  assert.equal(evidence.evidenceCorrection.productIsolationRegressionStatus, "PASS");
});
