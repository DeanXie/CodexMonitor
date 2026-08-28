// @vitest-environment jsdom
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { WorkspaceInfo } from "@/types";
import { MainHeader } from "./MainHeader";

vi.mock("@tauri-apps/plugin-opener", () => ({
  revealItemInDir: vi.fn(),
}));

const workspace: WorkspaceInfo = {
  id: "workspace-1",
  name: "Workspace One",
  path: "C:\\workspace-one",
  connected: true,
  settings: { sidebarCollapsed: false },
};

describe("MainHeader", () => {
  it("distinguishes Global Home from the current Workspace Overview", () => {
    const onBackToHome = vi.fn();
    const onBackToWorkspace = vi.fn();

    render(
      <MainHeader
        workspace={workspace}
        openTargets={[]}
        openAppIconById={{}}
        selectedOpenAppId=""
        onSelectOpenAppId={vi.fn()}
        branchName="main"
        branches={[]}
        onCheckoutBranch={vi.fn()}
        onCreateBranch={vi.fn()}
        onBackToHome={onBackToHome}
        onBackToWorkspace={onBackToWorkspace}
        onToggleTerminal={vi.fn()}
        isTerminalOpen={false}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Home" }));
    fireEvent.click(screen.getByRole("button", { name: "Workspace" }));

    expect(onBackToHome).toHaveBeenCalledTimes(1);
    expect(onBackToWorkspace).toHaveBeenCalledTimes(1);
  });

  it("opens and closes the in-instance Agent Monitor panel", () => {
    const onToggleAgentMonitor = vi.fn();
    const { rerender } = render(
      <MainHeader
        workspace={workspace}
        openTargets={[]}
        openAppIconById={{}}
        selectedOpenAppId=""
        onSelectOpenAppId={vi.fn()}
        branchName="main"
        branches={[]}
        onCheckoutBranch={vi.fn()}
        onCreateBranch={vi.fn()}
        onToggleAgentMonitor={onToggleAgentMonitor}
        agentMonitorOpen={false}
        onToggleTerminal={vi.fn()}
        isTerminalOpen={false}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Open Agent Monitor" }));
    expect(onToggleAgentMonitor).toHaveBeenCalledTimes(1);

    rerender(
      <MainHeader
        workspace={workspace}
        openTargets={[]}
        openAppIconById={{}}
        selectedOpenAppId=""
        onSelectOpenAppId={vi.fn()}
        branchName="main"
        branches={[]}
        onCheckoutBranch={vi.fn()}
        onCreateBranch={vi.fn()}
        onToggleAgentMonitor={onToggleAgentMonitor}
        agentMonitorOpen
        onToggleTerminal={vi.fn()}
        isTerminalOpen={false}
      />,
    );
    expect(screen.getByRole("button", { name: "Close Agent Monitor" })).toBeTruthy();
  });
});
