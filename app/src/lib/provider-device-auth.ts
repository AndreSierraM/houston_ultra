import { currentAgent } from "./agent-lookup";
import { osIsTauri } from "./os-bridge";
import { isCloudAgent } from "./runtime-router";

/** Headless device OAuth when the active engine cannot open the local browser. */
export function providerUsesDeviceAuth(): boolean {
  const agent = currentAgent();
  if (agent) return isCloudAgent(agent);
  return !osIsTauri();
}
