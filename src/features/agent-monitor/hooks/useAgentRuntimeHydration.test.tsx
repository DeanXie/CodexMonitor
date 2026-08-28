// @vitest-environment jsdom
import { renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { selectAgentMonitorRuntimeView } from "../utils/agentRuntimeSelector";
import { useAgentRuntimeHydration } from "./useAgentRuntimeHydration";
import { useAgentRuntimeStore } from "./useAgentRuntimeStore";

describe("useAgentRuntimeHydration", () => {
  it("keeps catch-up active while Agent Monitor is not mounted", async () => {
    const { result, rerender } = renderHook(
      ({ page, isProcessing }: { page: "chat" | "monitor"; isProcessing: boolean }) => {
        const store = useAgentRuntimeStore();
        useAgentRuntimeHydration({
          threadsByWorkspace: {
            workspace: [{ id: "existing-thread", name: "Existing", updatedAt: 2 }],
          },
          threadParentById: {},
          threadStatusById: {
            "existing-thread": { isProcessing, processingStartedAt: 1_000 },
          },
          activeTurnIdByThread: { "existing-thread": "existing-turn" },
          ingestRuntimeRecord: store.ingestRuntimeRecord,
        });
        return { page, runtimeState: store.runtimeState };
      },
      { initialProps: { page: "chat" as "chat" | "monitor", isProcessing: true } },
    );

    await waitFor(() => {
      expect(selectAgentMonitorRuntimeView(result.current.runtimeState, 4_000).threads)
        .toHaveLength(1);
    });
    rerender({ page: "monitor", isProcessing: true });

    expect(selectAgentMonitorRuntimeView(result.current.runtimeState, 4_000).roots[0])
      .toMatchObject({ threadId: "existing-thread", status: "running" });
  });
});
