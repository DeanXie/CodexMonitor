import { describe, expect, it } from "vitest";
import {
  collectThreadDeletionSubtree,
  isThreadDeletionBlocked,
} from "./threadDeletion";

describe("thread deletion safety", () => {
  it("collects a selected main thread and every known spawned descendant", () => {
    expect(
      collectThreadDeletionSubtree("main", {
        childA: "main",
        childB: "main",
        reviewer: "childA",
        other: "different-main",
      }),
    ).toEqual(new Set(["main", "childA", "childB", "reviewer"]));
  });

  it("blocks deletion when the target or one of its descendants is active", () => {
    const deletedIds = new Set(["main", "child"]);

    expect(
      isThreadDeletionBlocked(deletedIds, {
        child: { isProcessing: true },
      }, []),
    ).toBe(true);
    expect(
      isThreadDeletionBlocked(deletedIds, {}, [
        { threadId: "main", status: "waiting" },
      ]),
    ).toBe(true);
    expect(
      isThreadDeletionBlocked(deletedIds, {}, [
        { threadId: "main", status: "completed" },
      ]),
    ).toBe(false);
  });
});
