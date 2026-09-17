import { listen } from "@tauri-apps/api/event";
import type {
  AppServerEventGap,
  AppServerEvent,
  DictationEvent,
  DictationModelStatus,
  ProjectionFreshnessGenerationVector,
  ProjectionFreshnessQuerySnapshot,
  ProjectionDeliveryGapEvidence,
  TrayOpenThreadPayload,
} from "../types";
import type { GlobalSourceSnapshot } from "@/features/agent-monitor/global-source/types";

export type Unsubscribe = () => void;

export type TerminalOutputEvent = {
  workspaceId: string;
  terminalId: string;
  data: string;
};

export type TerminalExitEvent = {
  workspaceId: string;
  terminalId: string;
};

type SubscriptionOptions = {
  onError?: (error: unknown) => void;
};

type Listener<T> = (payload: T) => void;

function createEventHub<T>(
  eventName: string,
  shouldDeliver: (payload: T) => boolean = () => true,
) {
  const listeners = new Set<Listener<T>>();
  let unlisten: Unsubscribe | null = null;
  let listenPromise: Promise<Unsubscribe> | null = null;

  const start = (options?: SubscriptionOptions) => {
    if (unlisten || listenPromise) {
      return;
    }
    listenPromise = listen<T>(eventName, (event) => {
      if (!shouldDeliver(event.payload)) {
        return;
      }
      for (const listener of listeners) {
        try {
          listener(event.payload);
        } catch (error) {
          console.error(`[events] ${eventName} listener failed`, error);
        }
      }
    });
    listenPromise
      .then((handler) => {
        listenPromise = null;
        if (listeners.size === 0) {
          handler();
          return;
        }
        unlisten = handler;
      })
      .catch((error) => {
        listenPromise = null;
        options?.onError?.(error);
      });
  };

  const stop = () => {
    if (unlisten) {
      try {
        unlisten();
      } catch {
        // Ignore double-unlisten when tearing down.
      }
      unlisten = null;
    }
  };

  const subscribe = (
    onEvent: Listener<T>,
    options?: SubscriptionOptions,
  ): Unsubscribe => {
    listeners.add(onEvent);
    start(options);
    return () => {
      listeners.delete(onEvent);
      if (listeners.size === 0) {
        stop();
      }
    };
  };

  return { subscribe };
}

type AppServerEventGenerationContext = {
  generations: ProjectionFreshnessGenerationVector;
  hasCurrentCoverage: boolean;
};

const appServerEventGenerationContexts = new Map<
  string,
  AppServerEventGenerationContext
>();
const projectionFreshnessListeners = new Set<
  Listener<ProjectionFreshnessQuerySnapshot>
>();

function hasRequiredSessionGenerations(
  generations: ProjectionFreshnessGenerationVector,
): boolean {
  return Boolean(
    generations.workspaceSessionGeneration &&
      generations.appServerConnectionGeneration,
  );
}

function generationMatches(
  actual: string | null | undefined,
  expected: string | null,
): boolean {
  return (actual ?? null) === expected;
}

function shouldDeliverAppServerEvent(event: AppServerEvent): boolean {
  if (
    !event.workspaceSessionGeneration ||
    !event.appServerConnectionGeneration
  ) {
    return false;
  }
  const context = appServerEventGenerationContexts.get(event.workspace_id);
  if (!context?.hasCurrentCoverage) {
    return false;
  }
  const expected = context.generations;
  return (
    event.workspaceSessionGeneration === expected.workspaceSessionGeneration &&
    event.appServerConnectionGeneration === expected.appServerConnectionGeneration &&
    generationMatches(
      event.daemonProcessGeneration,
      expected.daemonProcessGeneration,
    ) &&
    generationMatches(
      event.remoteTransportGeneration,
      expected.remoteTransportGeneration,
    )
  );
}

export function recordProjectionFreshnessForEventDelivery(
  snapshot: ProjectionFreshnessQuerySnapshot,
): void {
  for (const listener of projectionFreshnessListeners) {
    try {
      listener(snapshot);
    } catch (error) {
      console.error("[events] projection freshness listener failed", error);
    }
  }
  const threadCatalog = snapshot.coverages.find(
    (coverage) => coverage.coverage === "thread_catalog",
  );
  if (
    !threadCatalog ||
    !hasRequiredSessionGenerations(threadCatalog.generations)
  ) {
    appServerEventGenerationContexts.delete(snapshot.workspaceId);
    return;
  }
  appServerEventGenerationContexts.set(snapshot.workspaceId, {
    generations: threadCatalog.generations,
    hasCurrentCoverage: threadCatalog.status === "current",
  });
}

