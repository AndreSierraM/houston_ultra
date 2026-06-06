import { deepStrictEqual } from "node:assert";
import { describe, it } from "node:test";
import {
  runtimeActivationPlan,
  runtimeDeactivationPlan,
} from "../src/lib/runtime-activation-plan.ts";
import type { Agent } from "../src/lib/types.ts";

const localAgent: Agent = {
  id: "local-1",
  name: "Local",
  folderPath: "/Users/me/.houston/workspaces/Ws/Local",
  configId: "cfg",
  createdAt: "2026-01-01T00:00:00.000Z",
  runtime: "local",
};

const cloudAgent: Agent = {
  id: "cloud-1",
  name: "Cloud",
  folderPath: "/data/agents/cloud-1",
  configId: "cfg",
  createdAt: "2026-01-01T00:00:00.000Z",
  runtime: "cloud_24_7",
};

const cloudUriAgent: Agent = {
  id: "cloud-uri",
  name: "Cloud URI",
  folderPath: "cloud://cloud-uri",
  configId: "cfg",
  createdAt: "2026-01-01T00:00:00.000Z",
};

describe("runtimeActivationPlan", () => {
  it("local agent clears cloud WS and repoints sidecar watcher", () => {
    deepStrictEqual(runtimeActivationPlan(localAgent), {
      cloud: false,
      stopLocalWatcher: true,
      connectCloudWs: false,
      disconnectCloudWs: true,
    });
  });

  it("cloud agent connects proxied WS and clears sidecar watcher", () => {
    deepStrictEqual(runtimeActivationPlan(cloudAgent), {
      cloud: true,
      stopLocalWatcher: true,
      connectCloudWs: true,
      disconnectCloudWs: false,
    });
  });

  it("cloud:// folderPath without runtime field is treated as cloud", () => {
    deepStrictEqual(runtimeActivationPlan(cloudUriAgent), {
      cloud: true,
      stopLocalWatcher: true,
      connectCloudWs: true,
      disconnectCloudWs: false,
    });
  });
});

describe("runtimeDeactivationPlan", () => {
  it("leaving cloud agent drops WS only (scheduler stays 24/7)", () => {
    deepStrictEqual(runtimeDeactivationPlan(cloudAgent), {
      disconnectCloudWs: true,
      stopLocalWatcher: false,
    });
  });

  it("leaving cloud:// agent drops proxied WS even without runtime field", () => {
    deepStrictEqual(runtimeDeactivationPlan(cloudUriAgent), {
      disconnectCloudWs: true,
      stopLocalWatcher: false,
    });
  });

  it("leaving local agent stops sidecar watcher only", () => {
    deepStrictEqual(runtimeDeactivationPlan(localAgent), {
      disconnectCloudWs: false,
      stopLocalWatcher: true,
    });
  });
});
