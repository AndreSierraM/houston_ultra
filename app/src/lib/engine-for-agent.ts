/**
 * Resolves the Houston engine client for a given agent — local sidecar or cloud proxy.
 */

import type { HoustonClient } from "@houston-ai/engine-client";
import { agentFromPath, currentAgent } from "./agent-lookup";
import { getEngine } from "./engine";
import { getAgentEngineClient, isCloudAgent } from "./runtime-router";
import type { Agent } from "./types";

const cloudClientCache = new Map<string, HoustonClient>();
let cachedCloudAgentId: string | null = null;

/** Resolve agent for engine routing from a folder path. */
export function agentForEngine(agentPath?: string | null): Agent | null {
  if (!agentPath) return currentAgent();
  const fromPath = agentFromPath(agentPath);
  if (fromPath) return fromPath;
  if (agentPath.startsWith("cloud://")) {
    const id = agentPath.slice("cloud://".length);
    const current = currentAgent();
    if (current?.id === id && isCloudAgent(current)) return current;
    return null;
  }
  const current = currentAgent();
  if (current?.folderPath === agentPath) return current;
  return null;
}

export async function resolveEngineForPath(agentPath: string): Promise<HoustonClient> {
  return resolveEngine(agentForEngine(agentPath), agentPath);
}

function clearCloudCache(agentId: string | null): void {
  if (agentId) {
    cloudClientCache.delete(agentId);
  }
}

function engineAgent(agent?: Agent | null, agentPath?: string | null): Agent | null {
  const resolved = agent ?? agentForEngine(agentPath);
  if (!resolved) return null;
  if (isCloudAgent(resolved)) return resolved;
  const live = currentAgent();
  if (live?.id === resolved.id && isCloudAgent(live)) return live;
  return resolved;
}

export async function resolveEngine(
  agent?: Agent | null,
  agentPath?: string | null,
): Promise<HoustonClient> {
  const resolved = engineAgent(agent, agentPath);
  if (!resolved || !isCloudAgent(resolved)) {
    if (cachedCloudAgentId) {
      clearCloudCache(cachedCloudAgentId);
      cachedCloudAgentId = null;
    }
    return getEngine();
  }

  if (cachedCloudAgentId && cachedCloudAgentId !== resolved.id) {
    clearCloudCache(cachedCloudAgentId);
  }
  cachedCloudAgentId = resolved.id;

  const cached = cloudClientCache.get(resolved.id);
  if (cached) {
    return cached;
  }

  const client = await getAgentEngineClient(resolved);
  cloudClientCache.set(resolved.id, client);
  return client;
}
