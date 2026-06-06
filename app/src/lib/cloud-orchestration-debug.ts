import { runtimeActivationPlan } from "./runtime-activation-plan.ts";
import type { Agent } from "./types";

export const CLOUD_DEBUG_SEED_AGENTS = [
  { configId: "bookkeeping", name: "Debug Bookkeeping" },
  { configId: "operations", name: "Debug Operations" },
  { configId: "sales", name: "Debug Sales" },
  { configId: "support", name: "Debug Support" },
] as const;

export type OrchestrationStepState = "idle" | "active" | "ok" | "error" | "skip";

export interface OrchestrationStep {
  id: string;
  state: OrchestrationStepState;
  detail?: string;
}

export interface OrchestrationFlowInput {
  cloudConfigured: boolean;
  cloudAuthReady: boolean;
  controlPlaneOk: boolean;
  controlPlaneLatencyMs?: number;
  currentAgent: Agent | null;
  wsConnected: boolean;
  engineProxyOk: boolean;
  engineProxyLatencyMs?: number;
}

export function buildOrchestrationFlow(input: OrchestrationFlowInput): OrchestrationStep[] {
  const plan = input.currentAgent ? runtimeActivationPlan(input.currentAgent) : null;
  const steps: OrchestrationStep[] = [
    {
      id: "cloud-config",
      state: input.cloudConfigured ? "ok" : "error",
    },
    {
      id: "cloud-auth",
      state: !input.cloudConfigured
        ? "skip"
        : input.cloudAuthReady
          ? "ok"
          : "error",
    },
    {
      id: "control-plane",
      state: !input.cloudConfigured
        ? "skip"
        : input.controlPlaneOk
          ? "ok"
          : "error",
      detail:
        input.controlPlaneOk && input.controlPlaneLatencyMs != null
          ? `${input.controlPlaneLatencyMs}ms`
          : undefined,
    },
    {
      id: "select-agent",
      state: input.currentAgent ? "ok" : "idle",
      detail: input.currentAgent?.name,
    },
    {
      id: "activate-runtime",
      state: !input.currentAgent
        ? "skip"
        : plan
          ? "ok"
          : "idle",
      detail: plan
        ? plan.cloud
          ? "cloud harness"
          : "local harness"
        : undefined,
    },
    {
      id: "cloud-ws",
      state: !plan?.connectCloudWs
        ? "skip"
        : input.wsConnected
          ? "ok"
          : "error",
    },
    {
      id: "engine-proxy",
      state: !plan?.cloud
        ? "skip"
        : input.engineProxyOk
          ? "ok"
          : input.engineProxyOk === false && input.currentAgent
            ? "error"
            : "active",
      detail:
        input.engineProxyOk && input.engineProxyLatencyMs != null
          ? `${input.engineProxyLatencyMs}ms`
          : undefined,
    },
  ];
  return steps;
}

export function isCloudDebugPanelEnabled(): boolean {
  return import.meta.env.DEV;
}
