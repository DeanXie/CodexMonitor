import { useCallback, useEffect, useMemo, useState } from "react";
import type {
  AppMention,
  ComposerSendIntent,
  FollowUpMessageBehavior,
  QueuedMessage,
  SendMessageResult,
  WorkspaceInfo,
} from "@/types";
import type { CreationAction, SendAction } from "./creationAction";

type UseQueuedSendOptions = {
  activeThreadId: string | null;
  activeTurnId: string | null;
  isProcessing: boolean;
  isReviewing: boolean;
  queueFlushPaused?: boolean;
  steerEnabled: boolean;
  followUpMessageBehavior: FollowUpMessageBehavior;
  appsEnabled: boolean;
  activeWorkspace: WorkspaceInfo | null;
  connectWorkspace: (workspace: WorkspaceInfo) => Promise<void>;
  beginSendAction?: (forceCreation?: boolean) => SendAction;
  startThreadForWorkspace: (
    workspaceId: string,
    options?: { activate?: boolean; creationAction?: CreationAction },
  ) => Promise<string | null>;
  sendUserMessage: (
    text: string,
    images?: string[],
    appMentions?: AppMention[],
    options?: { sendIntent?: ComposerSendIntent; sendAction?: SendAction },
  ) => Promise<SendMessageResult>;
  sendUserMessageToThread: (
    workspace: WorkspaceInfo,
    threadId: string,
    text: string,
    images?: string[],
    options?: { creationIntent?: import("@services/tauri").CreationIntentContext; turnIntent?: import("@services/tauri").CreationIntentContext },
  ) => Promise<void | SendMessageResult>;
  startFork: (text: string) => Promise<void>;
  startReview: (text: string) => Promise<void>;
  startResume: (text: string) => Promise<void>;
  startCompact: (text: string) => Promise<void>;
  startApps: (text: string) => Promise<void>;
  startMcp: (text: string) => Promise<void>;
  startFast: (text: string) => Promise<void>;
  startStatus: (text: string) => Promise<void>;
  clearActiveImages: () => void;
};

type UseQueuedSendResult = {
  queuedByThread: Record<string, QueuedMessage[]>;
  activeQueue: QueuedMessage[];
  handleSend: (
    text: string,
    images?: string[],
    appMentions?: AppMention[],
    submitIntent?: ComposerSendIntent,
  ) => Promise<void>;
  queueMessage: (
    text: string,
    images?: string[],
    appMentions?: AppMention[],
  ) => Promise<void>;
  removeQueuedMessage: (threadId: string, messageId: string) => void;
};

type SlashCommandKind =
  | "apps"
  | "compact"
  | "fast"
  | "fork"
  | "mcp"
  | "new"
  | "resume"
  | "review"
  | "status";

type CoordinatedQueuedMessage = QueuedMessage & { sendAction?: SendAction };

function parseSlashCommand(text: string, appsEnabled: boolean): SlashCommandKind | null {
  if (appsEnabled && /^\/apps\b/i.test(text)) {
    return "apps";
  }
  if (/^\/fork\b/i.test(text)) {
    return "fork";
  }
  if (/^\/fast\b/i.test(text)) {
    return "fast";
  }
  if (/^\/mcp\b/i.test(text)) {
    return "mcp";
  }
  if (/^\/review\b/i.test(text)) {
    return "review";
  }
  if (/^\/compact\b/i.test(text)) {
    return "compact";
  }
  if (/^\/new\b/i.test(text)) {
    return "new";
  }
  if (/^\/resume\b/i.test(text)) {
    return "resume";
  }
  if (/^\/status\b/i.test(text)) {
    return "status";
  }
  return null;
}

