import { deepStrictEqual, equal, ok } from "node:assert";
import { describe, it } from "node:test";
import {
  CLOUD_DEBUG_BURST_CONFIG_IDS,
  CLOUD_DEBUG_BURST_DEFAULT,
  CLOUD_DEBUG_BURST_SKILL_BY_CONFIG,
  burstAgentName,
  buildBurstBootstrapSeeds,
  buildBurstMissionPrompt,
  burstSkillUserText,
  clampBurstCount,
  createBurstSlots,
  feedItemPreview,
  feedPreviewLines,
  isBurstLabAgent,
  listBurstAgentsForCleanup,
  DEFAULT_CLOUD_DEBUG_BURST_SCENARIO,
} from "../src/lib/cloud-debug-burst.ts";
import type { Agent } from "../src/lib/types.ts";

function cloudAgent(name: string, id = crypto.randomUUID()): Agent {
  return {
    id,
    name,
    folderPath: `cloud://${id}`,
    configId: "support",
    createdAt: new Date().toISOString(),
    runtime: "cloud_24_7",
  };
}

describe("CLOUD_DEBUG_BURST_DEFAULT", () => {
  it("defaults to 5 agents", () => {
    equal(CLOUD_DEBUG_BURST_DEFAULT, 5);
  });
});

describe("burstAgentName", () => {
  it("uses config id and 1-based index", () => {
    equal(burstAgentName("support", 0), "support burst 1");
  });
});

describe("isBurstLabAgent", () => {
  it("matches burst lab names only", () => {
    ok(isBurstLabAgent(cloudAgent("support burst 1"), "support"));
    ok(!isBurstLabAgent(cloudAgent("support burst 1"), "sales"));
    ok(!isBurstLabAgent(cloudAgent("My support agent")));
  });
});

describe("listBurstAgentsForCleanup", () => {
  it("returns prior burst agents for the same config", () => {
    const cloudAgents = [
      cloudAgent("support burst 1"),
      cloudAgent("support burst 2"),
      cloudAgent("sales burst 1"),
      cloudAgent("regular agent"),
    ];
    const cleanup = listBurstAgentsForCleanup(cloudAgents, "support");
    equal(cleanup.length, 2);
  });
});

describe("clampBurstCount", () => {
  it("clamps to minimum 1 with no upper cap", () => {
    equal(clampBurstCount(0), 1);
    equal(clampBurstCount(8), 8);
    equal(clampBurstCount(99), 99);
    equal(clampBurstCount(500), 500);
  });
});

describe("CLOUD_DEBUG_BURST_SKILL_BY_CONFIG", () => {
  it("maps every seed config to a skill slug", () => {
    for (const configId of CLOUD_DEBUG_BURST_CONFIG_IDS) {
      ok(CLOUD_DEBUG_BURST_SKILL_BY_CONFIG[configId], configId);
    }
  });
});

describe("createBurstSlots", () => {
  it("uses placeholder session keys until createMission assigns activity-*", () => {
    const slots = createBurstSlots(2);
    equal(slots.length, 2);
    ok(!slots[0]!.sessionKey.startsWith("activity-"));
  });
});

describe("buildBurstMissionPrompt", () => {
  it("combines greeting and skill into one autonomous mission", () => {
    const prompt = buildBurstMissionPrompt(DEFAULT_CLOUD_DEBUG_BURST_SCENARIO, {
      name: "triage-a-ticket",
    });
    ok(prompt.includes(DEFAULT_CLOUD_DEBUG_BURST_SCENARIO.greeting));
    ok(prompt.includes("triage-a-ticket"));
    ok(prompt.includes(DEFAULT_CLOUD_DEBUG_BURST_SCENARIO.skillUserText));
  });
});

describe("buildBurstBootstrapSeeds", () => {
  it("seeds bookkeeping context required by log-an-expense step 1", () => {
    const seeds = buildBurstBootstrapSeeds("bookkeeping");
    ok(seeds["context/bookkeeping-context.md"]?.includes("Debug Burst"));
    ok(seeds["config/chart-of-accounts.json"]?.includes("99999"));
  });

  for (const configId of CLOUD_DEBUG_BURST_CONFIG_IDS) {
    it(`seeds context for ${configId}`, () => {
      const seeds = buildBurstBootstrapSeeds(configId);
      ok(Object.keys(seeds).length > 0);
      ok(seeds["config/context-ledger.json"]);
    });
  }
});

describe("burstSkillUserText", () => {
  it("guides sales burst to sales-health subject", () => {
    ok(burstSkillUserText("sales").includes("sales-health"));
  });
});

describe("feedItemPreview", () => {
  it("formats user and assistant lines", () => {
    equal(
      feedItemPreview({ feed_type: "user_message", data: "hello" }),
      "hello",
    );
    equal(
      feedItemPreview({ feed_type: "assistant_text", data: "world" }),
      "world",
    );
  });

  it("truncates tool preview", () => {
    const line = feedItemPreview({
      feed_type: "tool_call",
      data: { name: "Read", input: {} },
    });
    equal(line, "[tool] Read");
  });

  it("labels encoded skill user messages", () => {
    const wire =
      '<!--houston:skill {"skill":"triage-a-ticket","displayName":"Triage a Ticket"}-->\n\nUse the skill.';
    const line = feedItemPreview({ feed_type: "user_message", data: wire });
    equal(line, "[skill] Triage a Ticket");
  });
});

describe("feedPreviewLines", () => {
  it("keeps only last N preview lines", () => {
    const items = Array.from({ length: 30 }, (_, i) => ({
      feed_type: "assistant_text" as const,
      data: `line-${i}`,
    }));
    const lines = feedPreviewLines(items, 5);
    deepStrictEqual(lines, ["line-25", "line-26", "line-27", "line-28", "line-29"]);
  });
});
