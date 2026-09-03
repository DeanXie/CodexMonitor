// Synthetic IPC evidence used only by creation boundary tests.
export function acknowledgedCreation(threadId: string) {
  return {
    result: {
      thread: { id: threadId },
      creationAcknowledgement: {
        state: "THREAD_ACKNOWLEDGED" as const,
        threadKey: { codexHomeIdentity: "codex-home:test", threadId },
        persistence: "NOT_YET_CONFIRMED" as const,
        ephemeral: "UNKNOWN" as const,
        firstTurnAcceptance: "NOT_YET_ACCEPTED" as const,
        firstTurn: null,
        firstTurnOutcome: "UNKNOWN" as const,
      },
    },
  };
}
