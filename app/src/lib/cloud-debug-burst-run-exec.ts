import { activateAgentRuntime } from "./activate-agent-runtime.ts";
import { createCloudAgentWithBootstrap } from "./cloud-agent-create.ts";
import {
  deleteCloudAgent,
  listCloudAgents,
  waitForCloudEngineReady,
} from "./cloud-client.ts";
import {
  isActiveBurstPhase,
  refreshBurstSlotsFromSessions,
  summarizeBurstRun,
  type BurstRunSummary,
} from "./cloud-debug-burst-state.ts";
import {
  buildBurstBootstrapSeeds,
  burstAgentName,
  clampBurstCount,
  createBurstSlots,
  listBurstAgentsForCleanup,
  type BurstAgentSlot,
  type CloudDebugBurstScenario,
} from "./cloud-debug-burst.ts";
import { attachBurstAgentEventBridge } from "./cloud-debug-burst-events.ts";
import {
  runBurstMissionListen,
  warmBurstAgentEngine,
} from "./cloud-debug-burst-flow.ts";
import {
  assertBurstNotStopped,
  beginBurstRun,
  BURST_STOPPED_MESSAGE,
  detachBurstEventBridges,
  isBurstStopRequested,
  registerBurstEventUnsub,
} from "./cloud-debug-burst-stop.ts";
import { ensureAgentEngineWs, setCloudDebugWsCap } from "./runtime-router.ts";
import { useAgentStore } from "../stores/agents.ts";
import { useCloudBurstRunStore } from "../stores/cloud-burst-run.ts";
import type { Agent } from "./types.ts";

export interface ExecuteBurstRunInput {
  workspaceId: string;
  count: number;
  configId: string;
  scenario: CloudDebugBurstScenario;
  provider: string;
  model: string;
  slots: BurstAgentSlot[];
  loadAgents: (workspaceId: string, opts: { silent: boolean }) => Promise<void>;
}

export type ExecuteBurstRunResult =
  | { kind: "started"; agents: Agent[]; summary: BurstRunSummary }
  | { kind: "partial"; agents: Agent[]; summary: BurstRunSummary }
  | { kind: "stopped"; agents: Agent[]; summary: BurstRunSummary };

function patchSlot(
  slots: BurstAgentSlot[],
  index: number,
  patch: Partial<BurstAgentSlot>,
): BurstAgentSlot[] {
  return slots.map((s) => (s.index === index ? { ...s, ...patch } : s));
}

function activeSlotAgentIds(slots: BurstAgentSlot[]): Set<string> {
  const ids = new Set<string>();
  for (const slot of slots) {
    if (slot.agentId && isActiveBurstPhase(slot.phase)) {
      ids.add(slot.agentId);
    }
  }
  return ids;
}

/** Burst slots send chat before the run finishes; register each agent immediately. */
function registerBurstAgentInStore(agent: Agent): void {
  useAgentStore.setState((s) => {
    const existing = s.agents.find((a) => a.id === agent.id);
    if (existing) {
      return {
        agents: s.agents.map((a) => (a.id === agent.id ? { ...a, ...agent } : a)),
      };
    }
    return { agents: [...s.agents, agent] };
  });
}

