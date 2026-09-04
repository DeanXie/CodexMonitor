import { getCreationContext, type CreationIntentContext } from "@services/tauri";

export type CreationAction = {
  creationIntent: Promise<CreationIntentContext>;
};

export type FirstTurnAction = CreationAction & {
  turnIntent: Promise<CreationIntentContext>;
};

export type SendAction = {
  creationAction?: CreationAction;
  turnIntent?: Promise<CreationIntentContext>;
};

type CreationActionDependencies = {
  getProcessEpoch: () => Promise<string>;
  createId: () => string;
};

type FirstTurnActionDependencies = {
  createId: () => string;
};

// This is an explicit UI action token, not a deduplication key. Reusing this
// object across duplicate callbacks preserves its single id/epoch observation.
export function createCreationAction(
  { getProcessEpoch, createId }: CreationActionDependencies,
): CreationAction {
  const id = createId();
  const creationIntent = getProcessEpoch().then((processEpoch) => ({ id, processEpoch }));
  return { creationIntent };
}

export function createFirstTurnAction(
  creationAction: CreationAction,
  { createId }: FirstTurnActionDependencies,
): FirstTurnAction {
  const id = createId();
  return {
    creationIntent: creationAction.creationIntent,
    turnIntent: creationAction.creationIntent.then(({ processEpoch }) => ({ id, processEpoch })),
  };
}

export function createMonitorCreationAction(): CreationAction {
  return createCreationAction({
    getProcessEpoch: async () => (await getCreationContext()).processEpoch,
    createId: () => globalThis.crypto.randomUUID(),
  });
}

export function createMonitorFirstTurnAction(creationAction: CreationAction): FirstTurnAction {
  return createFirstTurnAction(creationAction, { createId: () => globalThis.crypto.randomUUID() });
}

export function createMonitorSendAction(creationAction?: CreationAction): SendAction {
  if (!creationAction) {
    return {};
  }
  const id = globalThis.crypto.randomUUID();
  return {
    creationAction,
    turnIntent: creationAction.creationIntent.then(({ processEpoch }) => ({ id, processEpoch })),
  };
}
