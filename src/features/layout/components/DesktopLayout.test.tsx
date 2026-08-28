// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { DesktopLayout } from "./DesktopLayout";

const noop = vi.fn();

afterEach(cleanup);

function renderLayout(agentMonitorSplitNode: React.ReactNode) {
  return render(
    <DesktopLayout
      sidebarNode={<nav>Sidebar</nav>}
      updateToastNode={null}
      approvalToastsNode={null}
      errorToastsNode={null}
      homeNode={<div>Global Home</div>}
      showHome={false}
      showWorkspace
      topbarLeftNode={<div>Header</div>}
      centerMode="chat"
      preloadGitDiffs={false}
      splitChatDiffView={false}
      messagesNode={<div>Chat messages</div>}
      gitDiffViewerNode={<div>Diff viewer</div>}
      gitDiffPanelNode={<div>Git panel</div>}
      planPanelNode={<div>Plan panel</div>}
      composerNode={<div>Composer</div>}
      terminalDockNode={null}
      debugPanelNode={null}
      hasActivePlan={false}
      agentMonitorSplitNode={agentMonitorSplitNode}
      onSidebarResizeStart={noop}
      onChatDiffSplitPositionResizeStart={noop}
      onRightPanelResizeStart={noop}
      onPlanPanelResizeStart={noop}
    />,
  );
}

describe("DesktopLayout Agent Monitor split", () => {
  it("shows Chat and Agent Monitor together while replacing the normal right panel", () => {
    renderLayout(<aside aria-label="Agent Monitor split">Runtime tree</aside>);

    expect(screen.getByText("Chat messages")).toBeTruthy();
    expect(screen.getByRole("complementary", { name: "Agent Monitor split" })).toBeTruthy();
    expect(screen.queryByText("Git panel")).toBeNull();
    expect(screen.queryByText("Plan panel")).toBeNull();
  });

  it("restores the normal right panel after the Agent Monitor split closes", () => {
    renderLayout(null);

    expect(screen.getByText("Chat messages")).toBeTruthy();
    expect(screen.getByText("Git panel")).toBeTruthy();
    expect(screen.getByText("Plan panel")).toBeTruthy();
  });
});
