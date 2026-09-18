import { describe, expect, it } from "vitest";
import { assessProjectionEventDelivery, buildProjectionStatusModel } from "./projectionStatusModel";
import type { ProjectionFreshnessQuerySnapshot, RemoteHostAvailabilitySnapshot } from "@/types";

const fixtures = import.meta.glob(
  "../../../../docs/fixtures/phase-3-5-final-acceptance/*.json",
  { eager: true, import: "default" },
) as Record<string, Record<string, unknown>>;

function fixture(name: string) {
  const entry = Object.entries(fixtures).find(([path]) => path.endsWith(`/${name}`));
  expect(entry, `fixture ${name}`).toBeDefined();
  return entry?.[1] ?? {};
}

function freshness(status: "current" | "stale" | "not_hydrated"): ProjectionFreshnessQuerySnapshot {
  return {
    workspaceId: "workspace-sanitized",
    threadKey: { codexHomeIdentity: "home-sanitized", threadId: "thread-sanitized" },
    coverages: ["workspace_catalog", "thread_catalog", "thread_detail", "observation_snapshot"].map(
      (coverage) => ({
        coverage,
        status,
        generations: {
          daemonProcessGeneration: "daemon-a",
          remoteTransportGeneration: "transport-a",
          workspaceSessionGeneration: "session-a",
          appServerConnectionGeneration: "connection-a",
        },
        source: status === "current" ? "thread_read" : null,
        observedAt: status === "current" ? 10 : null,
        hydratedAt: status === "current" ? 10 : null,
      }),
    ) as ProjectionFreshnessQuerySnapshot["coverages"],
  };
}

function availability(transport: "CONNECTED" | "DISCONNECTED"): RemoteHostAvailabilitySnapshot {
  return {
    targetId: "host-sanitized",
    expectedRemoteHostIdentity: "host-a",
    observedRemoteHostIdentity: "host-a",
    attemptId: 1,
    transport,
    auth: transport === "CONNECTED" ? "AUTHENTICATED" : "UNKNOWN",
    daemon: transport === "CONNECTED" ? "AVAILABLE" : "UNKNOWN",
    runtime: { workspaceId: "workspace-sanitized", state: transport === "CONNECTED" ? "READY" : "UNKNOWN" },
    observedAt: 10,
    lastSuccessfulHandshakeAt: 10,
    lastRuntimeReadyAt: 10,
    daemonProcessGeneration: "daemon-a",
    diagnostics: [],
  };
}

describe("Phase 3.5 final integration acceptance", () => {
  it("freezes authority and zero replay contracts", () => {
    const authority = fixture("authority-contract.json");
    expect((authority.identityHierarchy as string[]).length).toBe(7);
    expect(authority.mutationPolicy).toEqual({
      resumeThread: { retry: 0, replay: 0 },
      approvalDecision: { retry: 0, replay: 0 },
      threadDelete: { retry: 0, replay: 0 },
      upstreamUnsubscribe: { retry: 0, replay: 0 },
      forceTakeover: false,
    });
  });

  it("keeps stale and unavailable projections honest", () => {
    const stale = buildProjectionStatusModel({ freshness: freshness("stale"), availability: availability("CONNECTED") });
    const unavailable = buildProjectionStatusModel({ freshness: freshness("current"), availability: availability("DISCONNECTED") });
    expect(stale.primary).toBe("stale");
    expect(stale.claimsThreadAbsent).toBe(false);
    expect(unavailable.primary).toBe("stale");
    expect(unavailable.claimsThreadAbsent).toBe(false);
  });

  it("rejects stale generation delivery without changing shared truth", () => {
    expect(assessProjectionEventDelivery({ generationMatches: false, order: "current" })).toEqual({
      apply: false,
      invalidateAs: "stale",
    });
  });

  it("contains no forbidden ownership schema fields", () => {
    const serialized = JSON.stringify(fixture("fixture-family-manifest.json"));
    expect(serialized.toLowerCase()).not.toMatch(
      /remoteclientidentity|clientowner|writerowner|subscriptionowner|approvalowner|deleteowner|projectionowner|recoveryowner|leaseid|forcetakeover|recoverygeneration|eventgeneration/,
    );
  });
});
