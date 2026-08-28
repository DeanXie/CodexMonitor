// @vitest-environment jsdom
import { useState } from "react";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { applyRuntimeRecords, createRuntimeState, type RuntimeProtocolRecord } from "../runtime";
import type { GlobalSourceSnapshot } from "../global-source/types";
import { AgentMonitorPage } from "./AgentMonitorPage";

vi.mock("@/features/home/hooks/useLocalUsage", () => ({
  useLocalUsage: () => ({
    snapshot: {
      updatedAt: 1,
      days: [],
      totals: {
        last7DaysTokens: 10_000,
        last30DaysTokens: 10_000,
        averageDailyTokens: 10_000,
        cacheHitRatePercent: 44,
        peakDay: "2026-08-23",
        peakDayTokens: 10_000,
      },
      topModels: [{ model: "historical-model", tokens: 10_000, sharePercent: 100 }],
      sessionLinked: true,
    },
  }),
}));

afterEach(cleanup);

const historicalSnapshot = {
  updatedAt: 1,
  days: [],
  totals: {
    last7DaysTokens: 10_000,
    last30DaysTokens: 10_000,
    averageDailyTokens: 10_000,
    cacheHitRatePercent: 44,
    peakDay: "2026-08-23",
    peakDayTokens: 10_000,
  },
  topModels: [{ model: "historical-model", tokens: 10_000, sharePercent: 100 }],
};

function runtimeWithLiveEvidence() {
  const records: RuntimeProtocolRecord[] = [
    {
      source: "EVENT",
      capturedAt: "2026-08-23T08:00:00.000Z",
      label: "thread/started",
      payload: {
        workspace_id: "codex",
        message: {
          method: "thread/started",
          params: { thread: { id: "main", status: { type: "active" }, createdAt: 100 } },
        },
      },
    },
    {
      source: "EVENT",
      capturedAt: "2026-08-23T08:00:01.000Z",
      label: "thread/settings/updated",
      payload: {
        workspace_id: "codex",
        message: {
          method: "thread/settings/updated",
          params: { threadId: "main", threadSettings: { model: "gpt-live" } },
        },
      },
    },
    {
      source: "EVENT",
      capturedAt: "2026-08-23T08:00:02.000Z",
      label: "thread/tokenUsage/updated",
      payload: {
        workspace_id: "codex",
        message: {
          method: "thread/tokenUsage/updated",
          params: {
            threadId: "main",
            turnId: "turn-main",
            tokenUsage: {
              last: { inputTokens: 20, cachedInputTokens: 8, outputTokens: 5, totalTokens: 25 },
              total: { inputTokens: 20, cachedInputTokens: 8, outputTokens: 5, totalTokens: 25 },
            },
          },
        },
      },
    },
  ];
  return applyRuntimeRecords(createRuntimeState(), records, 1_000);
}

function runtimeWithRootThreads(threadIds: string[]) {
  return applyRuntimeRecords(
    createRuntimeState(),
    threadIds.map((threadId, index): RuntimeProtocolRecord => ({
      source: "EVENT",
      capturedAt: `2026-08-23T08:00:0${index}.000Z`,
      label: "thread/started",
      payload: {
        workspace_id: "codex",
        message: {
          method: "thread/started",
          params: { thread: { id: threadId, status: { type: "idle" } } },
        },
      },
    })),
    1_000,
  );
}

