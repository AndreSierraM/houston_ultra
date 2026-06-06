import { deepStrictEqual, equal } from "node:assert";
import { describe, it } from "node:test";
import {
  buildOrchestrationFlow,
  CLOUD_DEBUG_SEED_AGENTS,
} from "../src/lib/cloud-orchestration-debug.ts";
import type { Agent } from "../src/lib/types.ts";

const cloudAgent: Agent = {
  id: "agent-1",
  name: "Cloud One",
  folderPath: "cloud://agent-1",
  configId: "bookkeeping",
  runtime: "cloud_24_7",
  createdAt: "2026-01-01T00:00:00.000Z",
};

describe("buildOrchestrationFlow", () => {
  it("marks happy path steps ok when cloud agent is wired", () => {
    const steps = buildOrchestrationFlow({
      cloudConfigured: true,
      cloudAuthReady: true,
      controlPlaneOk: true,
      controlPlaneLatencyMs: 12,
      currentAgent: cloudAgent,
      wsConnected: true,
      engineProxyOk: true,
      engineProxyLatencyMs: 40,
    });

    equal(steps.find((s) => s.id === "cloud-ws")?.state, "ok");
    equal(steps.find((s) => s.id === "engine-proxy")?.state, "ok");
    equal(steps.find((s) => s.id === "engine-proxy")?.detail, "40ms");
  });

  it("skips cloud ws when no cloud agent is selected", () => {
    const steps = buildOrchestrationFlow({
      cloudConfigured: true,
      cloudAuthReady: true,
      controlPlaneOk: true,
      currentAgent: null,
      wsConnected: false,
      engineProxyOk: false,
    });

    equal(steps.find((s) => s.id === "cloud-ws")?.state, "skip");
    equal(steps.find((s) => s.id === "select-agent")?.state, "idle");
  });
});

describe("CLOUD_DEBUG_SEED_AGENTS", () => {
  it("lists four store agents for local e2e parity", () => {
    deepStrictEqual(
      CLOUD_DEBUG_SEED_AGENTS.map((a) => a.configId),
      ["bookkeeping", "operations", "sales", "support"],
    );
  });
});
