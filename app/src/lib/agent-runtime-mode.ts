import type { Agent, AgentRuntimeMode } from "./types";

/** Resolve local vs cloud from agent record (runtime field or cloud:// path). */
export function agentRuntime(agent: Agent): AgentRuntimeMode {
  if (agent.runtime === "cloud_24_7") return "cloud_24_7";
  if (agent.folderPath.startsWith("cloud://")) return "cloud_24_7";
  return "local";
}

export function isCloudAgent(agent: Agent): boolean {
  return agentRuntime(agent) === "cloud_24_7";
}
