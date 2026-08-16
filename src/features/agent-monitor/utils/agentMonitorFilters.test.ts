import { describe, expect, it } from "vitest";

import { filterAgentMonitorThreads } from "./agentMonitorFilters";

describe("filterAgentMonitorThreads", () => {
  it("limits a selected session to its root agent and all descendant agents", () => {
    const result = filterAgentMonitorThreads({
      threadsByWorkspace: {
        codex: [
          { id: "main", name: "Fix build", updatedAt: 1 },
          { id: "child", name: "Investigate", updatedAt: 2, isSubagent: true },
          { id: "grandchild", name: "Test", updatedAt: 3, isSubagent: true },
          { id: "other", name: "Another session", updatedAt: 4 },
        ],
        other: [{ id: "elsewhere", name: "Elsewhere", updatedAt: 5 }],
      },
      threadParentById: { child: "main", grandchild: "child" },
      workspaceId: "codex",
      sessionId: "child",
    });

    expect(result.map((thread) => thread.id)).toEqual(["main", "child", "grandchild"]);
  });

  it("limits all sessions to the selected workspace", () => {
    const result = filterAgentMonitorThreads({
      threadsByWorkspace: {
        codex: [{ id: "main", name: "Main", updatedAt: 1 }],
        other: [{ id: "other", name: "Other", updatedAt: 2 }],
      },
      threadParentById: {},
      workspaceId: "codex",
      sessionId: null,
    });

    expect(result.map((thread) => thread.id)).toEqual(["main"]);
  });
});
