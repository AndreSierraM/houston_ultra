export type BoardSendRoute = "queue-or-send" | "send-now";

/** Open session uses queue-or-send; other sessions send immediately. */
export function resolveBoardSendRoute(
  targetSessionKey: string,
  selectedSessionKey: string | null,
): BoardSendRoute {
  return targetSessionKey === selectedSessionKey ? "queue-or-send" : "send-now";
}

/** Composer submit on the active open session should try the queue first. */
export function shouldAttemptComposerQueue(
  ctxSessionKey: string | null | undefined,
  selectedSessionKey: string | null,
  selectedSessionActive: boolean,
): boolean {
  return !!(
    ctxSessionKey &&
    ctxSessionKey === selectedSessionKey &&
    selectedSessionActive
  );
}

/**
 * Queue capture succeeded → handled. Otherwise delegate to the panel hook
 * (e.g. Skills) so a missing agentPath/sessionKey does not swallow submit.
 */
export function resolveComposerSubmitHandled(
  attemptedQueue: boolean,
  queueCaptured: boolean,
): boolean {
  return attemptedQueue && queueCaptured;
}
