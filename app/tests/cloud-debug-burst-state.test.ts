import { deepStrictEqual, equal, ok } from "node:assert";
import { describe, it } from "node:test";
import {
  isActiveBurstPhase,
  reconcileBurstRunState,
  summarizeBurstRun,
} from "../src/lib/cloud-debug-burst-state.ts";
import type { BurstAgentSlot } from "../src/lib/cloud-debug-burst.ts";

function slot(phase: BurstAgentSlot["phase"], index = 0): BurstAgentSlot {
  return {
    index,
    agentId: "a1",
    name: `Burst ${index + 1}`,
    agentPath: "cloud://a1",
    sessionKey: `sk-${index}`,
    phase,
    error: null,
  };
}

describe("summarizeBurstRun", () => {
  it("detects active runs from slot phases", () => {
    const summary = summarizeBurstRun(
      [slot("listening"), slot("done", 1)],
      false,
    );
    equal(summary.running, 1);
    equal(summary.done, 1);
    ok(summary.isActive);
  });

  it("treats hook running flag as active even when slots are idle", () => {
    const summary = summarizeBurstRun([slot("pending")], true);
    ok(summary.isActive);
  });
});

describe("isActiveBurstPhase", () => {
  it("marks terminal phases inactive", () => {
    ok(isActiveBurstPhase("listening"));
    ok(!isActiveBurstPhase("done"));
    ok(!isActiveBurstPhase("error"));
  });
});

describe("summarizeBurstRun empty", () => {
  it("returns zero totals", () => {
    const summary = summarizeBurstRun([], false);
    deepStrictEqual(summary, {
      total: 0,
      running: 0,
      done: 0,
      error: 0,
      pending: 0,
      isActive: false,
    });
  });
});

describe("reconcileBurstRunState", () => {
  it("clears running flag when slots were lost", () => {
    const result = reconcileBurstRunState({ running: true, slots: [] });
    ok(result.changed);
    equal(result.running, false);
    deepStrictEqual(result.slots, []);
  });

  it("clears stale running when every slot is terminal", () => {
    const result = reconcileBurstRunState({
      running: true,
      slots: [slot("done"), slot("error", 1)],
    });
    ok(result.changed);
    equal(result.running, false);
  });

  it("revives running when slots are still active after reload", () => {
    const result = reconcileBurstRunState({
      running: false,
      slots: [slot("creating"), slot("creating", 1)],
    });
    ok(result.changed);
    equal(result.running, true);
  });
});
