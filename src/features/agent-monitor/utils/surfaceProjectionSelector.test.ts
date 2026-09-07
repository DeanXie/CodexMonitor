import { describe, expect, it } from "vitest";

import type { GlobalSourceSnapshot, SurfaceProjectionObservation } from "../global-source/types";
import { selectSurfaceProjectionView } from "./surfaceProjectionSelector";

const base = {
  revision: 1,
  generatedAtMs: 1_000,
  workspaceCodexHomeIdentities: {},
  threads: [],
} satisfies GlobalSourceSnapshot;

function projection(
  surface: SurfaceProjectionObservation["key"]["surface"],
  state: SurfaceProjectionObservation["state"],
): SurfaceProjectionObservation {
  return {
    key: {
      threadKey: { codexHomeIdentity: "home-1", threadId: "thread-1" },
      surface,
      projectionKind: surface === "CLI" ? "DISCOVERABILITY" : "CATALOG",
    },
    state,
    coverage: "COMPLETE",
    observedAt: 1_000,
    provenance: ["fixture"],
    diagnostics: [],
    reconciliationState: "NOT_REQUIRED",
    actionCapability: "OBSERVE_ONLY",
    membershipExpectation: "OPTIONAL",
  };
}

describe("selectSurfaceProjectionView", () => {
  it("cli_discoverability_is_independent_from_desktop_projection", () => {
    const view = selectSurfaceProjectionView({
      ...base,
      surfaceProjections: [projection("DESKTOP", "ABSENT"), projection("CLI", "PRESENT")],
    });

    const exact = view.byThreadKey.get("home-1\u001fthread-1") ?? [];
    expect(exact.map((entry) => [entry.key.surface, entry.state])).toEqual([
      ["CLI", "PRESENT"],
      ["DESKTOP", "ABSENT"],
    ]);
  });

  it("missing_transport_field_is_backward_safe_and_does_not_invent_observations", () => {
    const view = selectSurfaceProjectionView(base);

    expect(view.byThreadKey.size).toBe(0);
    expect(view.issues).toEqual([]);
  });
});
