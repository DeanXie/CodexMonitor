import { useCallback } from "react";
import type { DebugEntry } from "../../../types";

export type UpdateState = {
  stage: "disabled";
};

type UseUpdaterOptions = {
  enabled?: boolean;
  autoCheckOnMount?: boolean;
  onDebug?: (entry: DebugEntry) => void;
};

/**
 * The first DeanX daily-use distribution has no updater authority.
 *
 * Keep this inert compatibility surface while existing callers are migrated.
 * Inputs, persisted settings, and environment values cannot enable an updater
 * because this module deliberately imports no updater or process SDK.
 */
export function useUpdater(_options: UseUpdaterOptions = {}) {
  const noOp = useCallback(
    async (_options?: { announceNoUpdate?: boolean }) => undefined,
    [],
  );
  return {
    state: { stage: "disabled" } as UpdateState,
    startUpdate: noOp,
    checkForUpdates: noOp,
    dismiss: noOp,
  };
}
