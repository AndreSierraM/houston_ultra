/**
 * Resolves the Houston engine client for a given agent — local sidecar or cloud proxy.
 */

import type { HoustonClient } from "@houston-ai/engine-client";
import { agentFromPath, currentAgent } from "./agent-lookup";
import {
  isCloudEngineFilesystemPath,
  isSyntheticCloudPath,
} from "./engine-agent-path";
import { isCloudConfigured } from "./cloud-client";
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

function isExplicitlyCloud(agent?: Agent | null, agentPath?: string | null): boolean {
  if (agent != null && isCloudAgent(agent)) return true;
  if (agentPath != null && isSyntheticCloudPath(agentPath)) return true;
  if (
    agentPath != null &&
    isCloudEngineFilesystemPath(agentPath) &&
    isCloudConfigured()
  ) {
    return true;
  }
  return false;
}

function cloudRoutingError(agent?: Agent | null, agentPath?: string | null): Error {
  const target =
    (agentPath != null && agentPath) ||
    (agent != null && `agent ${agent.id}`) ||
    "cloud agent";
  return new Error(
    `[engine-for-agent] cloud engine required for ${target}, but no cloud agent record could be resolved`,
  );
}

export async function resolveEngine(
  agent?: Agent | null,
  agentPath?: string | null,
): Promise<HoustonClient> {
  const explicitlyCloud = isExplicitlyCloud(agent, agentPath);
  const resolved = engineAgent(agent, agentPath);

  if (explicitlyCloud) {
    if (!resolved || !isCloudAgent(resolved)) {
      throw cloudRoutingError(agent, agentPath);
    }
  } else if (!resolved || !isCloudAgent(resolved)) {
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