export async function executeBurstRun(
  input: ExecuteBurstRunInput,
): Promise<ExecuteBurstRunResult> {
  beginBurstRun();
  const count = clampBurstCount(input.count);
  const cloudAgents = await listCloudAgents();
  const burstAgentsToDelete = listBurstAgentsForCleanup(
    cloudAgents,
    input.configId,
    activeSlotAgentIds(input.slots),
  );

  for (const agent of burstAgentsToDelete) {
    await deleteCloudAgent(agent.id);
  }
  if (burstAgentsToDelete.length > 0) {
    useAgentStore.setState((s) => ({
      agents: s.agents.filter(
        (a) => !burstAgentsToDelete.some((d) => d.id === a.id),
      ),
      current: burstAgentsToDelete.some((d) => d.id === s.current?.id)
        ? null
        : s.current,
    }));
  }

  setCloudDebugWsCap(count);
  const initial = createBurstSlots(count);
  let latestSlots = initial;
  const burstStore = useCloudBurstRunStore.getState();
  burstStore.setSlots(initial);

  const trackSlot = (index: number, patch: Partial<BurstAgentSlot>) => {
    latestSlots = patchSlot(latestSlots, index, patch);
    burstStore.patchSlot(index, patch);
  };

  async function runOneSlot(slot: BurstAgentSlot): Promise<Agent | null> {
    try {
      assertBurstNotStopped();
      trackSlot(slot.index, { phase: "creating" });
      const name = burstAgentName(input.configId, slot.index);
      const {
        agent,
        credentialSync,
        credentialSyncError,
      } = await createCloudAgentWithBootstrap({
        name,
        configId: input.configId,
        provider: input.provider,
        model: input.model,
        seeds: buildBurstBootstrapSeeds(input.configId),
        syncProviderCredentials: true,
        syncComposioCredentials: false,
      });
      if (credentialSync === "failed") {
        throw new Error(
          credentialSyncError ??
            `Failed to sync ${input.provider} credentials to the cloud agent`,
        );
      }
      registerBurstAgentInStore(agent);
      assertBurstNotStopped();

      trackSlot(slot.index, {
        agentId: agent.id,
        name: agent.name,
        agentPath: agent.folderPath,
        phase: "connecting",
      });

      await waitForCloudEngineReady(agent.id);
      assertBurstNotStopped();
      await activateAgentRuntime(agent);
      await warmBurstAgentEngine(agent);
      await ensureAgentEngineWs(agent);
      registerBurstEventUnsub(await attachBurstAgentEventBridge(agent));
      assertBurstNotStopped();

      trackSlot(slot.index, { phase: "mission" });
      const flow = await runBurstMissionListen(
        agent,
        input.configId,
        input.scenario,
        input.provider,
        input.model,
        (update) =>
          trackSlot(slot.index, {
            ...(update.phase ? { phase: update.phase } : {}),
            ...(update.sessionKey ? { sessionKey: update.sessionKey } : {}),
            ...(update.agentPath ? { agentPath: update.agentPath } : {}),
          }),
      );

      if (!flow.ok) {
        const detail =
          flow.reason === "stopped"
            ? BURST_STOPPED_MESSAGE
            : flow.reason === "timeout"
              ? "Session timed out waiting for the agent"
              : flow.detail ?? "Session ended with an error";
        trackSlot(slot.index, { phase: "error", error: detail });
        return null;
      }

      trackSlot(slot.index, { phase: "done" });
      return agent;
    } catch (err) {
      if (isBurstStopRequested()) {
        trackSlot(slot.index, { phase: "error", error: BURST_STOPPED_MESSAGE });
        return null;
      }
      const message = err instanceof Error ? err.message : String(err);
      trackSlot(slot.index, { phase: "error", error: message });
      return null;
    }
  }

  let results: PromiseSettledResult<Agent | null>[] = [];
  try {
    results = await Promise.allSettled(initial.map((slot) => runOneSlot(slot)));
  } finally {
    detachBurstEventBridges();
  }
  const created: Agent[] = [];
  for (const result of results) {
    if (result.status === "fulfilled" && result.value) {
      created.push(result.value);
    }
  }

  if (created.length > 0) {
    useAgentStore.setState((s) => {
      const merged = [...s.agents];
      for (const agent of created) {
        if (!merged.some((a) => a.id === agent.id)) {
          merged.push(agent);
        }
      }
      return { agents: merged };
    });
  }

  await input.loadAgents(input.workspaceId, { silent: true });

  const summary = summarizeBurstRun(refreshBurstSlotsFromSessions(latestSlots), false);
  if (isBurstStopRequested()) {
    return { kind: "stopped", agents: created, summary };
  }
  if (summary.error > 0 && summary.done > 0) {
    return { kind: "partial", agents: created, summary };
  }
  if (summary.error > 0 && summary.done === 0) {
    throw new Error("Every burst slot failed");
  }
  return { kind: "started", agents: created, summary };
}
