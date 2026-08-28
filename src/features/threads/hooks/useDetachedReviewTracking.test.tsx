// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  loadDetachedReviewLinks,
  STORAGE_KEY_DETACHED_REVIEW_LINKS,
} from "@threads/utils/threadStorage";
import { useDetachedReviewTracking } from "./useDetachedReviewTracking";

describe("useDetachedReviewTracking deletion cleanup", () => {
  beforeEach(() => window.localStorage.clear());

  it("removes links touching confirmed deleted identities and preserves unrelated workspaces", () => {
    window.localStorage.setItem(
      STORAGE_KEY_DETACHED_REVIEW_LINKS,
      JSON.stringify({ "ws-2": { "other-child": "other-parent" } }),
    );
    const { result } = renderHook(() =>
      useDetachedReviewTracking({
        activeThreadId: null,
        dispatch: vi.fn(),
        recordThreadActivity: vi.fn(),
        safeMessageActivity: vi.fn(),
        threadsByWorkspace: {},
        threadParentById: {},
        updateThreadParent: vi.fn(),
      }),
    );
    act(() => {
      result.current.registerDetachedReviewChild("ws-1", "root", "child");
      result.current.registerDetachedReviewChild("ws-1", "keep-parent", "keep-child");
      result.current.forgetDetachedReviewThreads("ws-1", ["root", "child"]);
    });

    expect(loadDetachedReviewLinks()).toEqual({
      "ws-1": { "keep-child": "keep-parent" },
      "ws-2": { "other-child": "other-parent" },
    });
  });
});
