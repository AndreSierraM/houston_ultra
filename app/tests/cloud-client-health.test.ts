import { equal } from "node:assert";
import { describe, it } from "node:test";
import { isEngineHealthOk } from "../src/lib/cloud-client.ts";

describe("isEngineHealthOk", () => {
  it("accepts engine JSON health response", () => {
    equal(
      isEngineHealthOk({
        status: "ok",
        version: "0.1.0",
        protocol: 1,
      }),
      true,
    );
  });

  it("rejects control-plane plain ok string and malformed bodies", () => {
    equal(isEngineHealthOk("ok"), false);
    equal(isEngineHealthOk({ status: "degraded" }), false);
    equal(isEngineHealthOk(null), false);
  });
});
