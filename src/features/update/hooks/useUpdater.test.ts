// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useUpdater } from "./useUpdater";

describe("useUpdater custom-distribution safety boundary", () => {
  it("keeps release-like automatic and manual update paths inert", async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    const { result } = renderHook(() =>
      useUpdater({ enabled: true, autoCheckOnMount: true }),
    );

    await act(async () => {
      await result.current.checkForUpdates({ announceNoUpdate: true });
      await result.current.startUpdate();
      await result.current.dismiss();
    });

    expect(result.current.state).toEqual({ stage: "disabled" });
    expect(fetchMock).not.toHaveBeenCalled();
    vi.unstubAllGlobals();
  });

  it("cannot be enabled by persisted or legacy caller options", async () => {
    const onDebug = vi.fn();
    const { result } = renderHook(() =>
      useUpdater({ enabled: true, autoCheckOnMount: true, onDebug }),
    );

    await act(async () => {
      await result.current.checkForUpdates({ announceNoUpdate: true });
      await result.current.startUpdate();
    });

    expect(result.current.state.stage).toBe("disabled");
    expect(onDebug).not.toHaveBeenCalled();
  });
});
