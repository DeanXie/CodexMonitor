// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { WorkspaceInfo } from "@/types";
import { useMainAppMobileThreadRefresh } from "./useMainAppMobileThreadRefresh";

vi.mock("@threads/hooks/creationAction", () => ({
  createMonitorCreationAction: () => ({
    creationIntent: Promise.resolve({ processEpoch: "epoch", id: "mobile-intent" }),
  }),
}));

const workspace: WorkspaceInfo = {
  id: "ws-mobile",
  name: "Mobile workspace",
  path: "/tmp/mobile",
  connected: true,
  settings: { sidebarCollapsed: false },
};

describe("useMainAppMobileThreadRefresh", () => {
  it("uses one explicit creation action when refresh creates a missing thread", async () => {
    const startThreadForWorkspace = vi.fn().mockResolvedValue("thread-mobile");
    const refreshThread = vi.fn();
    const reconnectLive = vi.fn();
    const { result } = renderHook(() => useMainAppMobileThreadRefresh({
      activeWorkspace: workspace,
      activeThreadId: null,
      startThreadForWorkspace,
      refreshThread,
      reconnectLive,
    }));

    await act(async () => {
      result.current.handleMobileThreadRefresh();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(startThreadForWorkspace).toHaveBeenCalledWith(
      "ws-mobile",
      expect.objectContaining({ activate: true, creationAction: expect.any(Object) }),
    );
    expect(refreshThread).toHaveBeenCalledWith("ws-mobile", "thread-mobile");
    expect(reconnectLive).toHaveBeenCalledWith(
      "ws-mobile",
      "thread-mobile",
      { runResume: false },
    );
  });
});
