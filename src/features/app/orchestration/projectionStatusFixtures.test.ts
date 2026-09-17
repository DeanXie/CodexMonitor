import { describe, expect, it } from "vitest";

const fixtureModules = import.meta.glob(
  "../../../../src-tauri/tests/fixtures/phase-3-5-4d-offline-stale-ui/*.json",
  { eager: true, import: "default" },
) as Record<string, Record<string, unknown>>;
const expectedFixtures = [
  "client-a-stale-client-b-current.json",
  "confirmed-delete-stale-sidebar.json",
  "current-ui.json",
  "daemon-restart-stale-cache.json",
  "delete-outcome-unknown.json",
  "duplicate-event.json",
  "hydrating-ui.json",
  "known-event-gap.json",
  "mixed-coverage.json",
  "multi-client-convergence.json",
  "out-of-order-event.json",
  "reconnect-stale-cache.json",
  "stale-approval.json",
  "stale-cache.json",
  "unavailable-transport.json",
  "unavailable-workspace.json",
  "unknown-projection.json",
];

describe("Phase 3.5.4d compatibility fixtures", () => {
  it("freezes the sanitized offline, stale, gap, and multi-client scenarios", () => {
    const fixtureNames = Object.keys(fixtureModules)
      .map((path) => path.split("/").pop() ?? "")
      .sort();
    expect(fixtureNames).toEqual(expectedFixtures);
    for (const file of expectedFixtures) {
      const entry = Object.entries(fixtureModules).find(([path]) => path.endsWith(`/${file}`));
      const fixture = entry?.[1];
      expect(typeof fixture?.scenario).toBe("string");
    }
  });

  it("contains no client identity, ownership, lease, or mutation replay authority", () => {
    const serialized = JSON.stringify(fixtureModules).toLowerCase();
    expect(serialized).not.toContain("remoteclientidentity");
    expect(serialized).not.toMatch(/projectionowner|recoveryowner|leaseid|clientprojectionauthority|uiauthority/);
    expect(serialized).not.toContain("auto takeover");
  });
});
