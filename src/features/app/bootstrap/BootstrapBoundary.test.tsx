// @vitest-environment jsdom
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { BootstrapBoundary } from "./BootstrapBoundary";
import * as tauri from "@services/tauri";

vi.mock("@services/tauri", async () => {
  const actual = await vi.importActual<typeof import("@services/tauri")>("@services/tauri");
  return {
    ...actual,
    isMobileRuntime: vi.fn(),
    getBootstrapStatus: vi.fn(),
    activateFreshProfile: vi.fn(),
    recoverProfileActivation: vi.fn(),
  };
});

const status = (normalLoadAllowed: boolean): tauri.BootstrapStatus => ({
  inspection: {
    disposition: normalLoadAllowed ? "ready" : "legacy_migration_required",
    normalLoadAllowed,
    targetRoot: "X:/target",
    legacyRoot: "X:/legacy",
    reason: normalLoadAllowed ? "activated profile validated" : "migration required",
  },
  restartRequired: false,
});

describe("BootstrapBoundary", () => {
  afterEach(cleanup);

  beforeEach(() => {
    vi.mocked(tauri.isMobileRuntime).mockResolvedValue(false);
  });

  it("does not initialize business UI while activation is pending", async () => {
    vi.mocked(tauri.getBootstrapStatus).mockResolvedValue(status(false));
    render(
      <BootstrapBoundary>
        <div>business-ui</div>
      </BootstrapBoundary>,
    );
    await screen.findByText("migration required");
    expect(screen.queryByText("business-ui")).toBeNull();
  });

  it("renders business UI only after the backend permits normal loading", async () => {
    vi.mocked(tauri.getBootstrapStatus).mockResolvedValue(status(true));
    render(
      <BootstrapBoundary>
        <div>business-ui</div>
      </BootstrapBoundary>,
    );
    await waitFor(() => expect(screen.getByText("business-ui")).toBeTruthy());
  });

  it("keeps business UI gated until restart after an activation commit", async () => {
    vi.mocked(tauri.getBootstrapStatus).mockResolvedValue({
      ...status(true),
      restartRequired: true,
    });
    render(
      <BootstrapBoundary>
        <div>business-ui</div>
      </BootstrapBoundary>,
    );
    await screen.findByText("Activation complete. Restart the application.");
    expect(screen.queryByText("business-ui")).toBeNull();
  });
});
