/**
 * Resolves the Houston engine client for a given agent — local sidecar or cloud proxy.
 */

import type { HoustonClient } from "@houston-ai/engine-client";
import { getEngine } from "./engine";
import { getAgentEngineClient, isCloudAgent } from "./runtime-router";
import type { Agent } from "./types";

const cloudClientCache = new Map<string, HoustonClient>();
let cachedCloudAgentId: string | null = null;

function clearCloudCache(agentId: string | null): void {
  if (agentId) {
    cloudClientCache.delete(agentId);
  }
}

export async function resolveEngine(agent?: Agent | null): Promise<HoustonClient> {
  if (!agent || !isCloudAgent(agent)) {
    if (cachedCloudAgentId) {
      clearCloudCache(cachedCloudAgentId);
      cachedCloudAgentId = null;
    }
    return getEngine();
  }

  if (cachedCloudAgentId && cachedCloudAgentId !== agent.id) {
    clearCloudCache(cachedCloudAgentId);
  }
  cachedCloudAgentId = agent.id;

  const cached = cloudClientCache.get(agent.id);
  if (cached) {
    return cached;
  }

  const client = await getAgentEngineClient(agent);
  cloudClientCache.set(agent.id, client);
  return client;
}
