/**
 * Activates the full Houston harness for the selected agent's engine.
 *
 * Local and cloud agents share the same engine contract: file watcher,
 * routine scheduler, and (for cloud) a proxied WebSocket for events.
 * Provider API keys still spawn CLIs inside the target engine — they do
 * not bypass this path.
 */

import { resolveEngine } from "./engine-for-agent";
import {
  disconnectCloudEngineWs,
  ensureAgentEngineWs,
  isCloudAgent,
} from "./runtime-router";
import { getEngine } from "./engine";
import {
  runtimeActivationPlan,
  runtimeDeactivationPlan,
} from "./runtime-activation-plan";
import type { Agent } from "./types";

export {
  runtimeActivationPlan,
  runtimeDeactivationPlan,
} from "./runtime-activation-plan";

async function stopLocalWatcher(): Promise<void> {
  await getEngine().stopAgentWatcher();
}

/** Start watcher, scheduler, and cloud WS for one agent. */
export async function activateAgentRuntime(agent: Agent): Promise<void> {
  const plan = runtimeActivationPlan(agent);

  if (plan.stopLocalWatcher) {
    await stopLocalWatcher().catch(() => undefined);
  }
  if (!isCloudAgent(agent)) {
    disconnectCloudEngineWs();
  } else {
    await ensureAgentEngineWs(agent);
  }

  const engine = await resolveEngine(agent, agent.folderPath);
  await Promise.all([
    engine.startAgentWatcher(agent.folderPath),
    engine.startRoutineScheduler(agent.folderPath),
  ]);
}

/** Tear down harness resources when leaving an agent (best-effort). */
export async function deactivateAgentRuntime(agent: Agent | null): Promise<void> {
  if (!agent) return;
  const plan = runtimeDeactivationPlan(agent);
  if (plan.disconnectCloudWs) {
    disconnectCloudEngineWs();
  }
  if (plan.stopLocalWatcher) {
    await stopLocalWatcher().catch(() => undefined);
  }
}
