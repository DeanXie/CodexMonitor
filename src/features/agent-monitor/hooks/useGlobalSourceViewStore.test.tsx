// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { subscribeGlobalSourceSnapshot } from "@services/events";
import { getGlobalSourceSnapshot } from "@services/tauri";
import type { GlobalSourceSnapshot } from "../global-source/types";
import { useGlobalSourceViewStore } from "./useGlobalSourceViewStore";

vi.mock("@services/tauri", () => ({
  getGlobalSourceSnapshot: vi.fn(),
}));

vi.mock("@services/events", () => ({
  subscribeGlobalSourceSnapshot: vi.fn(),
}));

function snapshot(revision: number, threadId: string): GlobalSourceSnapshot {
  return {
    revision,
    generatedAtMs: revision * 100,
    workspaceCodexHomeIdentities: {},
    threads: [{
      key: { codexHomeIdentity: "home-1", threadId },
      parentThreadKey: null,
      agentPath: null,
      currentTurn: null,
      lifecycle: null,
      observedModel: null,
      tokenSnapshot: null,
      authorityProvenance: null,
      liveLaneCount: 0,
      nearLiveLaneCount: 1,
      historicalLaneCount: 0,
    }],
  };
}

describe("useGlobalSourceViewStore", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("subscribes once, hydrates the initial snapshot, and ignores older revisions", async () => {
    let listener: (snapshot: GlobalSourceSnapshot) => void = () => {};
    const unsubscribe = vi.fn();
    vi.mocked(subscribeGlobalSourceSnapshot).mockImplementation((onSnapshot) => {
      listener = onSnapshot;
      return unsubscribe;
    });
    vi.mocked(getGlobalSourceSnapshot).mockResolvedValue(snapshot(2, "initial"));

    const { result, unmount } = renderHook(() => useGlobalSourceViewStore());

    await waitFor(() => expect(result.current.snapshot.revision).toBe(2));
    expect(subscribeGlobalSourceSnapshot).toHaveBeenCalledTimes(1);
    act(() => listener(snapshot(1, "older")));
    expect(result.current.snapshot.threads[0]?.key.threadId).toBe("initial");
    act(() => listener(snapshot(3, "newer")));
    expect(result.current.snapshot.threads[0]?.key.threadId).toBe("newer");

    unmount();
    expect(unsubscribe).toHaveBeenCalledTimes(1);
  });

  it("keeps a newer event that arrives before the initial command resolves", async () => {
    let listener: (snapshot: GlobalSourceSnapshot) => void = () => {};
    let resolveInitial!: (snapshot: GlobalSourceSnapshot) => void;
    vi.mocked(subscribeGlobalSourceSnapshot).mockImplementation((onSnapshot) => {
      listener = onSnapshot;
      return vi.fn();
    });
    vi.mocked(getGlobalSourceSnapshot).mockReturnValue(new Promise((resolve) => {
      resolveInitial = resolve;
    }));

    const { result } = renderHook(() => useGlobalSourceViewStore());
    act(() => listener(snapshot(4, "event-first")));
    act(() => resolveInitial(snapshot(3, "initial-late")));

    await waitFor(() => expect(result.current.snapshot.revision).toBe(4));
    expect(result.current.snapshot.threads[0]?.key.threadId).toBe("event-first");
  });

  it("removes a deleted Agent Monitor session when the canonical snapshot retires it", async () => {
    let listener: (snapshot: GlobalSourceSnapshot) => void = () => {};
    vi.mocked(subscribeGlobalSourceSnapshot).mockImplementation((onSnapshot) => {
      listener = onSnapshot;
      return vi.fn();
    });
    vi.mocked(getGlobalSourceSnapshot).mockResolvedValue(snapshot(1, "thread-deleted"));
    const { result } = renderHook(() => useGlobalSourceViewStore());
    await waitFor(() => expect(result.current.snapshot.threads).toHaveLength(1));

    act(() => listener({ ...snapshot(2, "ignored"), threads: [] }));

    expect(result.current.snapshot.revision).toBe(2);
    expect(result.current.snapshot.threads).toEqual([]);
  });
});
