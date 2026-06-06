/**
 * Unified subscription helpers.
 *
 * Events flow over the engine WebSocket (topic-scoped). The only calls that
 * still use Tauri IPC are OS-level events (`app-activated`, `sync-connection`)
 * that the webview emits locally without going through the engine.
 *
 * Callers should NOT import `listen` from `@tauri-apps/api/event` — go
 * through this module so any future transport switch stays in one place.
 *
 * Topic model: the desktop app subscribes to the engine's firehose (`"*"`),
 * which matches every scoped topic on the server side (see
 * `engine/houston-engine-server/src/ws.rs`). Narrower clients (headless
 * agents, mobile) should add targeted subscribe helpers instead of using
 * the firehose so they don't waste bandwidth.
 */

import type { HoustonEvent } from "@houston-ai/core";
import { topics } from "@houston-ai/engine-client";
import { getEngineWs } from "./engine";
import { ensureAgentEngineWs, isCloudAgent } from "./runtime-router";
import type { Agent } from "./types";
import { legacyEmit, legacyListen } from "./os-bridge";

type Unsub = () => void;

function toHandler<T>(handler: (ev: T) => void) {
  return (payload: unknown) => handler(payload as T);
}

/**
 * Subscribe to every `HoustonEvent` emitted by the backend.
 *
 * Idempotent: calling this multiple times is safe — the underlying
 * `EngineWebSocket` de-duplicates subscriptions, so the firehose topic is
 * added once regardless of how many UI hooks mount.
 */
/**
 * Subscribe to Houston events.
 *
 * Without `agent`, always uses the local engine firehose (provider login,
 * Composio, Claude install, etc.). With a cloud agent, connects through
 * the cloud WS proxy for that agent's engine events.
 */
export function subscribeHoustonEvents(
  handler: (ev: HoustonEvent) => void,
  agent?: Agent | null,
): Unsub {
  if (agent && isCloudAgent(agent)) {
    let cancelled = false;
    let innerUnsub: Unsub | undefined;
    ensureAgentEngineWs(agent)
      .then((ws) => {
        if (cancelled) return;
        ws.subscribe([topics.firehose]);
        innerUnsub = ws.onEvent(toHandler(handler));
      })
      .catch(async (err: unknown) => {
        const { showErrorToast } = await import("./error-toast");
        const message = err instanceof Error ? err.message : String(err);
        showErrorToast("cloud-ws", message, err);
      });
    return () => {
      cancelled = true;
      innerUnsub?.();
    };
  }

  const ws = getEngineWs();
  ws.subscribe([topics.firehose]);
  return ws.onEvent(toHandler(handler));
}

/**
 * Listen to a raw Tauri event. Use for events that have no engine counterpart:
 * - `app-activated` (OS window resume)
 */
export function listenOsEvent<T>(event: string, handler: (ev: T) => void): Unsub {
  let off: Unsub | undefined;
  legacyListen<T>(event, (tauriEv) => handler(tauriEv.payload))
    .then((fn) => {
      off = fn;
    })
    .catch(() => {});
  return () => {
    off?.();
  };
}

/** Re-export `legacyEmit` so callers don't need to reach into os-bridge. */
export function emitOsEvent(event: string, payload?: unknown): Promise<void> {
  return legacyEmit(event, payload);
}
