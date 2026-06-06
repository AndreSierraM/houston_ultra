import { deepStrictEqual } from "node:assert";
import { describe, it } from "node:test";
import {
  resolveBoardSendRoute,
  resolveComposerSubmitHandled,
  shouldAttemptComposerQueue,
} from "../src/components/board/board-send-decision.ts";

describe("resolveBoardSendRoute", () => {
  it("queues or sends immediately for the open session only", () => {
    deepStrictEqual(resolveBoardSendRoute("session-a", "session-a"), "queue-or-send");
    deepStrictEqual(resolveBoardSendRoute("session-b", "session-a"), "send-now");
    deepStrictEqual(resolveBoardSendRoute("session-a", null), "send-now");
  });
});

describe("shouldAttemptComposerQueue", () => {
  it("captures composer submit only for the active open session", () => {
    deepStrictEqual(shouldAttemptComposerQueue("session-a", "session-a", true), true);
    deepStrictEqual(shouldAttemptComposerQueue("session-a", "session-a", false), false);
    deepStrictEqual(shouldAttemptComposerQueue("session-b", "session-a", true), false);
    deepStrictEqual(shouldAttemptComposerQueue(undefined, "session-a", true), false);
  });
});

describe("resolveComposerSubmitHandled", () => {
  it("returns true only when queue capture succeeded", () => {
    deepStrictEqual(resolveComposerSubmitHandled(true, true), true);
    deepStrictEqual(resolveComposerSubmitHandled(true, false), false);
    deepStrictEqual(resolveComposerSubmitHandled(false, false), false);
  });
});