export function useQueuedSend({
  activeThreadId,
  activeTurnId,
  isProcessing,
  isReviewing,
  queueFlushPaused = false,
  steerEnabled,
  followUpMessageBehavior,
  appsEnabled,
  activeWorkspace,
  connectWorkspace,
  beginSendAction,
  startThreadForWorkspace,
  sendUserMessage,
  sendUserMessageToThread,
  startFork,
  startReview,
  startResume,
  startCompact,
  startApps,
  startMcp,
  startFast,
  startStatus,
  clearActiveImages,
}: UseQueuedSendOptions): UseQueuedSendResult {
  const [queuedByThread, setQueuedByThread] = useState<
    Record<string, CoordinatedQueuedMessage[]>
  >({});
  const [inFlightByThread, setInFlightByThread] = useState<
    Record<string, CoordinatedQueuedMessage | null>
  >({});
  const [hasStartedByThread, setHasStartedByThread] = useState<
    Record<string, boolean>
  >({});

  const activeQueue = useMemo(
    () => (activeThreadId ? queuedByThread[activeThreadId] ?? [] : []),
    [activeThreadId, queuedByThread],
  );

  const enqueueMessage = useCallback((threadId: string, item: CoordinatedQueuedMessage) => {
    setQueuedByThread((prev) => ({
      ...prev,
      [threadId]: [...(prev[threadId] ?? []), item],
    }));
  }, []);

  const removeQueuedMessage = useCallback(
    (threadId: string, messageId: string) => {
      setQueuedByThread((prev) => ({
        ...prev,
        [threadId]: (prev[threadId] ?? []).filter(
          (entry) => entry.id !== messageId,
        ),
      }));
    },
    [],
  );

  const prependQueuedMessage = useCallback((threadId: string, item: CoordinatedQueuedMessage) => {
    setQueuedByThread((prev) => ({
      ...prev,
      [threadId]: [item, ...(prev[threadId] ?? [])],
    }));
  }, []);

  const createQueuedItem = useCallback(
    (
      text: string,
      images: string[],
      appMentions: AppMention[],
      sendAction?: SendAction,
    ): CoordinatedQueuedMessage => ({
      id: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
      text,
      createdAt: Date.now(),
      images,
      ...(appMentions.length > 0 ? { appMentions } : {}),
      ...(sendAction ? { sendAction } : {}),
    }),
    [],
  );

  const runSlashCommand = useCallback(
    async (command: SlashCommandKind, trimmed: string, sendAction?: SendAction) => {
      if (command === "fork") {
        await startFork(trimmed);
        return;
      }
      if (command === "review") {
        await startReview(trimmed);
        return;
      }
      if (command === "resume") {
        await startResume(trimmed);
        return;
      }
      if (command === "compact") {
        await startCompact(trimmed);
        return;
      }
      if (command === "apps") {
        await startApps(trimmed);
        return;
      }
      if (command === "mcp") {
        await startMcp(trimmed);
        return;
      }
      if (command === "fast") {
        await startFast(trimmed);
        return;
      }
      if (command === "status") {
        await startStatus(trimmed);
        return;
      }
      if (command === "new" && activeWorkspace) {
        const creationAction = sendAction?.creationAction;
        if (!creationAction) {
          return;
        }
        const threadId = await startThreadForWorkspace(activeWorkspace.id, { creationAction });
        const rest = trimmed.replace(/^\/new\b/i, "").trim();
        if (threadId && rest) {
          await sendUserMessageToThread(activeWorkspace, threadId, rest, [], {
            creationIntent: await creationAction.creationIntent,
            turnIntent: await sendAction.turnIntent,
          });
        }
      }
    },
    [
      activeWorkspace,
      sendUserMessageToThread,
      startFork,
      startReview,
      startResume,
      startCompact,
      startApps,
      startMcp,
      startFast,
      startStatus,
      startThreadForWorkspace,
    ],
  );

  const handleSend = useCallback(
    async (
      text: string,
      images: string[] = [],
      appMentions: AppMention[] = [],
      submitIntent: ComposerSendIntent = "default",
    ) => {
      const trimmed = text.trim();
      const command = parseSlashCommand(trimmed, appsEnabled);
      const nextImages = command ? [] : images;
      const nextMentions = command ? [] : appMentions;
      const canSteerCurrentTurn =
        isProcessing && steerEnabled && Boolean(activeTurnId);
      const effectiveIntent: ComposerSendIntent = !isProcessing
        ? "default"
        : submitIntent === "queue"
          ? "queue"
          : submitIntent === "steer"
            ? canSteerCurrentTurn
              ? "steer"
              : "queue"
            : followUpMessageBehavior === "steer" && canSteerCurrentTurn
              ? "steer"
              : "queue";
      if (!trimmed && nextImages.length === 0) {
        return;
      }
      if (activeThreadId && isReviewing) {
        return;
      }
      const sendAction = beginSendAction?.(command === "new");
      if (isProcessing && activeThreadId && effectiveIntent === "queue") {
        const item = createQueuedItem(trimmed, nextImages, nextMentions, sendAction);
        enqueueMessage(activeThreadId, item);
        clearActiveImages();
        return;
      }
      if (activeWorkspace && !activeWorkspace.connected) {
        await connectWorkspace(activeWorkspace);
      }
      if (command) {
        await runSlashCommand(command, trimmed, sendAction);
        clearActiveImages();
        return;
      }
      const sendResult =
        nextMentions.length > 0
          ? await sendUserMessage(trimmed, nextImages, nextMentions, {
            sendIntent: effectiveIntent,
            sendAction,
          })
          : await sendUserMessage(trimmed, nextImages, undefined, {
          sendIntent: effectiveIntent,
          sendAction,
          });
      if (
        sendResult.status === "steer_failed" &&
        activeThreadId &&
        isProcessing
      ) {
        enqueueMessage(
          activeThreadId,
          createQueuedItem(trimmed, nextImages, nextMentions, sendAction),
        );
      }
      clearActiveImages();
    },
    [
      activeThreadId,
      appsEnabled,
      activeWorkspace,
      beginSendAction,
      clearActiveImages,
      connectWorkspace,
      createQueuedItem,
      enqueueMessage,
      activeTurnId,
      followUpMessageBehavior,
      isProcessing,
      isReviewing,
      steerEnabled,
      runSlashCommand,
      sendUserMessage,
    ],
  );

  const queueMessage = useCallback(
    async (
      text: string,
      images: string[] = [],
      appMentions: AppMention[] = [],
    ) => {
      const trimmed = text.trim();
      const command = parseSlashCommand(trimmed, appsEnabled);
      const nextImages = command ? [] : images;
      const nextMentions = command ? [] : appMentions;
      if (!trimmed && nextImages.length === 0) {
        return;
      }
      if (activeThreadId && isReviewing) {
        return;
      }
      if (!activeThreadId) {
        return;
      }
      const item = createQueuedItem(
        trimmed,
        nextImages,
        nextMentions,
        beginSendAction?.(command === "new"),
      );
      enqueueMessage(activeThreadId, item);
      clearActiveImages();
    },
    [
      activeThreadId,
      appsEnabled,
      clearActiveImages,
      createQueuedItem,
      enqueueMessage,
      isReviewing,
    ],
  );

  useEffect(() => {
    if (!activeThreadId) {
      return;
    }
    const inFlight = inFlightByThread[activeThreadId];
    if (!inFlight) {
      return;
    }
    if (isProcessing || isReviewing) {
      if (!hasStartedByThread[activeThreadId]) {
        setHasStartedByThread((prev) => ({
          ...prev,
          [activeThreadId]: true,
        }));
      }
      return;
    }
    if (hasStartedByThread[activeThreadId]) {
      setInFlightByThread((prev) => ({ ...prev, [activeThreadId]: null }));
      setHasStartedByThread((prev) => ({ ...prev, [activeThreadId]: false }));
    }
  }, [
    activeThreadId,
    hasStartedByThread,
    inFlightByThread,
    isProcessing,
    isReviewing,
  ]);

  useEffect(() => {
    if (!activeThreadId || isProcessing || isReviewing || queueFlushPaused) {
      return;
    }
    if (inFlightByThread[activeThreadId]) {
      return;
    }
    const queue = queuedByThread[activeThreadId] ?? [];
    if (queue.length === 0) {
      return;
    }
    const threadId = activeThreadId;
    const nextItem = queue[0];
    setInFlightByThread((prev) => ({ ...prev, [threadId]: nextItem }));
    setHasStartedByThread((prev) => ({ ...prev, [threadId]: false }));
    setQueuedByThread((prev) => ({
      ...prev,
      [threadId]: (prev[threadId] ?? []).slice(1),
    }));
    (async () => {
      try {
        const trimmed = nextItem.text.trim();
        const command = parseSlashCommand(trimmed, appsEnabled);
        if (command) {
          await runSlashCommand(command, trimmed, nextItem.sendAction);
        } else {
          const queuedMentions = nextItem.appMentions ?? [];
          if (queuedMentions.length > 0) {
            await sendUserMessage(nextItem.text, nextItem.images ?? [], queuedMentions, {
              sendAction: nextItem.sendAction,
            });
          } else {
            await sendUserMessage(nextItem.text, nextItem.images ?? [], undefined, {
              sendAction: nextItem.sendAction,
            });
          }
        }
      } catch {
        setInFlightByThread((prev) => ({ ...prev, [threadId]: null }));
        setHasStartedByThread((prev) => ({ ...prev, [threadId]: false }));
        prependQueuedMessage(threadId, nextItem);
      }
    })();
  }, [
    activeThreadId,
    appsEnabled,
    inFlightByThread,
    isProcessing,
    isReviewing,
    queueFlushPaused,
    prependQueuedMessage,
    queuedByThread,
    runSlashCommand,
    sendUserMessage,
  ]);

  return {
    queuedByThread,
    activeQueue,
    handleSend,
    queueMessage,
    removeQueuedMessage,
  };
}