export function subscribeProjectionFreshnessSnapshots(
  onSnapshot: Listener<ProjectionFreshnessQuerySnapshot>,
): Unsubscribe {
  projectionFreshnessListeners.add(onSnapshot);
  return () => projectionFreshnessListeners.delete(onSnapshot);
}

export function invalidateAppServerEventGenerationContext(
  workspaceId: string,
): void {
  appServerEventGenerationContexts.delete(workspaceId);
}

export function clearAppServerEventGenerationContext(): void {
  appServerEventGenerationContexts.clear();
}

const appServerHub = createEventHub<AppServerEvent>(
  "app-server-event",
  shouldDeliverAppServerEvent,
);
const appServerGapHub = createEventHub<AppServerEventGap>("app-server-event-gap");
const globalSourceSnapshotHub = createEventHub<GlobalSourceSnapshot>(
  "global-source-snapshot-updated",
);
const dictationDownloadHub = createEventHub<DictationModelStatus>("dictation-download");
const dictationEventHub = createEventHub<DictationEvent>("dictation-event");
const terminalOutputHub = createEventHub<TerminalOutputEvent>("terminal-output");
const terminalExitHub = createEventHub<TerminalExitEvent>("terminal-exit");
const updaterCheckHub = createEventHub<void>("updater-check");
const trayOpenThreadHub = createEventHub<TrayOpenThreadPayload>("tray-open-thread");
const menuNewAgentHub = createEventHub<void>("menu-new-agent");
const menuNewWorktreeAgentHub = createEventHub<void>("menu-new-worktree-agent");
const menuNewCloneAgentHub = createEventHub<void>("menu-new-clone-agent");
const menuAddWorkspaceHub = createEventHub<void>("menu-add-workspace");
const menuAddWorkspaceFromUrlHub = createEventHub<void>("menu-add-workspace-from-url");
const menuOpenSettingsHub = createEventHub<void>("menu-open-settings");
const menuToggleProjectsSidebarHub = createEventHub<void>("menu-toggle-projects-sidebar");
const menuToggleGitSidebarHub = createEventHub<void>("menu-toggle-git-sidebar");
const menuToggleDebugPanelHub = createEventHub<void>("menu-toggle-debug-panel");
const menuToggleTerminalHub = createEventHub<void>("menu-toggle-terminal");
const menuNextAgentHub = createEventHub<void>("menu-next-agent");
const menuPrevAgentHub = createEventHub<void>("menu-prev-agent");
const menuNextWorkspaceHub = createEventHub<void>("menu-next-workspace");
const menuPrevWorkspaceHub = createEventHub<void>("menu-prev-workspace");
const menuCycleModelHub = createEventHub<void>("menu-composer-cycle-model");
const menuCycleAccessHub = createEventHub<void>("menu-composer-cycle-access");
const menuCycleReasoningHub = createEventHub<void>("menu-composer-cycle-reasoning");
const menuCycleCollaborationHub = createEventHub<void>("menu-composer-cycle-collaboration");
const menuComposerCycleModelHub = createEventHub<void>("menu-composer-cycle-model");
const menuComposerCycleAccessHub = createEventHub<void>("menu-composer-cycle-access");
const menuComposerCycleReasoningHub = createEventHub<void>("menu-composer-cycle-reasoning");
const menuComposerCycleCollaborationHub = createEventHub<void>(
  "menu-composer-cycle-collaboration",
);

export function subscribeAppServerEvents(
  onEvent: (event: AppServerEvent) => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return appServerHub.subscribe(onEvent, options);
}

export function subscribeAppServerEventGaps(
  onGap: (gap: ProjectionDeliveryGapEvidence) => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return appServerGapHub.subscribe((gap) => {
    for (const [workspaceId, context] of appServerEventGenerationContexts) {
      if (
        gap.daemonProcessGeneration ===
          context.generations.daemonProcessGeneration &&
        gap.remoteTransportGeneration ===
          context.generations.remoteTransportGeneration
      ) {
        context.hasCurrentCoverage = false;
        onGap({
          workspaceId,
          affectedCoverages: gap.affectedCoverages,
          skipped: gap.skipped,
          observedAt: gap.observedAt,
          daemonProcessGeneration: gap.daemonProcessGeneration,
          remoteTransportGeneration: gap.remoteTransportGeneration,
        });
      }
    }
  }, options);
}

export function subscribeGlobalSourceSnapshot(
  onEvent: (snapshot: GlobalSourceSnapshot) => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return globalSourceSnapshotHub.subscribe(onEvent, options);
}

