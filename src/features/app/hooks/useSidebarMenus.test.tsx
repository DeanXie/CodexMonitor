/** @vitest-environment jsdom */
import type { MouseEvent as ReactMouseEvent } from "react";
import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { WorkspaceInfo } from "../../../types";
import { useSidebarMenus } from "./useSidebarMenus";
import { fileManagerName } from "../../../utils/platformPaths";

const menuNew = vi.hoisted(() =>
  vi.fn(async ({ items }) => ({ popup: vi.fn(), items })),
);
const menuItemNew = vi.hoisted(() => vi.fn(async (options) => options));
const predefinedMenuItemNew = vi.hoisted(() => vi.fn(async (options) => options));

vi.mock("@tauri-apps/api/menu", () => ({
  Menu: { new: menuNew },
  MenuItem: { new: menuItemNew },
  PredefinedMenuItem: { new: predefinedMenuItemNew },
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ scaleFactor: () => 1 }),
}));

vi.mock("@tauri-apps/api/dpi", () => ({
  LogicalPosition: class LogicalPosition {
    x: number;
    y: number;
    constructor(x: number, y: number) {
      this.x = x;
      this.y = y;
    }
  },
}));

const revealItemInDir = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/plugin-opener", () => ({
  revealItemInDir: (...args: unknown[]) => revealItemInDir(...args),
}));

vi.mock("../../../services/toasts", () => ({
  pushErrorToast: vi.fn(),
}));

describe("useSidebarMenus", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });
  it("keeps Archive separate and adds a guarded destructive Delete action", async () => {
    const onArchiveThread = vi.fn();
    const onPermanentlyDeleteThread = vi.fn();
    const { result } = renderHook(() =>
      useSidebarMenus({
        onDeleteThread: onArchiveThread,
        onPermanentlyDeleteThread,
        isThreadDeleteBlocked: () => true,
        onSyncThread: vi.fn(),
        onPinThread: vi.fn(),
        onUnpinThread: vi.fn(),
        isThreadPinned: () => false,
        onRenameThread: vi.fn(),
        onReloadWorkspaceThreads: vi.fn(),
        onDeleteWorkspace: vi.fn(),
        onDeleteWorktree: vi.fn(),
      }),
    );
    const event = {
      preventDefault: vi.fn(),
      stopPropagation: vi.fn(),
      clientX: 12,
      clientY: 34,
    } as unknown as ReactMouseEvent;

    await result.current.showThreadMenu(event, "ws-1", "thread-1", false);

    const items = menuNew.mock.calls[menuNew.mock.calls.length - 1]?.[0].items;
    const archiveItem = items.find((item: { text?: string }) => item.text === "Archive");
    const deleteItem = items.find((item: { text?: string }) => item.text === "Delete");
    expect(predefinedMenuItemNew).toHaveBeenCalledWith({ item: "Separator" });
    expect(deleteItem.enabled).toBe(false);
    archiveItem.action();
    deleteItem.action();
    expect(onArchiveThread).toHaveBeenCalledWith("ws-1", "thread-1");
    expect(onPermanentlyDeleteThread).not.toHaveBeenCalled();
  });

  it("adds a show in file manager option for worktrees", async () => {
    const onDeleteThread = vi.fn();
    const onSyncThread = vi.fn();
    const onPinThread = vi.fn();
    const onUnpinThread = vi.fn();
    const isThreadPinned = vi.fn(() => false);
    const onRenameThread = vi.fn();
    const onReloadWorkspaceThreads = vi.fn();
    const onDeleteWorkspace = vi.fn();
    const onDeleteWorktree = vi.fn();

    const { result } = renderHook(() =>
      useSidebarMenus({
        onDeleteThread,
        onSyncThread,
        onPinThread,
        onUnpinThread,
        isThreadPinned,
        onRenameThread,
        onReloadWorkspaceThreads,
        onDeleteWorkspace,
        onDeleteWorktree,
      }),
    );

    const worktree: WorkspaceInfo = {
      id: "worktree-1",
      name: "feature/test",
      path: "/tmp/worktree-1",
      kind: "worktree",
      connected: true,
      settings: {
        sidebarCollapsed: false,
        worktreeSetupScript: "",
      },
      worktree: { branch: "feature/test" },
    };

    const event = {
      preventDefault: vi.fn(),
      stopPropagation: vi.fn(),
      clientX: 12,
      clientY: 34,
    } as unknown as ReactMouseEvent;

    await result.current.showWorktreeMenu(event, worktree);

    const menuArgs = menuNew.mock.calls[0]?.[0];
    const revealItem = menuArgs.items.find(
      (item: { text: string }) => item.text === `Show in ${fileManagerName()}`,
    );

    expect(revealItem).toBeDefined();
    await revealItem.action();
    expect(revealItemInDir).toHaveBeenCalledWith("/tmp/worktree-1");
  });
});
