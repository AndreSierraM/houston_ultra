import type { FeedItem } from "@houston-ai/chat";
import { isCloudAgent } from "./agent-runtime-mode.ts";
import { CLOUD_DEBUG_SEED_AGENTS } from "./cloud-orchestration-debug.ts";
import type { Agent, SkillSummary } from "./types.ts";

const SKILL_MARKER = "<!--houston:skill ";

function previewSkillUserMessage(data: string): string | null {
  const start = data.indexOf(SKILL_MARKER);
  if (start < 0) return null;
  const end = data.indexOf("-->", start);
  if (end < 0) return null;
  try {
    const json = data.slice(start + SKILL_MARKER.length, end);
    const payload = JSON.parse(json) as { skill?: string; displayName?: string };
    const label = payload.displayName ?? payload.skill;
    return label ? `[skill] ${label}` : null;
  } catch {
    return null;
  }
}

export const CLOUD_DEBUG_BURST_CONFIG_IDS = CLOUD_DEBUG_SEED_AGENTS.map((a) => a.configId);

export const CLOUD_DEBUG_BURST_DEFAULT = 5;

export function burstAgentName(configId: string, index: number): string {
  return `${configId} burst ${index + 1}`;
}

/** Cloud agents created by the burst lab use `{configId} burst {n}`. */
export function isBurstLabAgent(agent: Agent, configId?: string): boolean {
  if (!isCloudAgent(agent)) return false;
  const match = /^(\S+)\s+burst\s+\d+$/.exec(agent.name);
  if (!match) return false;
  if (configId && match[1] !== configId) return false;
  return true;
}

/** Prior burst-lab agents for the same config, replaced before each run. */
export function listBurstAgentsForCleanup(
  cloudAgents: Agent[],
  configId: string,
  excludeAgentIds: ReadonlySet<string> = new Set(),
): Agent[] {
  return cloudAgents.filter(
    (a) => isBurstLabAgent(a, configId) && !excludeAgentIds.has(a.id),
  );
}

/** Store skill slug per config for the skill step of the debug scenario. */
export const CLOUD_DEBUG_BURST_SKILL_BY_CONFIG: Record<string, string> = {
  support: "triage-a-ticket",
  bookkeeping: "log-an-expense",
  operations: "brief-me",
  sales: "check-my-sales",
};

export interface CloudDebugBurstScenario {
  greeting: string;
  skillUserText: string;
}

const BURST_SKILL_USER_TEXT_BASE =
  "Debug burst run. Pre-seeded context files exist on disk. Follow the skill step 1 only (read context and required files), then stop and summarize. No external APIs. Do not ask the user questions.";

export const DEFAULT_CLOUD_DEBUG_BURST_SCENARIO: CloudDebugBurstScenario = {
  greeting:
    "Say hello in one short sentence. Do not use tools yet.",
  skillUserText: BURST_SKILL_USER_TEXT_BASE,
};

/** Skill step copy tuned per store config (burst lab load tests). */
export function burstSkillUserText(configId: string): string {
  if (configId === "sales") {
    return `${BURST_SKILL_USER_TEXT_BASE} Use check-my-sales with subject=sales-health.`;
  }
  return BURST_SKILL_USER_TEXT_BASE;
}