export function subscribeDictationDownload(
  onEvent: (event: DictationModelStatus) => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return dictationDownloadHub.subscribe(onEvent, options);
}

export function subscribeDictationEvents(
  onEvent: (event: DictationEvent) => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return dictationEventHub.subscribe(onEvent, options);
}

export function subscribeTerminalOutput(
  onEvent: (event: TerminalOutputEvent) => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return terminalOutputHub.subscribe(onEvent, options);
}

export function subscribeTerminalExit(
  onEvent: (event: TerminalExitEvent) => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return terminalExitHub.subscribe(onEvent, options);
}

export function subscribeUpdaterCheck(
  onEvent: () => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return updaterCheckHub.subscribe(() => {
    onEvent();
  }, options);
}

export function subscribeTrayOpenThread(
  onEvent: (payload: TrayOpenThreadPayload) => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return trayOpenThreadHub.subscribe((payload) => {
    onEvent(payload);
  }, options);
}

export function subscribeMenuNewAgent(
  onEvent: () => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return menuNewAgentHub.subscribe(() => {
    onEvent();
  }, options);
}

export function subscribeMenuNewWorktreeAgent(
  onEvent: () => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return menuNewWorktreeAgentHub.subscribe(() => {
    onEvent();
  }, options);
}

export function subscribeMenuNewCloneAgent(
  onEvent: () => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return menuNewCloneAgentHub.subscribe(() => {
    onEvent();
  }, options);
}

export function subscribeMenuAddWorkspaceFromUrl(
  onEvent: () => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return menuAddWorkspaceFromUrlHub.subscribe(() => {
    onEvent();
  }, options);
}

export function subscribeMenuAddWorkspace(
  onEvent: () => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return menuAddWorkspaceHub.subscribe(() => {
    onEvent();
  }, options);
}

export function subscribeMenuOpenSettings(
  onEvent: () => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return menuOpenSettingsHub.subscribe(() => {
    onEvent();
  }, options);
}

export function subscribeMenuToggleProjectsSidebar(
  onEvent: () => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return menuToggleProjectsSidebarHub.subscribe(() => {
    onEvent();
  }, options);
}

export function subscribeMenuToggleGitSidebar(
  onEvent: () => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return menuToggleGitSidebarHub.subscribe(() => {
    onEvent();
  }, options);
}

export function subscribeMenuToggleDebugPanel(
  onEvent: () => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return menuToggleDebugPanelHub.subscribe(() => {
    onEvent();
  }, options);
}

export function subscribeMenuToggleTerminal(
  onEvent: () => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return menuToggleTerminalHub.subscribe(() => {
    onEvent();
  }, options);
}

export function subscribeMenuNextAgent(
  onEvent: () => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return menuNextAgentHub.subscribe(() => {
    onEvent();
  }, options);
}

export function subscribeMenuPrevAgent(
  onEvent: () => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return menuPrevAgentHub.subscribe(() => {
    onEvent();
  }, options);
}

export function subscribeMenuNextWorkspace(
  onEvent: () => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return menuNextWorkspaceHub.subscribe(() => {
    onEvent();
  }, options);
}

export function subscribeMenuPrevWorkspace(
  onEvent: () => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return menuPrevWorkspaceHub.subscribe(() => {
    onEvent();
  }, options);
}

export function subscribeMenuCycleModel(
  onEvent: () => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return menuCycleModelHub.subscribe(() => {
    onEvent();
  }, options);
}

export function subscribeMenuCycleAccessMode(
  onEvent: () => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return menuCycleAccessHub.subscribe(() => {
    onEvent();
  }, options);
}

export function subscribeMenuCycleReasoning(
  onEvent: () => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return menuCycleReasoningHub.subscribe(() => {
    onEvent();
  }, options);
}

export function subscribeMenuCycleCollaborationMode(
  onEvent: () => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return menuCycleCollaborationHub.subscribe(() => {
    onEvent();
  }, options);
}

export function subscribeMenuComposerCycleModel(
  onEvent: () => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return menuComposerCycleModelHub.subscribe(() => {
    onEvent();
  }, options);
}

export function subscribeMenuComposerCycleAccess(
  onEvent: () => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return menuComposerCycleAccessHub.subscribe(() => {
    onEvent();
  }, options);
}

export function subscribeMenuComposerCycleReasoning(
  onEvent: () => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return menuComposerCycleReasoningHub.subscribe(() => {
    onEvent();
  }, options);
}

export function subscribeMenuComposerCycleCollaboration(
  onEvent: () => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return menuComposerCycleCollaborationHub.subscribe(() => {
    onEvent();
  }, options);
}
