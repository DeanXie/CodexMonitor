import { describe, expect, it, vi } from "vitest";
import { createCreationAction, createFirstTurnAction } from "./creationAction";

describe("creation action intent boundary", () => {
  it("creates one creation id and obtains the process epoch once for repeated callbacks", async () => {
    const getProcessEpoch = vi.fn().mockResolvedValue("epoch-1");
    const createId = vi.fn().mockReturnValue("creation-1");
    const action = createCreationAction({ getProcessEpoch, createId });

    await expect(Promise.all([action.creationIntent, action.creationIntent])).resolves.toEqual([
      { id: "creation-1", processEpoch: "epoch-1" },
      { id: "creation-1", processEpoch: "epoch-1" },
    ]);
    expect(createId).toHaveBeenCalledTimes(1);
    expect(getProcessEpoch).toHaveBeenCalledTimes(1);
  });

  it("makes an explicit first-turn id while retaining its creation intent epoch", async () => {
    const getProcessEpoch = vi.fn().mockResolvedValue("epoch-1");
    const createId = vi.fn()
      .mockReturnValueOnce("creation-1")
      .mockReturnValueOnce("turn-1");
    const creation = createCreationAction({ getProcessEpoch, createId });
    const firstTurn = createFirstTurnAction(creation, { createId });

    await expect(firstTurn.turnIntent).resolves.toEqual({ id: "turn-1", processEpoch: "epoch-1" });
    await expect(firstTurn.creationIntent).resolves.toEqual({ id: "creation-1", processEpoch: "epoch-1" });
    expect(getProcessEpoch).toHaveBeenCalledTimes(1);
  });
});
