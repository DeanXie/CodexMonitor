// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useDeleteThreadPrompt } from "./useDeleteThreadPrompt";

const threadsByWorkspace = {
  "ws-1": [
    { id: "main", name: "Scenario A", updatedAt: 2 },
    { id: "child", name: "Sub agent", updatedAt: 1 },
    { id: "other", name: "Keep me", updatedAt: 0 },
  ],
};

describe("useDeleteThreadPrompt", () => {
  it("does not delete when confirmation is cancelled", () => {
    const deleteThread = vi.fn();
    const { result } = renderHook(() =>
      useDeleteThreadPrompt({
        threadsByWorkspace,
        threadParentById: { child: "main" },
        threadStatusById: {},
        runtimeTurns: [],
        deleteThread,
        onDeleted: vi.fn(),
      }),
    );

    act(() => result.current.requestDelete("ws-1", "main"));
    expect(result.current.prompt?.title).toBe("Scenario A");
    act(() => result.current.cancelDelete());
    expect(deleteThread).not.toHaveBeenCalled();
    expect(result.current.prompt).toBeNull();
  });

  it("deletes an idle subtree and reports every known descendant", async () => {
    const deleteThread = vi.fn().mockResolvedValue(undefined);
    const onDeleted = vi.fn();
    const { result } = renderHook(() =>
      useDeleteThreadPrompt({
        threadsByWorkspace,
        threadParentById: { child: "main" },
        threadStatusById: {},
        runtimeTurns: [],
        deleteThread,
        onDeleted,
      }),
    );

    act(() => result.current.requestDelete("ws-1", "main"));
    await act(async () => result.current.confirmDelete());

    expect(deleteThread).toHaveBeenCalledWith(
      "ws-1",
      "main",
      ["child"],
      expect.stringMatching(/^[0-9a-f-]{36}$/),
    );
    expect(onDeleted).toHaveBeenCalledWith(
      "ws-1",
      new Set(["main", "child"]),
    );
    expect(result.current.prompt).toBeNull();
  });

  it.each(["running", "waiting"] as const)(
    "prevents a %s runtime from being deleted even after the prompt opened",
    async (status) => {
      const deleteThread = vi.fn();
      const { result, rerender } = renderHook(
        ({ runtimeTurns }) =>
          useDeleteThreadPrompt({
            threadsByWorkspace,
            threadParentById: { child: "main" },
            threadStatusById: {},
            runtimeTurns,
            deleteThread,
            onDeleted: vi.fn(),
          }),
        { initialProps: { runtimeTurns: [] as Array<{ threadId: string; status: string }> } },
      );

      act(() => result.current.requestDelete("ws-1", "main"));
      rerender({ runtimeTurns: [{ threadId: "child", status }] });
      await act(async () => result.current.confirmDelete());

      expect(deleteThread).not.toHaveBeenCalled();
      expect(result.current.prompt?.blocked).toBe(true);
    },
  );

  it("keeps the prompt and local state intact when the server rejects deletion", async () => {
    const deleteThread = vi.fn().mockRejectedValue(new Error("owned elsewhere"));
    const onDeleted = vi.fn();
    const { result } = renderHook(() =>
      useDeleteThreadPrompt({
        threadsByWorkspace,
        threadParentById: {},
        threadStatusById: {},
        runtimeTurns: [],
        deleteThread,
        onDeleted,
      }),
    );

    act(() => result.current.requestDelete("ws-1", "main"));
    await act(async () => result.current.confirmDelete());

    expect(onDeleted).not.toHaveBeenCalled();
    expect(result.current.prompt?.error).toBe("owned elsewhere");
  });
});
