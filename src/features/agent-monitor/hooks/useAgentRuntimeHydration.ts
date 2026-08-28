import { useEffect } from "react";
import type { ThreadSummary } from "@/types";

import {
  buildAgentRuntimeHydrationRecords,
  type RuntimeProtocolRecord,
} from "../runtime";

type RuntimeHydrationThreadStatus = {
  isProcessing?: boolean;
  processingStartedAt?: number | null;
};

type AgentRuntimeHydrationOptions = {
  threadsByWorkspace: Record<string, ThreadSummary[]>;
  threadParentById: Record<string, string>;
  threadStatusById: Record<string, RuntimeHydrationThreadStatus | undefined>;
  activeTurnIdByThread: Record<string, string | null | undefined>;
  ingestRuntimeRecord: (record: RuntimeProtocolRecord, observedAtMs?: number) => void;
};

export function useAgentRuntimeHydration({
  threadsByWorkspace,
  threadParentById,
  threadStatusById,
  activeTurnIdByThread,
  ingestRuntimeRecord,
}: AgentRuntimeHydrationOptions) {
  useEffect(() => {
    const observedAtMs = Date.now();
    buildAgentRuntimeHydrationRecords({
      threadsByWorkspace,
      threadParentById,
      threadStatusById,
      activeTurnIdByThread,
      capturedAtMs: observedAtMs,
    }).forEach((record) => ingestRuntimeRecord(record, observedAtMs));
  }, [
    activeTurnIdByThread,
    ingestRuntimeRecord,
    threadParentById,
    threadStatusById,
    threadsByWorkspace,
  ]);
}
