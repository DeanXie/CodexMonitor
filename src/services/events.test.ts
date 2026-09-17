import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Event, EventCallback, UnlistenFn } from "@tauri-apps/api/event";
import { listen } from "@tauri-apps/api/event";
import currentLocalAppEvent from "../../docs/fixtures/generation-tagged-events/current-local-app-event.json";
import currentRemoteEvent from "../../docs/fixtures/generation-tagged-events/current-remote-event.json";
import type { AppServerEvent, AppServerEventGap } from "../types";
import type { GlobalSourceSnapshot } from "@/features/agent-monitor/global-source/types";
import {
  clearAppServerEventGenerationContext,
  recordProjectionFreshnessForEventDelivery,
  subscribeAppServerEventGaps,
  subscribeProjectionFreshnessSnapshots,
  subscribeAppServerEvents,
  subscribeGlobalSourceSnapshot,
  subscribeMenuCycleCollaborationMode,
  subscribeMenuCycleModel,
  subscribeMenuNewAgent,
  subscribeTerminalOutput,
} from "./events";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
}));

describe("events subscriptions", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(listen).mockResolvedValue(vi.fn());
    clearAppServerEventGenerationContext();
  });

  const generations = {
    daemonProcessGeneration: null,
    remoteTransportGeneration: null,
    workspaceSessionGeneration: "workspace-generation-current",
    appServerConnectionGeneration: "connection-generation-current",
  };

  const currentFreshness = (status: "current" | "stale" | "not_hydrated" = "current") => ({
    workspaceId: "ws-1",
    threadKey: null,
    coverages: [
      {
        coverage: "thread_catalog" as const,
        status,
        generations,
        source: status === "not_hydrated" ? null : ("thread_list" as const),
        observedAt: status === "not_hydrated" ? null : 10,
        hydratedAt: status === "not_hydrated" ? null : 10,
      },
    ],
  });

  const currentEvent = (): AppServerEvent => ({
    workspace_id: "ws-1",
    message: { method: "thread/updated" },
    daemonProcessGeneration: null,
    remoteTransportGeneration: null,
    workspaceSessionGeneration: "workspace-generation-current",
    appServerConnectionGeneration: "connection-generation-current",
  });

  it("keeps fixture and TypeScript event envelopes in schema parity", () => {
    const local: AppServerEvent = currentLocalAppEvent;
    const remote: AppServerEvent = currentRemoteEvent;

    expect(local.remoteTransportGeneration).toBeNull();
    expect(remote.daemonProcessGeneration).toBe("daemon-generation-a");
    expect(remote.workspaceSessionGeneration).toBe("workspace-generation-a");
    expect(remote.appServerConnectionGeneration).toBe("connection-generation-a");
  });

  it("delivers payloads and unsubscribes on cleanup", async () => {
    let listener: EventCallback<AppServerEvent> = () => {};
    const unlisten = vi.fn();

    vi.mocked(listen).mockImplementation((_event, handler) => {
      listener = handler as EventCallback<AppServerEvent>;
      return Promise.resolve(unlisten);
    });

    const onEvent = vi.fn();
    const cleanup = subscribeAppServerEvents(onEvent);
    recordProjectionFreshnessForEventDelivery(currentFreshness());
    const payload = currentEvent();

    const event: Event<AppServerEvent> = {
      event: "app-server-event",
      id: 1,
      payload,
    };
    listener(event);
    expect(onEvent).toHaveBeenCalledWith(payload);

    cleanup();
    await Promise.resolve();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it("current_generation_event_is_delivered", () => {
    recordProjectionFreshnessForEventDelivery(currentFreshness());
    const event = currentEvent();
    const onEvent = vi.fn();
    const cleanup = subscribeAppServerEvents(onEvent);
    const handler = vi.mocked(listen).mock.calls[0]?.[1] as EventCallback<AppServerEvent>;

    handler({ event: "app-server-event", id: 2, payload: event });

    expect(onEvent).toHaveBeenCalledWith(event);
    cleanup();
  });

  it("stale_transport_generation_event_is_rejected", () => {
    recordProjectionFreshnessForEventDelivery({
      ...currentFreshness(),
      coverages: [{
        ...currentFreshness().coverages[0],
        generations: {
          ...generations,
          daemonProcessGeneration: "daemon-current",
          remoteTransportGeneration: "transport-current",
        },
      }],
    });
    const onEvent = vi.fn();
    const cleanup = subscribeAppServerEvents(onEvent);
    const handler = vi.mocked(listen).mock.calls[0]?.[1] as EventCallback<AppServerEvent>;

    handler({
      event: "app-server-event",
      id: 3,
      payload: {
        ...currentEvent(),
        daemonProcessGeneration: "daemon-current",
        remoteTransportGeneration: "transport-stale",
      },
    });

    expect(onEvent).not.toHaveBeenCalled();
    cleanup();
  });

  it("stale_workspace_generation_event_is_rejected", () => {
    recordProjectionFreshnessForEventDelivery(currentFreshness());
    const onEvent = vi.fn();
    const cleanup = subscribeAppServerEvents(onEvent);
    const handler = vi.mocked(listen).mock.calls[0]?.[1] as EventCallback<AppServerEvent>;

    handler({
      event: "app-server-event",
      id: 4,
      payload: { ...currentEvent(), workspaceSessionGeneration: "workspace-stale" },
    });

    expect(onEvent).not.toHaveBeenCalled();
    cleanup();
  });

  it("stale_app_server_generation_event_is_rejected", () => {
    recordProjectionFreshnessForEventDelivery(currentFreshness());
    const onEvent = vi.fn();
    const cleanup = subscribeAppServerEvents(onEvent);
    const handler = vi.mocked(listen).mock.calls[0]?.[1] as EventCallback<AppServerEvent>;

    handler({
      event: "app-server-event",
      id: 5,
      payload: { ...currentEvent(), appServerConnectionGeneration: "connection-stale" },
    });

    expect(onEvent).not.toHaveBeenCalled();
    cleanup();
  });

  it("stale_daemon_generation_event_is_rejected", () => {
    recordProjectionFreshnessForEventDelivery({
      ...currentFreshness(),
      coverages: [{
        ...currentFreshness().coverages[0],
        generations: {
          ...generations,
          daemonProcessGeneration: "daemon-current",
          remoteTransportGeneration: "transport-current",
        },
      }],
    });
    const onEvent = vi.fn();
    const cleanup = subscribeAppServerEvents(onEvent);
    const handler = vi.mocked(listen).mock.calls[0]?.[1] as EventCallback<AppServerEvent>;

    handler({
      event: "app-server-event",
      id: 6,
      payload: {
        ...currentEvent(),
        daemonProcessGeneration: "daemon-stale",
        remoteTransportGeneration: "transport-current",
      },
    });

    expect(onEvent).not.toHaveBeenCalled();
    cleanup();
  });

  it("missing_required_generation_event_fails_closed", () => {
    recordProjectionFreshnessForEventDelivery(currentFreshness());
    const onEvent = vi.fn();
    const cleanup = subscribeAppServerEvents(onEvent);
    const handler = vi.mocked(listen).mock.calls[0]?.[1] as EventCallback<AppServerEvent>;
    const missing = currentEvent() as Partial<AppServerEvent>;
    delete missing.workspaceSessionGeneration;

    handler({ event: "app-server-event", id: 7, payload: missing as AppServerEvent });

    expect(onEvent).not.toHaveBeenCalled();
    cleanup();
  });

  it("frontend_without_hydrated_generation_context_does_not_accept_event_as_current", () => {
    const onEvent = vi.fn();
    const cleanup = subscribeAppServerEvents(onEvent);
    const handler = vi.mocked(listen).mock.calls[0]?.[1] as EventCallback<AppServerEvent>;

    handler({ event: "app-server-event", id: 8, payload: currentEvent() });

    expect(onEvent).not.toHaveBeenCalled();
    cleanup();
  });

  it("event_does_not_promote_not_hydrated_catalog_to_current", () => {
    recordProjectionFreshnessForEventDelivery(currentFreshness("not_hydrated"));
    const onEvent = vi.fn();
    const cleanup = subscribeAppServerEvents(onEvent);
    const handler = vi.mocked(listen).mock.calls[0]?.[1] as EventCallback<AppServerEvent>;

    handler({ event: "app-server-event", id: 9, payload: currentEvent() });

    expect(onEvent).not.toHaveBeenCalled();
    cleanup();
  });

  it("current_non_catalog_coverage_does_not_bypass_not_hydrated_catalog", () => {
    recordProjectionFreshnessForEventDelivery({
      ...currentFreshness("not_hydrated"),
      coverages: [
        currentFreshness("not_hydrated").coverages[0],
        {
          coverage: "thread_detail",
          status: "current",
          generations,
          source: "thread_read",
          observedAt: 10,
          hydratedAt: 10,
        },
      ],
    });
    const onEvent = vi.fn();
    const cleanup = subscribeAppServerEvents(onEvent);
    const handler = vi.mocked(listen).mock.calls[0]?.[1] as EventCallback<AppServerEvent>;

    handler({ event: "app-server-event", id: 91, payload: currentEvent() });

    expect(onEvent).not.toHaveBeenCalled();
    cleanup();
  });

  it("event_does_not_promote_stale_coverage_to_current", () => {
    recordProjectionFreshnessForEventDelivery(currentFreshness("stale"));
    const onEvent = vi.fn();
    const cleanup = subscribeAppServerEvents(onEvent);
    const handler = vi.mocked(listen).mock.calls[0]?.[1] as EventCallback<AppServerEvent>;

    handler({ event: "app-server-event", id: 10, payload: currentEvent() });

    expect(onEvent).not.toHaveBeenCalled();
    cleanup();
  });

  it("stale_event_causes_zero_current_reducer_mutations", () => {
    recordProjectionFreshnessForEventDelivery(currentFreshness());
    const reducerMutation = vi.fn();
    const cleanup = subscribeAppServerEvents(reducerMutation);
    const handler = vi.mocked(listen).mock.calls[0]?.[1] as EventCallback<AppServerEvent>;

    handler({
      event: "app-server-event",
      id: 11,
      payload: { ...currentEvent(), workspaceSessionGeneration: "workspace-stale" },
    });

    expect(reducerMutation).not.toHaveBeenCalled();
    cleanup();
  });

  it("stale_event_does_not_mutate_shared_observations", () => {
    recordProjectionFreshnessForEventDelivery(currentFreshness());
    const onEvent = vi.fn();
    const cleanup = subscribeAppServerEvents(onEvent);
    const handler = vi.mocked(listen).mock.calls[0]?.[1] as EventCallback<AppServerEvent>;

    handler({
      event: "app-server-event",
      id: 12,
      payload: { ...currentEvent(), appServerConnectionGeneration: "connection-stale" },
    });
    handler({ event: "app-server-event", id: 13, payload: currentEvent() });

    expect(onEvent).toHaveBeenCalledTimes(1);
    expect(onEvent).toHaveBeenCalledWith(currentEvent());
    cleanup();
  });

  it("same_payload_new_generation_is_not_treated_as_same_provenance", () => {
    recordProjectionFreshnessForEventDelivery(currentFreshness());
    const onEvent = vi.fn();
    const cleanup = subscribeAppServerEvents(onEvent);
    const handler = vi.mocked(listen).mock.calls[0]?.[1] as EventCallback<AppServerEvent>;
    const oldEvent = currentEvent();
    handler({ event: "app-server-event", id: 14, payload: oldEvent });

    const nextGenerations = {
      ...generations,
      workspaceSessionGeneration: "workspace-generation-next",
      appServerConnectionGeneration: "connection-generation-next",
    };
    recordProjectionFreshnessForEventDelivery({
      ...currentFreshness(),
      coverages: [{
        ...currentFreshness().coverages[0],
        generations: nextGenerations,
      }],
    });
    handler({ event: "app-server-event", id: 15, payload: oldEvent });
    const nextEvent = {
      ...oldEvent,
      workspaceSessionGeneration: "workspace-generation-next",
      appServerConnectionGeneration: "connection-generation-next",
    };
    handler({ event: "app-server-event", id: 16, payload: nextEvent });

    expect(onEvent).toHaveBeenCalledTimes(2);
    expect(onEvent).toHaveBeenNthCalledWith(1, oldEvent);
    expect(onEvent).toHaveBeenNthCalledWith(2, nextEvent);
    cleanup();
  });

  it("known_gap_is_scoped_to_hydrated_workspaces_on_this_frontend", () => {
    recordProjectionFreshnessForEventDelivery({
      ...currentFreshness(),
      coverages: [{
        ...currentFreshness().coverages[0],
        generations: {
          ...generations,
          daemonProcessGeneration: "daemon-current",
          remoteTransportGeneration: "transport-current",
        },
      }],
    });
    const onGap = vi.fn();
    const cleanup = subscribeAppServerEventGaps(onGap);
    const call = vi.mocked(listen).mock.calls.find(
      ([event]) => event === "app-server-event-gap",
    );
    const handler = call?.[1] as EventCallback<AppServerEventGap>;

    handler({
      event: "app-server-event-gap",
      id: 17,
      payload: {
        skipped: 3,
        observedAt: 20,
        daemonProcessGeneration: "daemon-current",
        remoteTransportGeneration: "transport-current",
        affectedCoverages: ["thread_catalog", "thread_detail", "observation_snapshot"],
      },
    });

    expect(onGap).toHaveBeenCalledWith(expect.objectContaining({
      workspaceId: "ws-1",
      skipped: 3,
    }));
    cleanup();
  });

  it("stale_transport_gap_is_not_delivered", () => {
    recordProjectionFreshnessForEventDelivery({
      ...currentFreshness(),
      coverages: [{
        ...currentFreshness().coverages[0],
        generations: {
          ...generations,
          daemonProcessGeneration: "daemon-current",
          remoteTransportGeneration: "transport-current",
        },
      }],
    });
    const onGap = vi.fn();
    const cleanup = subscribeAppServerEventGaps(onGap);
    const call = vi.mocked(listen).mock.calls.find(
      ([event]) => event === "app-server-event-gap",
    );
    const handler = call?.[1] as EventCallback<AppServerEventGap>;

    handler({
      event: "app-server-event-gap",
      id: 18,
      payload: {
        skipped: 1,
        observedAt: 20,
        daemonProcessGeneration: "daemon-current",
        remoteTransportGeneration: "transport-stale",
        affectedCoverages: ["thread_catalog"],
      },
    });

    expect(onGap).not.toHaveBeenCalled();
    cleanup();
  });

  it("authoritative_freshness_snapshots_are_observable_without_new_authority", () => {
    const onSnapshot = vi.fn();
    const cleanup = subscribeProjectionFreshnessSnapshots(onSnapshot);
    const snapshot = currentFreshness();

    recordProjectionFreshnessForEventDelivery(snapshot);

    expect(onSnapshot).toHaveBeenCalledWith(snapshot);
    cleanup();
  });

  it("fans out immutable Global Source snapshot updates", async () => {
    let listener: EventCallback<GlobalSourceSnapshot> = () => {};
    const unlisten = vi.fn();
    vi.mocked(listen).mockImplementation((_event, handler) => {
      listener = handler as EventCallback<GlobalSourceSnapshot>;
      return Promise.resolve(unlisten);
    });
    const onEvent = vi.fn();
    const cleanup = subscribeGlobalSourceSnapshot(onEvent);
    const payload: GlobalSourceSnapshot = {
      revision: 2,
      generatedAtMs: 10,
      workspaceCodexHomeIdentities: {},
      threads: [],
    };

    listener({ event: "global-source-snapshot-updated", id: 2, payload });

    expect(onEvent).toHaveBeenCalledWith(payload);
    cleanup();
    await Promise.resolve();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it("cleans up listeners that resolve after unsubscribe", async () => {
    let resolveListener: (handler: UnlistenFn) => void = () => {};
    const unlisten = vi.fn();

    vi.mocked(listen).mockImplementation(
      () =>
        new Promise<UnlistenFn>((resolve) => {
          resolveListener = resolve;
        }),
    );

    const cleanup = subscribeMenuNewAgent(() => {});
    cleanup();

    resolveListener(unlisten);
    await Promise.resolve();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it("delivers menu events to subscribers", async () => {
    let listener: EventCallback<void> = () => {};
    const unlisten = vi.fn();

    vi.mocked(listen).mockImplementation((_event, handler) => {
      listener = handler as EventCallback<void>;
      return Promise.resolve(unlisten);
    });

    const onEvent = vi.fn();
    const cleanup = subscribeMenuCycleModel(onEvent);

    const event: Event<void> = {
      event: "menu-composer-cycle-model",
      id: 1,
      payload: undefined,
    };
    listener(event);
    expect(onEvent).toHaveBeenCalledTimes(1);

    cleanup();
  });

  it("delivers collaboration cycle menu events to subscribers", async () => {
    let listener: EventCallback<void> = () => {};
    const unlisten = vi.fn();

    vi.mocked(listen).mockImplementation((_event, handler) => {
      listener = handler as EventCallback<void>;
      return Promise.resolve(unlisten);
    });

    const onEvent = vi.fn();
    const cleanup = subscribeMenuCycleCollaborationMode(onEvent);

    const event: Event<void> = {
      event: "menu-composer-cycle-collaboration",
      id: 1,
      payload: undefined,
    };
    listener(event);
    expect(onEvent).toHaveBeenCalledTimes(1);

    cleanup();
  });

  it("reports listen errors through options", async () => {
    const error = new Error("nope");
    vi.mocked(listen).mockRejectedValueOnce(error);

    const onError = vi.fn();
    const cleanup = subscribeTerminalOutput(() => {}, { onError });

    await Promise.resolve();
    await Promise.resolve();
    expect(onError).toHaveBeenCalledWith(error);

    cleanup();
  });
});
