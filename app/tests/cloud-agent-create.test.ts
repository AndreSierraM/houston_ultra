import { deepStrictEqual } from "node:assert";
import { describe, it } from "node:test";
import { cloudCreatePlan } from "../src/lib/cloud-create-plan.ts";

describe("cloudCreatePlan", () => {
  it("local create skips bootstrap and credential sync", () => {
    deepStrictEqual(cloudCreatePlan("local", true), {
      needsBootstrap: false,
      syncCredentials: false,
    });
  });

  it("cloud create with sync opt-in runs both steps", () => {
    deepStrictEqual(cloudCreatePlan("cloud_24_7", true), {
      needsBootstrap: true,
      syncCredentials: true,
    });
  });

  it("cloud create with sync opt-out skips credential sync only", () => {
    deepStrictEqual(cloudCreatePlan("cloud_24_7", false), {
      needsBootstrap: true,
      syncCredentials: false,
    });
  });
});
