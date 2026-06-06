import { useCallback, useEffect } from "react";
import type { BurstRunSummary } from "../lib/cloud-debug-burst-state";
import {
  reconcileBurstRunState,
  summarizeBurstRun,
} from "../lib/cloud-debug-burst-state";
import type { CloudDebugBurstScenario } from "../lib/cloud-debug-burst";
import { DEFAULT_CLOUD_DEBUG_BURST_SCENARIO } from "../lib/cloud-debug-burst";
import { executeBurstRun } from "../lib/cloud-debug-burst-run-exec";
import {
  detachBurstEventBridges,
  markStoppedBurstSlots,
  requestBurstStop,
  stopBurstSessions,
} from "../lib/cloud-debug-burst-stop";
import { useCloudBurstRunStore } from "../stores/cloud-burst-run";
import { useAgentStore } from "../stores/agents";
import type { Agent } from "../lib/types";

export interface CloudBurstRunParams {
  workspaceId: string;
  count: number;
  configId: string;
  scenario: CloudDebugBurstScenario;
  provider: string;
  model: string;
}

export type CloudBurstRunResult =
  | { kind: "started"; agents: Agent[] }
  | { kind: "already_running"; summary: BurstRunSummary }
  | { kind: "partial"; agents: Agent[]; summary: BurstRunSummary }
  | { kind: "stopped"; summary: BurstRunSummary };

export function useCloudBurstRun() {
  const loadAgents = useAgentStore((s) => s.loadAgents);
  const running = useCloudBurstRunStore((s) => s.running);
  const slots = useCloudBurstRunStore((s) => s.slots);
  const hydrate = useCloudBurstRunStore((s) => s.hydrate);
  const setRunning = useCloudBurstRunStore((s) => s.setRunning);
  const setConfigId = useCloudBurstRunStore((s) => s.setConfigId);
  const reset = useCloudBurstRunStore((s) => s.reset);

  useEffect(() => {
    hydrate();
  }, [hydrate]);

  const summary = summarizeBurstRun(slots, running);

  const runBurst = useCallback(
    async (params: CloudBurstRunParams): Promise<CloudBurstRunResult> => {
      if (!params.scenario.greeting.trim() || !params.scenario.skillUserText.trim()) {
        throw new Error("Scenario prompts are required");
      }

      setConfigId(params.configId);

      let state = useCloudBurstRunStore.getState();
      const reconciled = reconcileBurstRunState({
        running: state.running,
        slots: state.slots,
      });
      if (reconciled.changed) {
        state.setRunning(reconciled.running);
        if (reconciled.slots !== state.slots) {
          state.setSlots(reconciled.slots);
        }
        state = useCloudBurstRunStore.getState();
      }

      const liveSummary = summarizeBurstRun(state.slots, state.running);
      if (liveSummary.isActive) {
        return { kind: "already_running", summary: liveSummary };
      }

      setRunning(true);
      try {
        const result = await executeBurstRun({
          ...params,
          slots: state.slots,
          loadAgents,
        });
        if (result.kind === "stopped") {
          return { kind: "stopped", summary: result.summary };
        }
        if (result.kind === "partial") {
          return {
            kind: "partial",
            agents: result.agents,
            summary: result.summary,
          };
        }
        return { kind: "started", agents: result.agents };
      } finally {
        setRunning(false);
      }
    },
    [loadAgents, setConfigId, setRunning],
  );

  const stopBurst = useCallback(async () => {
    const state = useCloudBurstRunStore.getState();
    if (!state.running && !summarizeBurstRun(state.slots, state.running).isActive) {
      return;
    }
    requestBurstStop();
    await stopBurstSessions(state.slots);
    detachBurstEventBridges();
    state.setSlots(markStoppedBurstSlots(state.slots));
    state.setRunning(false);
  }, []);

  return {
    running,
    slots,
    summary,
    runBurst,
    stopBurst,
    reset,
    defaultScenario: DEFAULT_CLOUD_DEBUG_BURST_SCENARIO,
  };
}