/** Minimal on-disk context so burst skills can complete step 1 without Composio. */
export function buildBurstBootstrapSeeds(configId: string): Record<string, string> {
  const ledger = JSON.stringify({
    universal: { company: { name: "Debug Burst Co" } },
  });

  switch (configId) {
    case "bookkeeping":
      return {
        "context/bookkeeping-context.md": [
          "# Debug burst bookkeeping",
          "",
          "Entity: Debug Burst Co LLC. Cash accounting. Fiscal year: calendar.",
          "Stage: seed. Team size: 1.",
        ].join("\n"),
        "config/context-ledger.json": JSON.stringify(
          {
            universal: {
              company: { name: "Debug Burst Co LLC", entityType: "LLC" },
              accountingMethod: "cash",
              suspenseCode: "99999",
            },
          },
          null,
          2,
        ),
        "config/chart-of-accounts.json": JSON.stringify(
          {
            accounts: [
              { code: "1000", name: "Cash", type: "asset", statementSection: "assets" },
              { code: "6000", name: "Other Expenses", type: "expense", statementSection: "opex" },
              { code: "99999", name: "Suspense", type: "expense", statementSection: "opex" },
            ],
          },
          null,
          2,
        ),
        "config/prior-categorizations.json": "[]",
        "config/party-rules.json": "{}",
      };
    case "support":
      return {
        "context/support-context.md": [
          "# Debug burst support",
          "",
          "Routing: bug, how-to, billing, other.",
          "Response tiers: P1 under 1 hour, P4 under 1 week.",
          "VIPs: none for debug burst.",
        ].join("\n"),
        "config/context-ledger.json": ledger,
        "customers.json": "[]",
        "conversations.json": "[]",
      };
    case "operations":
      return {
        "context/operations-context.md": [
          "# Debug burst operations",
          "",
          "Timezone: UTC. Priorities: debug burst validation.",
          "VIPs: none. Brief delivery: on demand.",
        ].join("\n"),
        "config/context-ledger.json": JSON.stringify(
          {
            universal: { company: { name: "Debug Burst Co" } },
            domains: { rhythm: { timezone: "UTC" } },
          },
          null,
          2,
        ),
      };
    case "sales":
      return {
        "context/sales-context.md": [
          "# Debug burst sales playbook",
          "",
          "ICP: seed-stage SaaS. Deal stages: lead, qualified, proposal, closed.",
          "Qualification: lightweight BANT for debug burst.",
        ].join("\n"),
        "config/context-ledger.json": ledger,
        "outputs.json": "[]",
      };
    default:
      return { "config/context-ledger.json": ledger };
  }
}

/** One autonomous mission prompt; the agent runs both steps in a single session. */
export function buildBurstMissionPrompt(
  scenario: CloudDebugBurstScenario,
  skill: Pick<SkillSummary, "name">,
): string {
  return [
    "Autonomous debug burst mission. Complete both steps in this session without asking the user.",
    "",
    "Step 1 - Greeting:",
    scenario.greeting,
    "",
    `Step 2 - Skill: invoke the "${skill.name}" skill and follow it with this context:`,
    scenario.skillUserText,
    "",
    "When finished, summarize what you did.",
  ].join("\n");
}

export type BurstAgentPhase =
  | "pending"
  | "creating"
  | "connecting"
  | "mission"
  | "listening"
  | "done"
  | "error";

export interface BurstAgentSlot {
  index: number;
  agentId: string | null;
  name: string;
  agentPath: string | null;
  sessionKey: string;
  phase: BurstAgentPhase;
  error: string | null;
}

export function clampBurstCount(raw: number): number {
  if (!Number.isFinite(raw)) return 1;
  return Math.max(1, Math.floor(raw));
}

export function createBurstSlots(count: number): BurstAgentSlot[] {
  return Array.from({ length: count }, (_, index) => ({
    index,
    agentId: null,
    name: `Burst ${index + 1}`,
    agentPath: null,
    sessionKey: crypto.randomUUID(),
    phase: "pending" as const,
    error: null,
  }));
}

/** One-line preview for debug chat cards. */
export function feedItemPreview(item: FeedItem): string | null {
  switch (item.feed_type) {
    case "user_message": {
      const skill = previewSkillUserMessage(item.data);
      if (skill) return skill;
      return item.data.trim() || null;
    }
    case "assistant_text":
    case "assistant_text_streaming":
      return item.data.trim() || null;
    case "thinking":
    case "thinking_streaming":
      return item.data.trim() ? `[thinking] ${item.data.trim()}` : null;
    case "tool_call":
      return `[tool] ${item.data.name}`;
    case "tool_result":
      return item.data.is_error
        ? `[tool error] ${item.data.content.slice(0, 120)}`
        : `[tool ok] ${item.data.content.slice(0, 120)}`;
    case "system_message":
      return item.data.trim() || null;
    case "provider_error":
      return `[error] ${"message" in item.data ? item.data.message : item.data.kind}`;
    case "tool_runtime_error":
      return `[error] ${item.data.details}`;
    case "final_result":
      return item.data.result.trim() || "[done]";
    case "file_changes": {
      const n = item.data.created.length + item.data.modified.length;
      return n > 0 ? `[files] ${n} changed` : null;
    }
    case "context_compacted":
      return "[context compacted]";
    default:
      return null;
  }
}

export function feedPreviewLines(items: FeedItem[], maxLines = 24): string[] {
  const lines: string[] = [];
  for (const item of items) {
    const line = feedItemPreview(item);
    if (line) lines.push(line);
  }
  return lines.slice(-maxLines);
}
