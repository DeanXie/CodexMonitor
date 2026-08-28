// @vitest-environment jsdom
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { AgentMonitorNode } from "../types";
import { AgentTreeNode } from "./AgentTreeNode";

const node: AgentMonitorNode = {
  threadId: "thread-main",
  name: "A very long main agent name",
  role: null,
  isSubagent: false,
  modelId: "gpt-5.6-terra",
  effort: null,
  status: "waiting",
  runtimeMs: 42_000,
  tokenUsage: {
    totalTokens: 126_457,
    inputTokens: 125_269,
    outputTokens: 1_188,
    cachedInputTokens: 97_280,
    reasoningOutputTokens: 0,
  },
  totalTokens: 126_457,
  source: {
    sourceKind: "codex-cli-rollout",
    temporalClass: "NEAR_LIVE",
    freshnessState: "fresh",
    ageMs: 420,
    observedAgeMs: 100,
    sourceTimestampMs: 1,
    observedTimestampMs: 2,
  },
  modelSource: {
    sourceKind: "codex-cli-rollout",
    temporalClass: "NEAR_LIVE",
    freshnessState: "settled",
    ageMs: 1_000,
    observedAgeMs: 500,
    sourceTimestampMs: 1,
    observedTimestampMs: 2,
    sourceInstanceId: "rollout-tail:fixture",
    sourceGeneration: "file:fixture",
    freshnessReason: "task complete",
  },
  children: [],
};

describe("AgentTreeNode", () => {
  it("groups overview and token metrics for the split-view compact card", () => {
    const { container } = render(<AgentTreeNode node={node} />);

    expect(container.querySelector(".agent-monitor-agent-overview")).not.toBeNull();
    expect(container.querySelector(".agent-monitor-token-grid")).not.toBeNull();
    expect(screen.getByText("gpt-5.6-terra").getAttribute("title")).toContain("rollout-tail:fixture");
    expect(screen.getByText("A very long main agent name").getAttribute("title")).toBe("A very long main agent name");
    expect(screen.getByText("126,457")).toBeTruthy();
    const source = screen.getByText("NEAR LIVE · 420 ms");
    expect(source.getAttribute("title")).toContain("generation: unavailable");
    expect(source.getAttribute("title")).toContain("observed timestamp: 2");
    expect(source.getAttribute("title")).toContain("source age: 420 ms");
    expect(source.getAttribute("title")).toContain("observed age: 100 ms");
  });
});
