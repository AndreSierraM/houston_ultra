/**
 * Routes agent operations to the local engine or cloud proxy.
 */

import { HoustonClient, EngineWebSocket } from "@houston-ai/engine-client";
import { getEngine, getEngineWs } from "./engine";
import {
  cloudEngineBaseUrl,
  cloudEngineWsUrl,
  type AgentRuntimeMode,
} from "./cloud-client";
import type { Agent } from "./types";

let cloudWsAgentId: string | null = null;
let cloudWs: EngineWebSocket | null = null;

export function agentRuntime(agent: Agent): AgentRuntimeMode {
  return agent.runtime ?? "local";
}

export function isCloudAgent(agent: Agent): boolean {
  return agentRuntime(agent) === "cloud_24_7";
}

export async function getAgentEngineClient(agent: Agent): Promise<HoustonClient> {
  if (!isCloudAgent(agent)) {
    return getEngine();
  }
  const token = await cloudSessionToken();
  return new HoustonClient({
    baseUrl: cloudEngineBaseUrl(agent.id),
    token,
  });
}

export function getAgentEngineWsUrl(agent: Agent): string {
  if (!isCloudAgent(agent)) {
    const cfg = window.__HOUSTON_ENGINE__;
    if (!cfg?.baseUrl) {
      throw new Error("Local engine is not ready");
    }
    const base = cfg.baseUrl.replace(/^http/, "ws");
    return `${base}/v1/ws`;
  }
  return cloudEngineWsUrl(agent.id);
}

async function cloudSessionToken(): Promise<string> {
  const { cloudBearerToken } = await import("./cloud-client");
  return cloudBearerToken();
}

function disconnectCloudEngineWs(): void {
  if (!cloudWs) return;
  try {
    cloudWs.disconnect();
  } catch {
    /* ignore */
  }
  cloudWs = null;
  cloudWsAgentId = null;
}

/** Shared WS for an agent. Local agents reuse the engine singleton. */
export function getAgentEngineWs(agent: Agent): EngineWebSocket {
  if (!isCloudAgent(agent)) {
    return getEngineWs();
  }
  if (cloudWs && cloudWsAgentId === agent.id) {
    return cloudWs;
  }
  throw new Error(
    `[runtime-router] cloud WebSocket for ${agent.id} is not connected yet`,
  );
}

/** Connect (or reuse) the cloud WS for this agent. Disconnects prior cloud WS on switch. */
export async function ensureAgentEngineWs(agent: Agent): Promise<EngineWebSocket> {
  if (!isCloudAgent(agent)) {
    return getEngineWs();
  }
  if (cloudWs && cloudWsAgentId === agent.id) {
    return cloudWs;
  }
  disconnectCloudEngineWs();
  const token = await cloudSessionToken();
  const wsUrl = `${getAgentEngineWsUrl(agent)}?token=${encodeURIComponent(token)}`;
  const wsClient = { wsUrl: () => wsUrl };
  const ws = new EngineWebSocket(wsClient as HoustonClient);
  ws.connect();
  cloudWs = ws;
  cloudWsAgentId = agent.id;
  return ws;
}
