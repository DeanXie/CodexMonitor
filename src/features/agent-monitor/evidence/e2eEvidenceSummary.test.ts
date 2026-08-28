import { describe, expect, it } from "vitest";
import phase24C from "../../../../docs/evidence/phase-2-4-c.json";

import {
  serializeEvidenceSummary,
  validateEvidenceSummary,
} from "../../../../scripts/e2e-evidence-summary.mjs";

function validSummary() {
  return {
    evidenceVersion: 1,
    phase: "2.4",
    scenario: "C",
    timestamp: "2026-08-26T12:00:00.000Z",
    fullThreadId: "01a03e8f-aa9d-77c1-b787-c2a9bd89c8e9",
    parentThreadId: null,
    childThreadIds: ["child-confirmed"],
    sourceLanes: [
      { sourceKind: "monitor-app-server", temporalClass: "LIVE" },
      { sourceKind: "codex-cli-rollout", temporalClass: "NEAR_LIVE" },
    ],
    observedModels: [
      { threadId: "01a03e8f-aa9d-77c1-b787-c2a9bd89c8e9", model: "gpt-5.6-terra", provenance: "monitor-app-server" },
    ],
    authoritativeTokens: [
      { threadId: "01a03e8f-aa9d-77c1-b787-c2a9bd89c8e9", totalTokens: 312_607, provenance: "monitor-app-server" },
    ],
    lifecycle: "completed",
    freshness: "settled",
    lagMs: { minimum: 100, maximum: 600, samples: 4 },
    cursor: { generation: "file:redacted", committedByteOffset: 1000, recordOrdinal: 12 },
    result: "PASS",
  };
}

describe("E2E evidence summary", () => {
  it("serializes the whitelisted gate evidence independently of the diagnostic journal", () => {
    const written = JSON.parse(serializeEvidenceSummary(validSummary()));
    expect(written.fullThreadId).toBe("01a03e8f-aa9d-77c1-b787-c2a9bd89c8e9");
    expect(written.sourceLanes).toHaveLength(2);
    expect(written.authoritativeTokens[0].totalTokens).toBe(312_607);
  });

  it("rejects raw prompt, reasoning, and unknown content fields", () => {
    expect(() => validateEvidenceSummary({ ...validSummary(), prompt: "secret" })).toThrow(/unsupported field/i);
    expect(() => validateEvidenceSummary({ ...validSummary(), reasoning: "secret" })).toThrow(/unsupported field/i);
    expect(() => validateEvidenceSummary({ ...validSummary(), rawDiagnostic: "secret" })).toThrow(/unsupported field/i);
  });

  it("rejects unverifiable source lanes and cumulative Token values", () => {
    expect(() => validateEvidenceSummary({
      ...validSummary(),
      sourceLanes: [{ sourceKind: "guessed-source", temporalClass: "LIVE" }],
    })).toThrow(/sourceKind/i);
    expect(() => validateEvidenceSummary({
      ...validSummary(),
      authoritativeTokens: [{ threadId: "thread", totalTokens: -1, provenance: "fixture" }],
    })).toThrow(/totalTokens/i);
  });

  it("validates the committed Phase 2.4 C summary without raw conversation content", () => {
    const summary = validateEvidenceSummary(phase24C);
    const text = JSON.stringify(phase24C);

    expect(summary.result).toBe("PASS");
    expect(summary.fullThreadId).toBe("01a03e8f-aa9d-77c1-b787-c2a9bd89c8e9");
    expect(text).not.toMatch(/prompt|reasoning|rawDiagnostic/i);
  });
});
