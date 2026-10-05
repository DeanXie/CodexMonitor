// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { BootstrapBoundary } from "./BootstrapBoundary";
import type { BootstrapStatus } from "@services/tauri";

// Keep the component and production IPC wrappers real. Only the native boundary
// is simulated; the matching production permission gate is covered in Rust.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const freshStatus: BootstrapStatus = {
  inspection: {
    disposition: "fresh_activation_required",
    normalLoadAllowed: false,
    targetRoot: "X:/isolated/target",
    legacyRoot: "X:/isolated/legacy",
    reason: "fresh profile requires explicit activation",
  },
  restartRequired: false,
  runtimeState: "blocked",
};

describe("BootstrapBoundary pre-READY IPC contract", () => {
  afterEach(cleanup);

  beforeEach(() => {
    vi.clearAllMocks();
    // Deterministic pre-READY protocol double, not an always-successful service
    // mock. Unexpected/business commands are refused and activation stays gated.
    vi.mocked(invoke).mockImplementation(async (command, args) => {
      switch (command) {
        case "is_mobile_runtime":
          return false;
        case "get_bootstrap_status":
          return freshStatus;
        case "activate_fresh_profile":
          if ((args as { intent?: string })?.intent !== "create_fresh_profile") {
            throw new Error("explicit fresh intent required");
          }
          return {
            ...freshStatus,
            inspection: {
              ...freshStatus.inspection,
              disposition: "runtime_validation_required",
            },
            restartRequired: true,
          } satisfies BootstrapStatus;
        default:
          throw new Error("normal business IPC is unavailable until runtime validation succeeds");
      }
    });
  });

  it("queries fresh status through the safe platform probe without mounting business UI", async () => {
    render(<BootstrapBoundary><div>business-ui</div></BootstrapBoundary>);

    expect(await screen.findByRole("button", { name: "Create fresh profile" })).toBeTruthy();
    expect(vi.mocked(invoke).mock.calls.map(([command]) => command)).toEqual([
      "is_mobile_runtime",
      "get_bootstrap_status",
    ]);
    expect(screen.queryByText("business-ui")).toBeNull();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("uses explicit activation intent but never grants same-process business access", async () => {
    render(<BootstrapBoundary><div>business-ui</div></BootstrapBoundary>);
    fireEvent.click(await screen.findByRole("button", { name: "Create fresh profile" }));

    await screen.findByText("Activation complete. Restart the application.");
    expect(invoke).toHaveBeenCalledWith("activate_fresh_profile", {
      intent: "create_fresh_profile",
    });
    expect(screen.queryByText("business-ui")).toBeNull();
    expect(screen.queryByRole("button", { name: "Create fresh profile" })).toBeNull();
    expect(vi.mocked(invoke).mock.calls.map(([command]) => command)).toEqual([
      "is_mobile_runtime",
      "get_bootstrap_status",
      "activate_fresh_profile",
    ]);
  });

  it("does not bypass a rejected platform probe to initialize business UI", async () => {
    vi.mocked(invoke).mockRejectedValueOnce(
      "normal business IPC is unavailable until runtime validation succeeds",
    );
    render(<BootstrapBoundary><div>business-ui</div></BootstrapBoundary>);

    await waitFor(() => expect(screen.getByRole("alert").textContent).toBe(
      "normal business IPC is unavailable until runtime validation succeeds",
    ));
    expect(vi.mocked(invoke).mock.calls.map(([command]) => command)).toEqual([
      "is_mobile_runtime",
    ]);
    expect(screen.queryByRole("button", { name: "Create fresh profile" })).toBeNull();
    expect(screen.queryByText("business-ui")).toBeNull();
  });
});
