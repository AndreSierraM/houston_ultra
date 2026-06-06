import { deepStrictEqual } from "node:assert";
import { describe, it } from "node:test";
import {
  buildCloudBootstrapRequest,
  buildCredentialSyncPayload,
  cloudCreatePlan,
  cloudCredentialSyncAction,
  composioCredentialSyncAction,
  resolveComposioCredentialSyncOutcome,
} from "../src/lib/cloud-create-plan.ts";

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

describe("buildCloudBootstrapRequest", () => {
  it("includes config provider in the bootstrap payload", () => {
    deepStrictEqual(
      buildCloudBootstrapRequest({
        configId: "cfg-1",
        name: "Cloud Agent",
        provider: "anthropic",
        model: "claude-opus-4-7",
      }),
      {
        configId: "cfg-1",
        name: "Cloud Agent",
        provider: "anthropic",
        model: "claude-opus-4-7",
      },
    );
  });

  it("drops unknown provider slugs from the bootstrap payload", () => {
    deepStrictEqual(
      buildCloudBootstrapRequest({
        configId: "cfg-1",
        name: "Cloud Agent",
        provider: "subq",
        model: "subq-model",
      }),
      {
        configId: "cfg-1",
        name: "Cloud Agent",
        model: "subq-model",
      },
    );
  });

  it("passes store installedPath and agentSeeds into the bootstrap payload", () => {
    deepStrictEqual(
      buildCloudBootstrapRequest({
        configId: "store-agent",
        name: "My Store Agent",
        color: "blue",
        claudeMd: "# Instructions",
        installedPath: "/Users/me/.houston/agents/store-agent",
        seeds: { "AGENTS.md": "# Seed" },
        provider: "openrouter",
        model: "anthropic/claude-sonnet-4",
      }),
      {
        configId: "store-agent",
        name: "My Store Agent",
        color: "blue",
        claudeMd: "# Instructions",
        installedPath: "/Users/me/.houston/agents/store-agent",
        seeds: { "AGENTS.md": "# Seed" },
        provider: "openrouter",
        model: "anthropic/claude-sonnet-4",
      },
    );
  });

  it("omits installedPath and seeds when the wizard has no store template", () => {
    deepStrictEqual(
      buildCloudBootstrapRequest({
        configId: "blank",
        name: "Custom Agent",
      }),
      {
        configId: "blank",
        name: "Custom Agent",
      },
    );
  });
});

describe("cloudCredentialSyncAction", () => {
  it("syncs when opt-in and provider is a config provider", () => {
    deepStrictEqual(cloudCredentialSyncAction(true, "openai"), "sync");
  });

  it("skips when user opted out of credential sync", () => {
    deepStrictEqual(cloudCredentialSyncAction(false, "anthropic"), "skip");
  });

  it("skips when provider is missing or not a config provider", () => {
    deepStrictEqual(cloudCredentialSyncAction(true, undefined), "skip");
    deepStrictEqual(cloudCredentialSyncAction(true, "subq"), "skip");
  });
});

describe("buildCredentialSyncPayload", () => {
  it("wraps provider and import body for control plane create", () => {
    deepStrictEqual(
      buildCredentialSyncPayload("anthropic", {
        sessionId: "sess-1",
        ciphertext: {
          version: 1,
          ephemeralPublicKey: "ephemeral",
          nonce: "nonce",
          ciphertext: "cipher",
        },
      }),
      {
        provider: "anthropic",
        importBody: {
          sessionId: "sess-1",
          ciphertext: {
            version: 1,
            ephemeralPublicKey: "ephemeral",
            nonce: "nonce",
            ciphertext: "cipher",
          },
        },
      },
    );
  });
});

describe("cloud agent credential sync timing", () => {
  it("requires post-create sync when opt-in (session must live on cloud pod)", () => {
    // Inline credentialSync on POST /cloud/agents cannot work: the import
    // session private key must be on the cloud engine, but the client only
    // has local engine access before the agent record exists. createCloudAgentWithBootstrap
    // always creates the agent first, then syncProviderCredentialsToCloudAgent.
    deepStrictEqual(cloudCredentialSyncAction(true, "anthropic"), "sync");
    deepStrictEqual(cloudCreatePlan("cloud_24_7", true).syncCredentials, true);
  });
});

describe("composioCredentialSyncAction", () => {
  it("syncs when user opted into credential sync", () => {
    deepStrictEqual(composioCredentialSyncAction(true), "sync");
  });

  it("skips when user opted out of composio sync", () => {
    deepStrictEqual(composioCredentialSyncAction(false), "skip");
  });

  it("is independent from provider credential sync opt-in", () => {
    deepStrictEqual(composioCredentialSyncAction(true), "sync");
    deepStrictEqual(composioCredentialSyncAction(false), "skip");
  });
});

describe("resolveComposioCredentialSyncOutcome", () => {
  it("skips when composio sync action is skip", () => {
    deepStrictEqual(resolveComposioCredentialSyncOutcome("skip", { ok: true }), "skipped");
  });

  it("maps a successful composio export/import to success", () => {
    deepStrictEqual(resolveComposioCredentialSyncOutcome("sync", { ok: true }), "success");
  });

  it("maps missing local composio credentials to no_local_credentials", () => {
    deepStrictEqual(
      resolveComposioCredentialSyncOutcome("sync", {
        ok: false,
        reason: "no_local_credentials",
        message: "no exportable credentials found for provider 'composio'",
      }),
      "no_local_credentials",
    );
  });

  it("maps real composio sync errors to failed", () => {
    deepStrictEqual(
      resolveComposioCredentialSyncOutcome("sync", {
        ok: false,
        reason: "error",
        message: "cloud engine unreachable",
      }),
      "failed",
    );
  });
});
