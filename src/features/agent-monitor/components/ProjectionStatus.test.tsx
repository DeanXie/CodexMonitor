// @vitest-environment jsdom
import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import type { SurfaceProjectionObservation } from "../global-source/types";
import { ProjectionIssues, ProjectionStatusDetails } from "./ProjectionStatus";

afterEach(cleanup);

function observation(
  overrides: Partial<SurfaceProjectionObservation> = {},
): SurfaceProjectionObservation {
  return {
    key: {
      threadKey: { codexHomeIdentity: "home-1", threadId: "thread-1" },
      surface: "DESKTOP",
      projectionKind: "CATALOG",
    },
    state: "PRESENT",
    coverage: "COMPLETE",
    observedAt: 1_000,
    provenance: ["fixture"],
    diagnostics: [],
    reconciliationState: "NOT_REQUIRED",
    actionCapability: "OBSERVE_ONLY",
    membershipExpectation: "OPTIONAL",
    ...overrides,
  };
}

describe("ProjectionStatusDetails", () => {
  it("canonical_present_desktop_absent_does_not_render_deleted", () => {
    render(<ProjectionStatusDetails observations={[observation({ state: "ABSENT" })]} />);

    expect(screen.getByText("Not present in this surface")).toBeTruthy();
    expect(screen.queryByText(/thread (missing|deleted)/i)).toBeNull();
  });

  it("desktop_stale_projection_renders_stale_not_active", () => {
    render(<ProjectionStatusDetails observations={[observation({
      state: "STALE",
      reconciliationState: "PENDING",
    })]} />);

    expect(screen.getByText("Surface still references a deleted Thread")).toBeTruthy();
    expect(screen.queryByText(/^Active$/)).toBeNull();
  });

  it("unknown_projection_renders_unknown_not_absent", () => {
    render(<ProjectionStatusDetails observations={[observation({
      state: "UNKNOWN",
      coverage: "PARTIAL",
      reconciliationState: "UNKNOWN",
    })]} />);

    expect(screen.getByText("Not enough evidence")).toBeTruthy();
    expect(screen.queryByText("Not present in this surface")).toBeNull();
  });

  it("pending_reconciliation_does_not_claim_active_repair", () => {
    render(<ProjectionStatusDetails observations={[observation({
      state: "STALE",
      reconciliationState: "PENDING",
      actionCapability: "OBSERVE_ONLY",
    })]} />);

    expect(screen.getByText("Waiting for Desktop to refresh")).toBeTruthy();
    expect(screen.queryByText(/repairing/i)).toBeNull();
  });

  it("observe_only_capability_has_no_repair_action", () => {
    render(<ProjectionStatusDetails observations={[observation()]} />);

    expect(screen.getByText("Observe only")).toBeTruthy();
    expect(screen.queryByRole("button", { name: /repair/i })).toBeNull();
  });

  it("reconciled_projection_renders_reconciled", () => {
    render(<ProjectionStatusDetails observations={[observation({
      state: "ABSENT",
      reconciliationState: "RECONCILED",
    })]} />);

    expect(screen.getByText("Reconciled")).toBeTruthy();
  });
});

describe("ProjectionIssues", () => {
  it("desktop_stale_orphan_does_not_create_agent_node", () => {
    render(<ProjectionIssues observations={[observation({
      state: "STALE",
      reconciliationState: "PENDING",
      diagnostics: ["DESKTOP_STALE_ORPHAN"],
    })]} />);

    const issues = screen.getByRole("region", { name: "Projection Issues" });
    expect(within(issues).getByText("Desktop retained a projection for a Thread that no longer exists.")).toBeTruthy();
    expect(screen.queryByRole("treeitem")).toBeNull();
    expect(screen.queryByRole("button", { name: /repair/i })).toBeNull();
  });
});
