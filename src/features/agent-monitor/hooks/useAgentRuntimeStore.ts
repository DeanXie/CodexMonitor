import { useCallback, useReducer } from "react";

import {
  applyRuntimeEvent,
  clearLiveRuntimeState,
  createRuntimeState,
  getLiveRuntimeClearState,
  normalizeRuntimeRecord,
  type RuntimeProtocolRecord,
} from "../runtime";

type RuntimeStoreAction =
  | {
      type: "ingest";
      record: RuntimeProtocolRecord;
      observedAtMs: number;
    }
  | { type: "clear" };

function runtimeStoreReducer(
  state: ReturnType<typeof createRuntimeState>,
  action: RuntimeStoreAction,
) {
  if (action.type === "clear") {
    return clearLiveRuntimeState(state);
  }
  return normalizeRuntimeRecord(action.record, action.observedAtMs).reduce(
    applyRuntimeEvent,
    state,
  );
}

export function useAgentRuntimeStore() {
  const [runtimeState, dispatch] = useReducer(
    runtimeStoreReducer,
    undefined,
    createRuntimeState,
  );

  const ingestRuntimeRecord = useCallback(
    (record: RuntimeProtocolRecord, observedAtMs = Date.now()) => {
      dispatch({ type: "ingest", record, observedAtMs });
    },
    [],
  );

  const clearLiveRuntime = useCallback(() => {
    dispatch({ type: "clear" });
  }, []);

  const clearState = getLiveRuntimeClearState(runtimeState);

  return {
    runtimeState,
    ingestRuntimeRecord,
    clearLiveRuntime,
    canClearLiveRuntime: clearState.canClear,
    activeRuntimeTurnCount: clearState.activeTurnIds.length,
  };
}
