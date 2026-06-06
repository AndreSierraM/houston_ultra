/**
 * WS subscription bridge for parallel cloud burst agents.
 */

import type { HoustonEvent } from "@houston-ai/core";
import { topics } from "@houston-ai/engine-client";
import { handleBurstHoustonEvent } from "./cloud-debug-burst-event-handler.ts";
import { ensureAgentEngineWs, isCloudAgent } from "./runtime-router.ts";
import type { Agent } from "./types.ts";

/** Subscribe to one burst agent's proxied engine firehose. */
export async function attachBurstAgentEventBridge(agent: Agent): Promise<() => void> {
  if (!isCloudAgent(agent)) {
    return () => {};
  }
  const ws = await ensureAgentEngineWs(agent);
  ws.subscribe([topics.firehose]);
  return ws.onEvent((payload) => handleBurstHoustonEvent(payload as HoustonEvent));
}
