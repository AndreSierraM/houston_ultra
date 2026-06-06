import { deepStrictEqual, throws } from "node:assert";
import { describe, it } from "node:test";
import {
  engineAgentPath,
  isCloudEngineFilesystemPath,
  isSyntheticCloudPath,
  resolveEngineAgentPath,
} from "../src/lib/engine-agent-path.ts";
import type { Agent } from "../src/lib/types.ts";

const localAgent: Agent = {
  id: "local-1",
  name: "Local",
  folderPath: "/Users/me/.houston/workspaces/Ws/Local",
  configId: "cfg",
  createdAt: "2026-01-01T00:00:00.000Z",
  runtime: "local",
};

const cloudRealPathAgent: Agent = {
  id: "cloud-1",
  name: "Cloud",
  folderPath: "/data/agents/cloud-1",
  configId: "cfg",
  createdAt: "2026-01-01T00:00:00.000Z",
  runtime: "cloud_24_7",
};

const cloudSyntheticAgent: Agent = {
  id: "cloud-uri",
  name: "Cloud URI",
  folderPath: "cloud://cloud-uri",
  configId: "cfg",
  createdAt: "2026-01-01T00:00:00.000Z",
};

describe("isSyntheticCloudPath", () => {
  it("detects cloud:// placeholders", () => {
    deepStrictEqual(isSyntheticCloudPath("cloud://agent-1"), true);
    deepStrictEqual(isSyntheticCloudPath("cloud://"), true);
  });

  it("rejects real filesystem paths", () => {
    deepStrictEqual(isSyntheticCloudPath("/data/agents/cloud-1"), false);
    deepStrictEqual(
      isSyntheticCloudPath("/Users/me/.houston/workspaces/Ws/Local"),
      false,
    );
  });
});

describe("isCloudEngineFilesystemPath", () => {
  it("detects cloud pod agent roots under /data", () => {
    deepStrictEqual(
      isCloudEngineFilesystemPath("/data/workspace/Cloud/Burst-ops-1"),
      true,
    );
    deepStrictEqual(isCloudEngineFilesystemPath("/data/.houston"), true);
  });

  it("rejects desktop-local and synthetic paths", () => {
    deepStrictEqual(
      isCloudEngineFilesystemPath("/Users/me/.houston/workspaces/Ws/Local"),
      false,
    );
    deepStrictEqual(isCloudEngineFilesystemPath("cloud://agent-1"), false);
  });
});

describe("engineAgentPath", () => {
  it("returns folderPath for local and provisioned cloud agents", () => {
    deepStrictEqual(engineAgentPath(localAgent), localAgent.folderPath);
    deepStrictEqual(
      engineAgentPath(cloudRealPathAgent),
      cloudRealPathAgent.folderPath,
    );
  });

  it("returns null for synthetic cloud:// folderPath", () => {
    deepStrictEqual(engineAgentPath(cloudSyntheticAgent), null);
  });
});

describe("resolveEngineAgentPath", () => {
  it("returns real folderPath when available", () => {
    deepStrictEqual(resolveEngineAgentPath(localAgent), localAgent.folderPath);
    deepStrictEqual(
      resolveEngineAgentPath(cloudRealPathAgent),
      cloudRealPathAgent.folderPath,
    );
  });

  it("throws when folderPath is synthetic", () => {
    throws(
      () => resolveEngineAgentPath(cloudSyntheticAgent),
      (err: unknown) => {
        deepStrictEqual(err instanceof Error, true);
        deepStrictEqual(
          (err as Error).message.includes("cloud-uri"),
          true,
        );
        deepStrictEqual(
          (err as Error).message.includes("synthetic"),
          true,
        );
        return true;
      },
    );
  });
});
