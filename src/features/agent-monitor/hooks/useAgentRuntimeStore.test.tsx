// @vitest-environment jsdom
import { useState } from "react";
import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { WorkspaceInfo } from "@/types";
import { useWorkspaceSelection } from "@/features/workspaces/hooks/useWorkspaceSelection";
import type { RuntimeProtocolRecord } from "../runtime";
import { useAgentRuntimeStore } from "./useAgentRuntimeStore";

describe("useAgentRuntimeStore", () => {
  it("clears idle Runtime and continues ingesting new events", () => {
    const { result } = renderHook(() => useAgentRuntimeStore());
    const first: RuntimeProtocolRecord = {
      source: "EVENT",
      capturedAt: "2026-08-23T08:00:00.000Z",
      label: "thread/started",
      payload: {
        workspace_id: "workspace-live",
        message: {
          method: "thread/started",
          params: { thread: { id: "cleared-thread", status: { type: "idle" } } },
        },
      },
    };
    const next: RuntimeProtocolRecord = {
      source: "EVENT",
      capturedAt: "2026-08-23T08:00:01.000Z",
      label: "thread/started",
      payload: {
        workspace_id: "workspace-live",
        message: {
          method: "thread/started",
          params: { thread: { id: "new-thread", status: { type: "active" } } },
        },
      },
    };

    act(() => result.current.ingestRuntimeRecord(first, 1_000));
    const store = result.current as typeof result.current & {
      clearLiveRuntime?: () => void;
      canClearLiveRuntime?: boolean;
    };
    expect(store.clearLiveRuntime).toBeTypeOf("function");
    expect(store.canClearLiveRuntime).toBe(true);
    act(() => store.clearLiveRuntime!());
    expect(result.current.runtimeState).toEqual({
      threads: {},
      turns: {},
      assignments: {},
      pendingTurnRequestsByThread: {},
      appliedEventKeys: {},
    });

    act(() => result.current.ingestRuntimeRecord(next, 2_000));
    expect(result.current.runtimeState.threads["new-thread"].status?.value).toBe("active");
  });

  it("owns Runtime State and ingests normalized protocol records", () => {
    const { result } = renderHook(() => useAgentRuntimeStore());
    const record: RuntimeProtocolRecord = {
      source: "EVENT",
      capturedAt: "2026-08-23T08:00:00.000Z",
      label: "thread/settings/updated",
      payload: {
        workspace_id: "workspace-live",
        message: {
          method: "thread/settings/updated",
          emittedAtMs: 1_787_440_105_934,
          params: {
            threadId: "thread-live",
            threadSettings: { model: "gpt-5.6-sol" },
          },
        },
      },
    };

    act(() => result.current.ingestRuntimeRecord(record, 90_000));

    expect(result.current.runtimeState.threads["thread-live"].observedModel).toMatchObject({
      value: "gpt-5.6-sol",
      provenance: {
        serverTimeMs: 1_787_440_105_934,
        observedAtMs: 90_000,
      },
    });
  });

  it("keeps repeated protocol records idempotent", () => {
    const { result } = renderHook(() => useAgentRuntimeStore());
    const record: RuntimeProtocolRecord = {
      source: "EVENT",
      capturedAt: "2026-08-23T08:00:00.000Z",
      label: "thread/tokenUsage/updated",
      payload: {
        workspace_id: "workspace-live",
        message: {
          method: "thread/tokenUsage/updated",
          emittedAtMs: 1_787_440_106_000,
          params: {
            threadId: "thread-live",
            turnId: "turn-live",
            tokenUsage: {
              last: { inputTokens: 10, cachedInputTokens: 4, outputTokens: 2, totalTokens: 12 },
              total: { inputTokens: 10, cachedInputTokens: 4, outputTokens: 2, totalTokens: 12 },
            },
          },
        },
      },
    };

    act(() => {
      result.current.ingestRuntimeRecord(record, 90_000);
      result.current.ingestRuntimeRecord(record, 90_001);
    });

    expect(result.current.runtimeState.turns["turn-live"].tokenDelta.totalTokens).toBe(12);
  });

  it("retains accumulated Runtime State while page consumers mount and unmount", () => {
    const { result, rerender } = renderHook(
      ({ page }: { page: "chat" | "home" | "monitor" }) => ({
        page,
        store: useAgentRuntimeStore(),
      }),
      { initialProps: { page: "chat" as "chat" | "home" | "monitor" } },
    );
    const threadStarted: RuntimeProtocolRecord = {
      source: "EVENT",
      capturedAt: "2026-08-23T08:00:00.000Z",
      label: "thread/started",
      payload: {
        workspace_id: "workspace-live",
        message: {
          method: "thread/started",
          params: { thread: { id: "main", status: { type: "active" } } },
        },
      },
    };
    const assignmentStarted: RuntimeProtocolRecord = {
      source: "EVENT",
      capturedAt: "2026-08-23T08:00:01.000Z",
      label: "item/started",
      payload: {
        workspace_id: "workspace-live",
        message: {
          method: "item/started",
          params: {
            threadId: "main",
            turnId: "turn-main",
            item: {
              id: "assignment-hidden-page",
              type: "subAgentActivity",
              kind: "started",
              agentThreadId: "child-hidden-page",
              agentPath: "/root/child_hidden_page",
            },
          },
        },
      },
    };

    act(() => {
      result.current.store.ingestRuntimeRecord(threadStarted, 1_000);
      result.current.store.ingestRuntimeRecord(assignmentStarted, 2_000);
    });
    rerender({ page: "home" });
    rerender({ page: "monitor" });

    expect(result.current.store.runtimeState.threads.main.childThreadIds).toEqual([
      "child-hidden-page",
    ]);
    expect(Object.keys(result.current.store.runtimeState.assignments)).toEqual([
      "assignment-hidden-page",
    ]);
  });

  it("keeps one Runtime Store across internal Chat, Home, and Agent Monitor navigation", () => {
    const workspace: WorkspaceInfo = {
      id: "workspace-live",
      name: "Runtime Workspace",
      path: "C:\\runtime-workspace",
      connected: true,
      settings: { sidebarCollapsed: false },
    };
    const { result } = renderHook(() => {
      const store = useAgentRuntimeStore();
      const [activeWorkspaceId, setActiveWorkspaceId] = useState<string | null>(
        workspace.id,
      );
      const [showAgentMonitor, setShowAgentMonitor] = useState(false);
      const navigation = useWorkspaceSelection({
        workspaces: [workspace],
        isCompact: false,
        activeWorkspaceId,
        setActiveTab: () => {},
        setActiveWorkspaceId,
        updateWorkspaceSettings: async () => workspace,
        setCenterMode: () => {},
        setSelectedDiffPath: () => {},
      });
      return {
        ...store,
        activeWorkspaceId,
        showAgentMonitor,
        selectHome: navigation.selectHome,
        selectChat: () => navigation.selectWorkspace(workspace.id),
        openAgentMonitor: () => setShowAgentMonitor(true),
        backFromAgentMonitor: () => {
          setShowAgentMonitor(false);
          navigation.selectHome();
        },
      };
    });
    const threadStarted: RuntimeProtocolRecord = {
      source: "EVENT",
      capturedAt: "2026-08-23T08:00:00.000Z",
      label: "thread/started",
      payload: {
        workspace_id: workspace.id,
        message: {
          method: "thread/started",
          params: { thread: { id: "navigation-main", status: { type: "active" } } },
        },
      },
    };

    act(() => result.current.ingestRuntimeRecord(threadStarted, 1_000));
    act(() => result.current.selectHome());
    expect(result.current.activeWorkspaceId).toBeNull();
    act(() => result.current.selectChat());
    expect(result.current.activeWorkspaceId).toBe(workspace.id);
    act(() => result.current.selectHome());
    act(() => result.current.openAgentMonitor());
    expect(result.current.showAgentMonitor).toBe(true);
    act(() => result.current.backFromAgentMonitor());

    expect(result.current.showAgentMonitor).toBe(false);
    expect(result.current.activeWorkspaceId).toBeNull();
    expect(result.current.runtimeState.threads["navigation-main"].status?.value).toBe(
      "active",
    );
  });
});
