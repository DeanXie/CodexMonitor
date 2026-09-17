import { useCallback, useEffect, useMemo, useState } from "react";
import type {
  AuthoritativeObservationSnapshot,
  ProjectionDeliveryGapEvidence,
  ProjectionFreshnessCoverage,
  ProjectionFreshnessQuerySnapshot,
  RemoteHostAvailabilitySnapshot,
  WorkspaceInfo,
} from "@/types";
import {
  subscribeAppServerEventGaps,
  subscribeProjectionFreshnessSnapshots,
} from "@services/events";
import {
  getAuthoritativeObservationSnapshot,
  getProjectionFreshness,
  getRemoteHostAvailability,
} from "@services/tauri";
import {
  buildProjectionStatusModel,
  isApprovalProjectionActionable,
} from "@app/orchestration/projectionStatusModel";
import type { ApprovalRequest } from "@/types";

const coverages: ProjectionFreshnessCoverage[] = [
  "workspace_catalog",
  "thread_catalog",
  "thread_detail",
  "observation_snapshot",
];

function unknownFreshness(
  workspaceId: string,
  threadId: string | null,
): ProjectionFreshnessQuerySnapshot {
  return {
    workspaceId,
    threadKey: threadId
      ? { codexHomeIdentity: "", threadId }
      : null,
    coverages: coverages.map((coverage) => ({
      coverage,
      status: "not_hydrated",
      generations: {
        daemonProcessGeneration: null,
        remoteTransportGeneration: null,
        workspaceSessionGeneration: null,
        appServerConnectionGeneration: null,
      },
      source: null,
      observedAt: null,
      hydratedAt: null,
    })),
  };
}

function gapHasBeenHydrated(
  gap: ProjectionDeliveryGapEvidence,
  snapshot: ProjectionFreshnessQuerySnapshot,
) {
  return gap.affectedCoverages.every((coverage) => {
    const evidence = snapshot.coverages.find(
      (candidate) => candidate.coverage === coverage,
    );
    return Boolean(
      evidence &&
        evidence.status === "current" &&
        evidence.generations.daemonProcessGeneration ===
          gap.daemonProcessGeneration &&
        evidence.generations.remoteTransportGeneration ===
          gap.remoteTransportGeneration &&
        (evidence.hydratedAt ?? -1) >= gap.observedAt,
    );
  });
}

