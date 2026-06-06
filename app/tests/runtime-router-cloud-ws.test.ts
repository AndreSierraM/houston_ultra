import { deepStrictEqual, equal } from "node:assert";
import { describe, it } from "node:test";
import { CloudAgentWsRegistry } from "../src/lib/cloud-agent-ws-registry.ts";

interface MockEngineWebSocket {
  id: string;
  disconnectCalls: number;
  disconnect(): void;
}

function mockWs(id: string): MockEngineWebSocket {
  return {
    id,
    disconnectCalls: 0,
    disconnect() {
      this.disconnectCalls += 1;
    },
  };
}

describe("CloudAgentWsRegistry (ensureAgentEngineWs reuse map)", () => {
  it("reuses the same mock WS per agent id on repeated get", () => {
    const registry = new CloudAgentWsRegistry<MockEngineWebSocket>(4);
    const ws = mockWs("cloud-1");
    registry.set("cloud-1", ws);

    const first = registry.get("cloud-1");
    const second = registry.get("cloud-1");

    equal(first, ws);
    equal(second, ws);
    equal(registry.size(), 1);
  });

  it("keeps separate WS entries per agent id without disconnecting others", () => {
    const registry = new CloudAgentWsRegistry<MockEngineWebSocket>(4);
    const wsA = mockWs("agent-a");
    const wsB = mockWs("agent-b");
    registry.set("agent-a", wsA);
    registry.set("agent-b", wsB);

    deepStrictEqual(registry.get("agent-a"), wsA);
    deepStrictEqual(registry.get("agent-b"), wsB);
    equal(wsA.disconnectCalls, 0);
    equal(wsB.disconnectCalls, 0);
  });

  it("evicts the oldest entry when max size is reached", () => {
    const registry = new CloudAgentWsRegistry<MockEngineWebSocket>(2);
    const oldest = mockWs("oldest");
    const middle = mockWs("middle");
    const newest = mockWs("newest");
    registry.set("oldest", oldest);
    registry.set("middle", middle);

    registry.evictOldestIfNeeded((ws) => ws.disconnect());
    registry.set("newest", newest);

    equal(oldest.disconnectCalls, 1);
    equal(registry.get("oldest"), undefined);
    equal(registry.get("middle"), middle);
    equal(registry.get("newest"), newest);
    equal(registry.size(), 2);
  });

  it("remove disconnects only the targeted agent entry", () => {
    const registry = new CloudAgentWsRegistry<MockEngineWebSocket>(4);
    const wsA = mockWs("agent-a");
    const wsB = mockWs("agent-b");
    registry.set("agent-a", wsA);
    registry.set("agent-b", wsB);

    registry.remove("agent-a");

    equal(registry.get("agent-a"), undefined);
    equal(registry.get("agent-b"), wsB);
  });

  it("agentIds lists connected agent ids in LRU order", () => {
    const registry = new CloudAgentWsRegistry<MockEngineWebSocket>(4);
    registry.set("a", mockWs("a"));
    registry.set("b", mockWs("b"));
    deepStrictEqual(registry.agentIds(), ["a", "b"]);
  });

  it("setMaxSize shrinks pool and reports maxSlots", () => {
    const registry = new CloudAgentWsRegistry<MockEngineWebSocket>(4);
    registry.set("a", mockWs("a"));
    registry.set("b", mockWs("b"));
    registry.set("c", mockWs("c"));
    registry.setMaxSize(2);
    equal(registry.maxSlots(), 2);
    equal(registry.size(), 2);
  });
});
