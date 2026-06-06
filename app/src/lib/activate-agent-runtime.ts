/**
 * Activates the full Houston harness for the selected agent's engine.
 *
 * Local and cloud agents share the same engine contract: file watcher,
 * routine scheduler, and (for cloud) a proxied WebSocket for events.
 * Provider API keys still spawn CLIs inside the target engine — they do
 * not bypass this path.
 */

import { ensureAgentEngineWs, isCloudAgent } from "./runtime-router";
import { tauriRoutines, tauriWatcher } from "./tauri";
import type { Agent } from "./types";

async function stopLocalWatcher(): Promise<void> {
  const { getEngine } = await import("./engine");
  await getEngine().stopAgentWatcher();
}

/** Start watcher, scheduler, and cloud WS for one agent. */
export async function activateAgentRuntime(agent: Agent): Promise<void> {
  if (isCloudAgent(agent)) {
    await ensureAgentEngineWs(agent);
  } else {
    // Avoid a stale local watcher pointing at a previous agent folder.
    await stopLocalWatcher().catch(() => undefined);
  }

  await Promise.all([
    tauriWatcher.start(agent.folderPath),
    tauriRoutines.startScheduler(agent.folderPath),
  ]);
}

/** Tear down harness resources when leaving an agent (best-effort). */
export async function deactivateAgentRuntime(agent: Agent | null): Promise<void> {
  if (!agent) return;

  if (isCloudAgent(agent)) {
    await tauriRoutines.stopScheduler(agent.folderPath).catch(() => undefined);
    return;
  }

  await Promise.all([
    tauriRoutines.stopScheduler(agent.folderPath).catch(() => undefined),
    stopLocalWatcher().catch(() => undefined),
  ]);
}