export function useRemoteProjectionStatus({
  backendMode,
  activeWorkspace,
  activeThreadId,
  activeTargetId,
  deliveryMode,
  recoverWorkspace,
  approvals = [],
}: {
  backendMode: "local" | "remote";
  activeWorkspace: WorkspaceInfo | null;
  activeThreadId: string | null;
  activeTargetId: string | null;
  deliveryMode: "live" | "polling" | "disconnected";
  recoverWorkspace: (workspace: WorkspaceInfo) => Promise<unknown>;
  approvals?: ApprovalRequest[];
}) {
  const [freshness, setFreshness] = useState<ProjectionFreshnessQuerySnapshot | null>(null);
  const [availability, setAvailability] = useState<RemoteHostAvailabilitySnapshot | null>(null);
  const [observation, setObservation] = useState<AuthoritativeObservationSnapshot | null>(null);
  const [approvalObservations, setApprovalObservations] = useState<
    Record<
      string,
      {
        snapshot: AuthoritativeObservationSnapshot;
        freshness: "current" | "unknown";
      }
    >
  >({});
  const [gap, setGap] = useState<ProjectionDeliveryGapEvidence | null>(null);

  const approvalQueryKey = approvals
    .map((request) => {
      const threadId =
        typeof request.params?.threadId === "string"
          ? request.params.threadId
          : typeof request.params?.thread_id === "string"
            ? request.params.thread_id
            : "";
      return `${request.workspace_id}:${threadId}:${String(request.request_id)}`;
    })
    .sort()
    .join("|");

  const refresh = useCallback(async () => {
    if (backendMode !== "remote" || !activeWorkspace) {
      return;
    }
    const [freshnessResult, availabilityResult] = await Promise.allSettled([
      getProjectionFreshness(activeWorkspace.id, activeThreadId ?? undefined),
      getRemoteHostAvailability(),
    ]);
    if (freshnessResult.status === "fulfilled") {
      setFreshness(freshnessResult.value);
    }
    if (availabilityResult.status === "fulfilled") {
      setAvailability(
        availabilityResult.value.find((entry) => entry.targetId === activeTargetId) ??
        availabilityResult.value.find(
          (entry) => entry.runtime.workspaceId === activeWorkspace.id,
        ) ??
        null,
      );
    }
    if (activeThreadId) {
      try {
        setObservation(
          await getAuthoritativeObservationSnapshot(
            activeWorkspace.id,
            activeThreadId,
          ),
        );
      } catch {
        setObservation(null);
      }
    } else {
      setObservation(null);
    }
  }, [activeTargetId, activeThreadId, activeWorkspace, backendMode]);

  useEffect(() => {
    if (backendMode !== "remote" || !activeWorkspace) {
      setFreshness(null);
      setAvailability(null);
      setObservation(null);
      setApprovalObservations({});
      setGap(null);
      return;
    }
    void refresh();
  }, [activeWorkspace, backendMode, deliveryMode, refresh]);

  useEffect(
    () =>
      subscribeProjectionFreshnessSnapshots((snapshot) => {
        const approvalFreshness = snapshot.coverages.find(
          (coverage) => coverage.coverage === "observation_snapshot",
        );
        const matchingApprovals = approvals.filter((request) => {
          const threadId =
            typeof request.params?.threadId === "string"
              ? request.params.threadId
              : typeof request.params?.thread_id === "string"
                ? request.params.thread_id
                : null;
          return (
            request.workspace_id === snapshot.workspaceId &&
            threadId === snapshot.threadKey?.threadId
          );
        });
        if (approvalFreshness?.status === "current" && snapshot.threadKey) {
          void Promise.all(
            matchingApprovals.map(async (request) => {
              const current = await getAuthoritativeObservationSnapshot(
                request.workspace_id,
                snapshot.threadKey!.threadId,
              );
              return current.workspaceSessionGeneration ===
                approvalFreshness.generations.workspaceSessionGeneration &&
                current.appServerConnectionGeneration ===
                  approvalFreshness.generations.appServerConnectionGeneration
                ? ([`${request.workspace_id}:${snapshot.threadKey!.threadId}`, current] as const)
                : null;
            }),
          )
            .then((entries) => {
              setApprovalObservations((current) => ({
                ...current,
                ...Object.fromEntries(
                  entries
                    .filter((entry) => entry !== null)
                    .map(([key, value]) => [
                      key,
                      { snapshot: value, freshness: "current" as const },
                    ]),
                ),
              }));
            })
            .catch(() => undefined);
        }
        if (
          snapshot.workspaceId !== activeWorkspace?.id ||
          (activeThreadId && snapshot.threadKey?.threadId !== activeThreadId)
        ) {
          return;
        }
        setFreshness(snapshot);
        setGap((currentGap) =>
          currentGap && gapHasBeenHydrated(currentGap, snapshot)
            ? null
            : currentGap,
        );
      }),
    [activeThreadId, activeWorkspace?.id, approvalQueryKey],
  );

  useEffect(
    () =>
      subscribeAppServerEventGaps((nextGap) => {
        setApprovalObservations((current) =>
          Object.fromEntries(
            Object.entries(current).filter(
              ([key]) => !key.startsWith(`${nextGap.workspaceId}:`),
            ),
          ),
        );
        if (!activeWorkspace || nextGap.workspaceId !== activeWorkspace.id) {
          return;
        }
        setGap(nextGap);
        void recoverWorkspace(activeWorkspace)
          .then(() => refresh())
          .catch(() => undefined);
      }),
    [activeWorkspace, recoverWorkspace, refresh],
  );

  useEffect(() => {
    if (backendMode !== "remote") {
      return;
    }
    const requests = approvals.flatMap((request) => {
      const threadId =
        typeof request.params?.threadId === "string"
          ? request.params.threadId
          : typeof request.params?.thread_id === "string"
            ? request.params.thread_id
            : null;
      return threadId ? [{ request, threadId }] : [];
    });
    let cancelled = false;
    void Promise.all(
      requests.map(async ({ request, threadId }) => {
        try {
          const [snapshot, projectionFreshness] = await Promise.all([
            getAuthoritativeObservationSnapshot(request.workspace_id, threadId),
            getProjectionFreshness(request.workspace_id, threadId),
          ]);
          const observationFreshness = projectionFreshness.coverages.find(
            (coverage) => coverage.coverage === "observation_snapshot",
          );
          const isCurrent = Boolean(
            observationFreshness?.status === "current" &&
              observationFreshness.generations.workspaceSessionGeneration ===
                snapshot.workspaceSessionGeneration &&
              observationFreshness.generations.appServerConnectionGeneration ===
                snapshot.appServerConnectionGeneration &&
              projectionFreshness.threadKey?.threadId === threadId,
          );
          return [
            `${request.workspace_id}:${threadId}`,
            { snapshot, freshness: isCurrent ? "current" : "unknown" },
          ] as const;
        } catch {
          return null;
        }
      }),
    ).then((entries) => {
      if (!cancelled) {
        setApprovalObservations(
          Object.fromEntries(entries.filter((entry) => entry !== null)),
        );
      }
    });
    return () => { cancelled = true; };
  }, [approvalQueryKey, backendMode]);

  const effectiveFreshness =
    freshness ?? unknownFreshness(activeWorkspace?.id ?? "", activeThreadId);
  const model = useMemo(
    () =>
      buildProjectionStatusModel({
        freshness: effectiveFreshness,
        availability,
        gap,
        deliveryMode,
      }),
    [availability, deliveryMode, effectiveFreshness, gap],
  );

  const isApprovalActionable = useCallback(
    (request: ApprovalRequest) => {
      const threadId =
        typeof request.params?.threadId === "string"
          ? request.params.threadId
          : typeof request.params?.thread_id === "string"
            ? request.params.thread_id
            : null;
      const requestObservation = threadId
        ? approvalObservations[`${request.workspace_id}:${threadId}`] ?? null
        : null;
      return (
      isApprovalProjectionActionable({
        request,
        observation: requestObservation?.snapshot ?? null,
        observationFreshness:
          requestObservation?.freshness === "current" &&
          model.availability.kind === "ready"
            ? "current"
            : "unknown",
      })
      );
    },
    [approvalObservations, model.availability.kind],
  );

  return { model, observation, isApprovalActionable };
}
