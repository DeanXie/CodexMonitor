import type {
  GlobalSourceSnapshot,
  GlobalSourceThreadKey,
  SurfaceProjectionObservation,
} from "../global-source/types";

export const DESKTOP_STALE_ORPHAN = "DESKTOP_STALE_ORPHAN";

export type SurfaceProjectionView = {
  byThreadKey: Map<string, SurfaceProjectionObservation[]>;
  issues: SurfaceProjectionObservation[];
};

export function surfaceProjectionThreadKey(key: GlobalSourceThreadKey) {
  return `${key.codexHomeIdentity}\u001f${key.threadId}`;
}

function compareProjection(
  left: SurfaceProjectionObservation,
  right: SurfaceProjectionObservation,
) {
  return left.key.surface.localeCompare(right.key.surface)
    || left.key.projectionKind.localeCompare(right.key.projectionKind)
    || left.observedAt - right.observedAt
    || left.state.localeCompare(right.state);
}

export function selectSurfaceProjectionView(
  snapshot: GlobalSourceSnapshot,
): SurfaceProjectionView {
  const observations = [...(snapshot.surfaceProjections ?? [])].sort(compareProjection);
  const canonicalThreadKeys = new Set(snapshot.threads.map((thread) =>
    surfaceProjectionThreadKey(thread.key)));
  const byThreadKey = new Map<string, SurfaceProjectionObservation[]>();
  for (const observation of observations) {
    const key = surfaceProjectionThreadKey(observation.key.threadKey);
    const current = byThreadKey.get(key) ?? [];
    current.push(observation);
    byThreadKey.set(key, current);
  }
  return {
    byThreadKey,
    issues: observations.filter((observation) =>
      observation.state === "STALE"
      && observation.diagnostics.includes(DESKTOP_STALE_ORPHAN)
      && !canonicalThreadKeys.has(surfaceProjectionThreadKey(observation.key.threadKey))),
  };
}
