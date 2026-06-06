import type { Agent } from "./types";

function isCloudRuntime(agent: Agent): boolean {
  return agent.runtime === "cloud_24_7";
}

/** Which harness steps apply when selecting an agent (regression-tested). */
export function runtimeActivationPlan(agent: Agent): {
  cloud: boolean;
  stopLocalWatcher: boolean;
  connectCloudWs: boolean;
  disconnectCloudWs: boolean;
} {
  const cloud = isCloudRuntime(agent);
  return {
    cloud,
    // Sidecar engine keeps one watcher — always clear before repointing.
    stopLocalWatcher: true,
    connectCloudWs: cloud,
    // Local firehose uses the sidecar singleton, not the cloud proxy.
    disconnectCloudWs: !cloud,
  };
}

/** Tear-down when leaving an agent (best-effort). Schedulers stay running. */
export function runtimeDeactivationPlan(agent: Agent): {
  disconnectCloudWs: boolean;
  stopLocalWatcher: boolean;
} {
  return {
    disconnectCloudWs: isCloudRuntime(agent),
    stopLocalWatcher: !isCloudRuntime(agent),
  };
}
