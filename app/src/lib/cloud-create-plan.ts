import type {
  BuildAgentBootstrapRequest,
  CredentialImportRequest,
} from "@houston-ai/engine-client";
import type { AgentRuntimeMode, CredentialSyncPayload } from "./cloud-client";

/** Must match `CONFIG_PROVIDERS` in `data/config.ts`. */
const BOOTSTRAP_PROVIDERS = ["anthropic", "openai", "openrouter"] as const;

function isBootstrapProvider(value: string): boolean {
  return (BOOTSTRAP_PROVIDERS as readonly string[]).includes(value);
}

export function cloudCreatePlan(
  runtime: AgentRuntimeMode,
  syncOptIn: boolean,
): { needsBootstrap: boolean; syncCredentials: boolean } {
  const cloud = runtime === "cloud_24_7";
  return {
    needsBootstrap: cloud,
    syncCredentials: cloud && syncOptIn,
  };
}

export interface CloudBootstrapRequestInput {
  configId: string;
  name: string;
  color?: string;
  claudeMd?: string;
  installedPath?: string;
  seeds?: Record<string, string>;
  provider?: string;
  model?: string;
}

/** Pure bootstrap payload for cloud agent create (regression-tested). */
export function buildCloudBootstrapRequest(
  input: CloudBootstrapRequestInput,
): BuildAgentBootstrapRequest {
  return {
    configId: input.configId,
    name: input.name,
    ...(input.color ? { color: input.color } : {}),
    ...(input.claudeMd ? { claudeMd: input.claudeMd } : {}),
    ...(input.installedPath ? { installedPath: input.installedPath } : {}),
    ...(input.seeds ? { seeds: input.seeds } : {}),
    ...(input.provider && isBootstrapProvider(input.provider)
      ? { provider: input.provider }
      : {}),
    ...(input.model ? { model: input.model } : {}),
  };
}

export type CloudCredentialSyncAction = "sync" | "skip";

/** Whether credential sync should run during cloud agent create. */
export function cloudCredentialSyncAction(
  syncOptIn: boolean,
  provider?: string,
): CloudCredentialSyncAction {
  if (!syncOptIn || !provider || !isBootstrapProvider(provider)) {
    return "skip";
  }
  return "sync";
}

/** Composio sync is opt-in separately from AI provider credential sync. */
export function composioCredentialSyncAction(
  syncComposioOptIn: boolean,
): CloudCredentialSyncAction {
  return syncComposioOptIn ? "sync" : "skip";
}

export type ComposioCredentialSyncOutcome =
  | "skipped"
  | "success"
  | "failed"
  | "no_local_credentials";

export type ComposioCredentialSyncSafeResult =
  | { ok: true }
  | {
      ok: false;
      reason: "no_local_credentials" | "error";
      message: string;
    };

/** Maps a post-create Composio sync attempt to the create result outcome. */
export function resolveComposioCredentialSyncOutcome(
  action: CloudCredentialSyncAction,
  result: ComposioCredentialSyncSafeResult | null,
): ComposioCredentialSyncOutcome {
  if (action === "skip") {
    return "skipped";
  }
  if (!result) {
    return "skipped";
  }
  if (result.ok) {
    return "success";
  }
  if (result.reason === "no_local_credentials") {
    return "no_local_credentials";
  }
  return "failed";
}

/** Wire payload for control plane `credentialSync` on POST /v1/cloud/agents. */
export function buildCredentialSyncPayload(
  provider: string,
  importBody: CredentialImportRequest,
): CredentialSyncPayload {
  return { provider, importBody };
}