describe("AgentMonitorPage", () => {
  it("shows an external CLI session as NEAR LIVE and filters it independently from Monitor LIVE", () => {
    const globalSourceSnapshot: GlobalSourceSnapshot = {
      revision: 1,
      generatedAtMs: 9_500,
      workspaceCodexHomeIdentities: { codex: "home-1" },
      threads: [{
        key: { codexHomeIdentity: "home-1", threadId: "cli-main-12345678" },
        parentThreadKey: null,
        agentPath: null,
        currentTurn: null,
        lifecycle: null,
        observedModel: null,
        tokenSnapshot: null,
        authorityProvenance: {
          sourceKind: "codex-cli-rollout",
          temporalClass: "NEAR_LIVE",
          sourceInstanceId: "tail:home-1",
          sourceGeneration: "rollout:1",
          sourceTimestampMs: 9_000,
          observedTimestampMs: 9_400,
          freshness: {
            state: "fresh",
            lastCompleteRecordObservedAtMs: 9_400,
            reason: "complete record",
          },
        },
        liveLaneCount: 0,
        nearLiveLaneCount: 1,
        historicalLaneCount: 0,
      }],
    };

    render(
      <AgentMonitorPage
        runtimeState={createRuntimeState()}
        globalSourceSnapshot={globalSourceSnapshot}
        localUsageSnapshot={historicalSnapshot}
        currentThreadId="monitor-chat"
        now={10_000}
      />,
    );

    expect(within(screen.getByLabelText("Session")).getByText(
      "CLI — Main Agent — cli-main",
    )).toBeTruthy();
    expect(screen.getByText("NEAR LIVE · 1s")).toBeTruthy();
    expect(screen.getByText("Current session not observed yet")).toBeTruthy();
    expect((screen.getByLabelText("Activity") as HTMLSelectElement).value).toBe("active-fresh");
    expect(within(screen.getByLabelText("Activity")).getAllByRole("option").map((option) => option.textContent)).toEqual([
      "Active / Fresh",
      "All",
      "Settled",
    ]);

    fireEvent.change(screen.getByLabelText("Source"), {
      target: { value: "monitor-live" },
    });
    expect(screen.getByText("No agent threads are available yet.")).toBeTruthy();
    expect(within(screen.getByLabelText("Session")).queryByText(
      "CLI — Main Agent — cli-main",
    )).toBeNull();
  });

  it("changes the activity summary semantics with the Activity filter", () => {
    const staleProvenance = {
      sourceKind: "codex-cli-rollout" as const,
      temporalClass: "NEAR_LIVE" as const,
      sourceInstanceId: "tail:home-1",
      sourceGeneration: "rollout:1",
      sourceTimestampMs: 1_000,
      observedTimestampMs: 9_000,
      freshness: {
        state: "stale" as const,
        lastCompleteRecordObservedAtMs: 9_000,
        reason: "rollout source timestamp is not recent",
      },
    };
    const globalSourceSnapshot: GlobalSourceSnapshot = {
      revision: 1,
      generatedAtMs: 9_000,
      workspaceCodexHomeIdentities: {},
      threads: [{
        key: { codexHomeIdentity: "home-1", threadId: "stale-waiting" },
        parentThreadKey: null,
        agentPath: null,
        currentTurn: null,
        lifecycle: { value: "waiting", provenance: staleProvenance },
        observedModel: null,
        tokenSnapshot: null,
        authorityProvenance: staleProvenance,
        liveLaneCount: 0,
        nearLiveLaneCount: 1,
        historicalLaneCount: 0,
      }],
    };

    render(
      <AgentMonitorPage
        runtimeState={createRuntimeState()}
        globalSourceSnapshot={globalSourceSnapshot}
        localUsageSnapshot={historicalSnapshot}
        now={10_000}
      />,
    );

    expect(screen.getByText("Active")).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Activity"), { target: { value: "all" } });
    const recordedActive = screen.getByText("Recorded Active").closest(".agent-monitor-summary-card");
    expect(recordedActive?.getAttribute("title")).toBe(
      "Includes stale unresolved agents whose last recorded lifecycle was Running or Waiting.",
    );
    expect(within(recordedActive as HTMLElement).getByText("1")).toBeTruthy();

    fireEvent.change(screen.getByLabelText("Activity"), { target: { value: "settled" } });
    expect(screen.queryByText("Active")).toBeNull();
    expect(screen.queryByText("Recorded Active")).toBeNull();
  });

  it("auto-selects the observed current Chat and does not override a manual choice", async () => {
    const currentId = "01a02fb4-1111-2222-3333-444444444444";
    const otherId = "01a02eee-1111-2222-3333-444444444444";
    const emptyRuntime = createRuntimeState();
    const observedRuntime = runtimeWithRootThreads([otherId, currentId]);
    const props = {
      localUsageSnapshot: historicalSnapshot,
      currentThreadId: currentId,
      threadTitlesById: {
        [currentId]: "场景 A 实时验证任务",
        [otherId]: "旧任务",
      },
      now: 6_000,
      variant: "split" as const,
    };
    const { rerender } = render(
      <AgentMonitorPage runtimeState={emptyRuntime} {...props} />,
    );

    expect(screen.getByText("Current session not observed yet")).toBeTruthy();
    expect((screen.getByLabelText("Session") as HTMLSelectElement).value).toBe("");

    rerender(<AgentMonitorPage runtimeState={observedRuntime} {...props} />);
    await waitFor(() => {
      expect((screen.getByLabelText("Session") as HTMLSelectElement).value).toBe(currentId);
    });
    const options = within(screen.getByLabelText("Session")).getAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual([
      "All Sessions",
      "● Current — 场景 A 实时验证任务 — 01a02fb4",
      "旧任务 — 01a02eee",
    ]);

    fireEvent.change(screen.getByLabelText("Session"), { target: { value: otherId } });
    rerender(<AgentMonitorPage runtimeState={observedRuntime} {...props} />);
    expect((screen.getByLabelText("Session") as HTMLSelectElement).value).toBe(otherId);
  });

  it("disables Clear Live Runtime while a turn is running or waiting", () => {
    const onClearLiveRuntime = vi.fn();
    render(
      <AgentMonitorPage
        runtimeState={runtimeWithLiveEvidence()}
        localUsageSnapshot={historicalSnapshot}
        canClearLiveRuntime={false}
        activeRuntimeTurnCount={1}
        onClearLiveRuntime={onClearLiveRuntime}
        now={6_000}
      />,
    );

    const clear = screen.getByRole("button", { name: "Clear Live Runtime" });
    expect((clear as HTMLButtonElement).disabled).toBe(true);
    expect(clear.getAttribute("title")).toContain("1 Runtime turn");
    fireEvent.click(clear);
    expect(onClearLiveRuntime).not.toHaveBeenCalled();
  });

  it("clears the Live view while preserving Historical model usage", async () => {
    function ClearHarness() {
      const [runtimeState, setRuntimeState] = useState(runtimeWithLiveEvidence);
      return (
        <AgentMonitorPage
          runtimeState={runtimeState}
          localUsageSnapshot={historicalSnapshot}
          currentThreadId="main"
          threadTitlesById={{ main: "Current task" }}
          workspaceOptions={[{ id: "codex", label: "CodexMonitor", path: "F:/AI/CodexMonitor" }]}
          canClearLiveRuntime
          activeRuntimeTurnCount={0}
          onClearLiveRuntime={() => setRuntimeState(createRuntimeState())}
          now={6_000}
        />
      );
    }
    render(<ClearHarness />);
    const history = screen.getByRole("region", { name: "Historical model usage" });
    expect(within(history).getByText("historical-model")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Clear Live Runtime" }).getAttribute("title"))
      .toContain("CLI/rollout sources are preserved");
    await waitFor(() => {
      expect((screen.getByLabelText("Session") as HTMLSelectElement).value).toBe("main");
    });
    expect(within(screen.getByLabelText("Session")).getAllByRole("option")).toHaveLength(2);

    fireEvent.click(screen.getByRole("button", { name: "Clear Live Runtime" }));

    const summary = screen.getByLabelText("Agent Monitor summary");
    expect(within(summary).getByText("Agents").nextElementSibling?.textContent).toBe("0");
    expect(within(summary).getByText("Active").nextElementSibling?.textContent).toBe("0");
    expect(within(summary).getAllByText("unavailable")).toHaveLength(2);
    expect(screen.getByText("No agent threads are available yet.")).toBeTruthy();
    expect(screen.getByText("Current session not observed yet")).toBeTruthy();
    expect(within(screen.getByLabelText("Session")).getAllByRole("option")).toHaveLength(1);
    expect(within(history).getByText("historical-model")).toBeTruthy();
  });

  it("renders live values only from Runtime and labels history separately", () => {
    render(
      <AgentMonitorPage
        runtimeState={runtimeWithLiveEvidence()}
        localUsageSnapshot={historicalSnapshot}
        workspaceOptions={[{ id: "codex", label: "CodexMonitor", path: "F:/AI/CodexMonitor" }]}
        now={6_000}
      />,
    );

    const tree = screen.getByRole("region", { name: "Live Agent Runtime" });
    expect(within(tree).getByText("gpt-live")).toBeTruthy();
    expect(within(tree).getByText("25")).toBeTruthy();
    expect(within(tree).queryByText("historical-model")).toBeNull();
    expect(screen.getByText("Root Thread Tokens")).toBeTruthy();
    const history = screen.getByRole("region", { name: "Historical model usage" });
    expect(within(history).getByText("historical-model")).toBeTruthy();
  });

  it("does not backfill missing live model or token from historical usage", () => {
    const runtimeState = applyRuntimeRecords(createRuntimeState(), [{
      source: "EVENT",
      capturedAt: "2026-08-23T08:00:00.000Z",
      label: "thread/started",
      payload: {
        workspace_id: "codex",
        message: {
          method: "thread/started",
          params: { thread: { id: "main", status: { type: "idle" } } },
        },
      },
    }], 1_000);

    render(
      <AgentMonitorPage
        runtimeState={runtimeState}
        localUsageSnapshot={historicalSnapshot}
        workspaceOptions={[{ id: "codex", label: "CodexMonitor", path: "F:/AI/CodexMonitor" }]}
        now={6_000}
      />,
    );

    const tree = screen.getByRole("region", { name: "Live Agent Runtime" });
    expect(within(tree).getAllByText("unavailable").length).toBeGreaterThanOrEqual(2);
    expect(within(tree).queryByText("historical-model")).toBeNull();
    expect(within(tree).queryByText("10,000")).toBeNull();
  });

  it("filters Runtime agents by workspace and root session", () => {
    const records: RuntimeProtocolRecord[] = [
      ["codex", "main"],
      ["codex", "child"],
      ["other", "other"],
    ].map(([workspaceId, threadId]) => ({
      source: "EVENT" as const,
      capturedAt: "2026-08-23T08:00:00.000Z",
      label: "thread/started",
      payload: {
        workspace_id: workspaceId,
        message: {
          method: "thread/started",
          params: { thread: { id: threadId, status: { type: "idle" } } },
        },
      },
    }));
    records.push({
      source: "EVENT",
      capturedAt: "2026-08-23T08:00:01.000Z",
      label: "item/started",
      payload: {
        workspace_id: "codex",
        message: {
          method: "item/started",
          params: {
            threadId: "main",
            turnId: "turn-main",
            item: {
              id: "assignment-child",
              type: "subAgentActivity",
              kind: "started",
              agentThreadId: "child",
              agentPath: "/root/child",
            },
          },
        },
      },
    });
    const runtimeState = applyRuntimeRecords(createRuntimeState(), records, 1_000);

    render(
      <AgentMonitorPage
        runtimeState={runtimeState}
        localUsageSnapshot={null}
        workspaceOptions={[
          { id: "codex", label: "CodexMonitor" },
          { id: "other", label: "Other" },
        ]}
        now={6_000}
      />,
    );

    fireEvent.change(screen.getByLabelText("Workspace"), { target: { value: "codex" } });
    fireEvent.change(screen.getByLabelText("Session"), { target: { value: "main" } });
    expect(screen.getByText(/Created:/)).toBeTruthy();
    expect(screen.getByText("child")).toBeTruthy();
    expect(screen.queryByText("Main Agent · other")).toBeNull();
  });

  it("renders a close control in split-panel mode", () => {
    const onClose = vi.fn();
    render(
      <AgentMonitorPage
        runtimeState={runtimeWithLiveEvidence()}
        localUsageSnapshot={historicalSnapshot}
        now={6_000}
        variant="split"
        onClose={onClose}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Close Agent Monitor" }));

    expect(onClose).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("main").classList.contains("is-split")).toBe(true);
  });
});
