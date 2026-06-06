import type { AgentRuntimeMode } from "./cloud-client";

export function cloudCreatePlan(
  runtime: AgentRuntimeMode,
  syncOptIn: boolean,
): { needsBootstrap: boolean; syncCredentials: boolean } {
  const cloud = runtime === "cloud_24_7";
  return {
    needsBootstrap: cloud,
    syncCredentials: cloud && syncOptIn,
  };
}
