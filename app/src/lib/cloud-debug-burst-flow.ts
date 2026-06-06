import { createMission } from "./create-mission.ts";
import { queryKeys } from "./query-keys.ts";
import { queryClient } from "./query-client.ts";
import { engineAgentPath } from "./engine-agent-path.ts";
import { getAgentEngineClient } from "./runtime-router.ts";
import { tauriSkills } from "./tauri.ts";
import { useFeedStore } from "../stores/feeds.ts";
import type { FeedItem } from "@houston-ai/chat";
import {
  getSessionStatusKey,
  useSessionStatusStore,
  type SessionRunStatus,
} from "../stores/session-status.ts";
import type { Agent } from "./types.ts";
import type { SkillSummary } from "./types.ts";
import {
  buildBurstMissionPrompt,
  CLOUD_DEBUG_BURST_SKILL_BY_CONFIG,
  type CloudDebugBurstScenario,
} from "./cloud-debug-burst.ts";
import {
  assertBurstNotStopped,
  isBurstStopRequested,
} from "./cloud-debug-burst-stop.ts";

const MISSION_WAIT_MS = 300_000;
const SESSION_POLL_MS = 400;

/** Last provider or runtime error from the burst session feed, if any. */
export function extractBurstSessionError(
  agentPath: string,
  sessionKey: string,
): string | null {
  const items: FeedItem[] =
    useFeedStore.getState().items[agentPath]?.[sessionKey] ?? [];
  for (let i = items.length - 1; i >= 0; i--) {
    const item = items[i]!;
    if (item.feed_type === "provider_error") {
      return "message" in item.data ? item.data.message : item.data.kind;
    }
    if (item.feed_type === "tool_runtime_error") {
      return item.data.details;
    }
  }
  return null;
}

function burstEnginePath(agent: Agent): string {
  const path = engineAgentPath(agent);
  if (!path) {
    throw new Error(
      `Agent "${agent.name}" (${agent.id}) is still provisioning; no engine path yet`,
    );
  }
  return path;
}

function sessionStatusKey(agentPath: string, sessionKey: string): string {
  return getSessionStatusKey(agentPath, sessionKey);
}

function sessionTurnStarted(
  baseline: SessionRunStatus | undefined,
  status: SessionRunStatus | undefined,
): boolean {
  if (status === "starting" || status === "running") return true;
  return status !== baseline && status !== undefined;
}

export async function waitForSessionTerminal(
  agentPath: string,
  sessionKey: string,
  timeoutMs = MISSION_WAIT_MS,
  baseline?: SessionRunStatus,
): Promise<"completed" | "error" | "timeout" | "stopped"> {
  const key = sessionStatusKey(agentPath, sessionKey);
  const turnBaseline =
    baseline ?? useSessionStatusStore.getState().statuses[key];
  const deadline = Date.now() + timeoutMs;
  let sawTurn = false;

  while (Date.now() < deadline) {
    if (isBurstStopRequested()) return "stopped";
    const status = useSessionStatusStore.getState().statuses[key];
    if (sessionTurnStarted(turnBaseline, status)) {
      sawTurn = true;
    }
    if (sawTurn && status === "completed") return "completed";
    if (sawTurn && status === "error") return "error";
    await new Promise((r) => setTimeout(r, SESSION_POLL_MS));
  }
  return "timeout";
}

export async function resolveBurstSkill(
  agent: Agent,
  configId: string,
): Promise<SkillSummary> {
  const preferred = CLOUD_DEBUG_BURST_SKILL_BY_CONFIG[configId];
  const skills = await tauriSkills.list(burstEnginePath(agent));
  if (preferred) {
    const hit = skills.find((s) => s.name === preferred);
    if (hit) return hit;
  }
  const featured = skills.find((s) => s.featured);
  if (featured) return featured;
  if (skills[0]) return skills[0];
  throw new Error(`No skills on agent ${agent.name} (${configId})`);
}

/** Warm engine client cache so skill list hits the cloud proxy. */
export async function warmBurstAgentEngine(agent: Agent): Promise<void> {
  await getAgentEngineClient(agent);
}

export type BurstFlowStepResult =
  | { ok: true; sessionKey: string }
  | {
      ok: false;
      reason: "timeout" | "error" | "stopped";
      detail?: string;
      sessionKey?: string;
    };

export interface BurstMissionProgress {
  phase?: "mission" | "listening";
  sessionKey?: string;
  agentPath?: string;
}

/**
 * Create one board mission per burst slot, then listen on WS until the agent
 * finishes autonomously (greeting + skill in a single session).
 */
export async function runBurstMissionListen(
  agent: Agent,
  configId: string,
  scenario: CloudDebugBurstScenario,
  provider: string,
  model: string,
  onProgress: (update: BurstMissionProgress) => void,
): Promise<BurstFlowStepResult> {
  const path = burstEnginePath(agent);
  onProgress({ phase: "mission", agentPath: path });
  assertBurstNotStopped();

  const skill = await resolveBurstSkill(agent, configId);
  assertBurstNotStopped();
  const missionText = buildBurstMissionPrompt(scenario, skill);

  const { sessionKey } = await createMission(
    {
      id: agent.id,
      name: agent.name,
      color: agent.color,
      folderPath: path,
    },
    missionText,
    {
      title: `Burst: ${skill.name}`,
      titleText: scenario.greeting,
      providerOverride: provider,
      modelOverride: model,
      routeAgent: agent,
    },
  );

  onProgress({ sessionKey, agentPath: path, phase: "mission" });
  void queryClient.invalidateQueries({ queryKey: queryKeys.activity(path) });
  void queryClient.invalidateQueries({ queryKey: ["all-conversations"] });

  useFeedStore.getState().pushFeedItem(path, sessionKey, {
    feed_type: "user_message",
    data: missionText,
  });

  onProgress({ phase: "listening" });
  const baseline =
    useSessionStatusStore.getState().statuses[sessionStatusKey(path, sessionKey)];
  const outcome = await waitForSessionTerminal(
    path,
    sessionKey,
    MISSION_WAIT_MS,
    baseline,
  );

  if (outcome === "timeout") {
    return { ok: false, reason: "timeout", sessionKey };
  }
  if (outcome === "stopped") {
    return { ok: false, reason: "stopped", sessionKey };
  }
  if (outcome === "error") {
    const detail = extractBurstSessionError(path, sessionKey);
    return { ok: false, reason: "error", sessionKey, detail: detail ?? undefined };
  }

  return { ok: true, sessionKey };
}
