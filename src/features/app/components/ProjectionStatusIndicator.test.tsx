// @vitest-environment jsdom
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ProjectionStatusIndicator } from "./ProjectionStatusIndicator";

describe("ProjectionStatusIndicator", () => {
  it("renders concise primary freshness with secondary diagnostics", () => {
    render(
      <ProjectionStatusIndicator
        model={{
          primary: "stale",
          primaryLabel: "Stale",
          coverages: {
            workspace_catalog: "current",
            thread_catalog: "current",
            thread_detail: "stale",
            observation_snapshot: "unavailable",
          },
          coverageDetails: "Thread catalog: Current · Thread detail: Stale · Observation snapshot: Unavailable",
          mixed: true,
          availability: { kind: "workspace_unavailable", label: "Workspace unavailable" },
          deliveryMode: "polling",
          retainsCachedProjection: true,
          claimsThreadAbsent: false,
          observationIsCurrent: false,
        }}
      />,
    );

    expect(screen.getByText("Stale")).toBeTruthy();
    expect(screen.getByText("Workspace unavailable")).toBeTruthy();
    expect(screen.getByText("Polling")).toBeTruthy();
    expect(screen.getByLabelText("Remote projection status").getAttribute("title"))
      .toContain("Thread detail: Stale");
  });
});
