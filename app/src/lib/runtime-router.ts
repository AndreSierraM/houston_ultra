/**
 * Routes agent operations to the local engine or cloud proxy.
 */

import { HoustonClient, EngineWebSocket } from "@houston-ai/engine-client";
import { getEngine, getEngineWs } from "./engine";
import {
  cloudEngineBaseUrl,
  cloudEngineWsUrl,
} from "./cloud-client";
import type { Agent } from "./types";
import { CloudAgentWsRegistry } from "./cloud-agent-ws-registry";
import { isCloudAgent } from "./agent-runtime-mode";

export { agentRuntime, isCloudAgent } from "./agent-runtime-mode";

const DEFAULT_CLOUD_WS = import.meta.env.DEV ? 128 : 4;
const cloudWsRegistry = new CloudAgentWsRegistry<EngineWebSocket>(DEFAULT_CLOUD_WS);

/** Dev cloud debug burst: raise proxied WS pool before spawning many agents. */
export function setCloudDebugWsCap(max: number): void {
  cloudWsRegistry.setMaxSize(max);
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

/** Drop proxied cloud WS for one agent, or all when agentId is omitted. */
export function disconnectCloudEngineWs(agentId?: string): void {
  if (agentId) {
    const ws = cloudWsRegistry.remove(agentId);
    if (!ws) return;
    try {
      ws.disconnect();
    } catch {
      /* ignore */
    }
    return;
  }
  for (const ws of cloudWsRegistry.clear()) {
    try {
      ws.disconnect();
    } catch {
      /* ignore */
    }
  }
}

/** Shared WS for an agent. Local agents reuse the engine singleton. */
export function getAgentEngineWs(agent: Agent): EngineWebSocket {
  if (!isCloudAgent(agent)) {
    return getEngineWs();
  }
  const ws = cloudWsRegistry.get(agent.id);
  if (ws) {
    return ws;
  }
  throw new Error(
    `[runtime-router] cloud WebSocket for ${agent.id} is not connected yet`,
  );
}

/** Connect (or reuse) the cloud WS for this agent without disconnecting others. */
export async function ensureAgentEngineWs(agent: Agent): Promise<EngineWebSocket> {
  if (!isCloudAgent(agent)) {
    return getEngineWs();
  }
  const existing = cloudWsRegistry.get(agent.id);
  if (existing) {
    return existing;
  }
  cloudWsRegistry.evictOldestIfNeeded((ws) => {
    try {
      ws.disconnect();
    } catch {
      /* ignore */
    }
  });
  const token = await cloudSessionToken();
  const wsUrl = `${getAgentEngineWsUrl(agent)}?token=${encodeURIComponent(token)}`;
  const wsClient = { wsUrl: () => wsUrl };
  const ws = new EngineWebSocket(wsClient as HoustonClient);
  ws.connect();
  cloudWsRegistry.set(agent.id, ws);
  return ws;
}

/** Dev-only snapshot of proxied cloud WebSocket slots. */
export function cloudWsDebugSnapshot(): {
  maxSlots: number;
  connectedAgentIds: string[];
} {
  return {
    maxSlots: cloudWsRegistry.maxSlots(),
    connectedAgentIds: cloudWsRegistry.agentIds(),
  };
}
