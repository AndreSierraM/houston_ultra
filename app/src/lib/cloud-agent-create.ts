import type { AgentBootstrapBundle } from "@houston-ai/engine-client";
import { getEngine } from "./engine";
import { getAgentEngineClient } from "./runtime-router";
import { createCloudAgent, type CreateCloudAgentInput } from "./cloud-client";
import { isConfigProvider } from "../data/config";
import {
  buildCloudBootstrapRequest,
  cloudCredentialSyncAction,
  composioCredentialSyncAction,
  resolveComposioCredentialSyncOutcome,
  type ComposioCredentialSyncOutcome,
  type ComposioCredentialSyncSafeResult,
} from "./cloud-create-plan";
import type { Agent } from "./types";

export { cloudCreatePlan } from "./cloud-create-plan";

export type CloudCredentialSyncOutcome = "skipped" | "success" | "failed";

export interface CloudAgentCreateInput {
  name: string;
  configId: string;
  color?: string;
  claudeMd?: string;
  installedPath?: string;
  seeds?: Record<string, string>;
  provider?: string;
  model?: string;
  syncProviderCredentials: boolean;
  /** Copy local Composio session (user_data.json) to the cloud pod. Independent of provider sync. */
  syncComposioCredentials: boolean;
}

export interface CloudAgentCreateResult {
  agent: Agent;
  credentialSync: CloudCredentialSyncOutcome;
  credentialSyncError?: string;
  composioCredentialSync: ComposioCredentialSyncOutcome;
  composioCredentialSyncError?: string;
}

export type { ComposioCredentialSyncOutcome };

const COMPOSIO_PROVIDER_ID = "composio";

export async function buildCloudBootstrapBundle(
  input: Omit<CloudAgentCreateInput, "syncProviderCredentials">,
): Promise<AgentBootstrapBundle> {
  return getEngine().buildAgentBootstrapBundle(buildCloudBootstrapRequest(input));
}

export async function syncProviderCredentialsToCloudAgent(
  agent: Agent,
  provider: string,
): Promise<void> {
  const cloudEngine = await getAgentEngineClient(agent);
  const session = await cloudEngine.createProviderCredentialImportSession(provider);
  const exported = await getEngine().exportProviderCredentials(provider, {
    sessionId: session.sessionId,
    publicKey: session.publicKey,
  });
  await cloudEngine.importProviderCredentials(provider, {
    sessionId: session.sessionId,
    ciphertext: exported.ciphertext,
  });
}

export async function syncComposioCredentialsToCloudAgent(agent: Agent): Promise<void> {
  await syncProviderCredentialsToCloudAgent(agent, COMPOSIO_PROVIDER_ID);
}

export function canSyncProviderCredentialsToCloud(provider: string): boolean {
  return isConfigProvider(provider);
}

export type CloudCredentialSyncResult =
  | { ok: true }
  | {
      ok: false;
      /**
       * `no_local_credentials` = the local engine has no exportable key for this
       * provider, so there is nothing to sync (connect it on this device first).
       * `unsupported` = provider can't be cloud-synced. `error` = real failure.
       */
      reason: "unsupported" | "no_local_credentials" | "error";
      message: string;
    };

export async function syncComposioCredentialsToCloudAgentSafe(
  agent: Agent,
): Promise<ComposioCredentialSyncSafeResult> {
  try {
    await syncComposioCredentialsToCloudAgent(agent);
    return { ok: true };
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    if (/no exportable credentials/i.test(message)) {
      return { ok: false, reason: "no_local_credentials", message };
    }
    return { ok: false, reason: "error", message };
  }
}

export async function syncProviderCredentialsToCloudAgentSafe(
  agent: Agent,
  provider: string,
): Promise<CloudCredentialSyncResult> {
  if (!canSyncProviderCredentialsToCloud(provider)) {
    return {
      ok: false,
      reason: "unsupported",
      message: `Provider ${provider} does not support cloud credential sync`,
    };
  }
  try {
    await syncProviderCredentialsToCloudAgent(agent, provider);
    return { ok: true };
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    // The engine reports an empty export as "no exportable credentials found
    // for provider '<id>'" — that's not a failure to report, it just means the
    // provider isn't connected locally yet. Surface it as an actionable hint.
    if (/no exportable credentials/i.test(message)) {
      return { ok: false, reason: "no_local_credentials", message };
    }
    return { ok: false, reason: "error", message };
  }
}

function baseCreateInput(
  input: CloudAgentCreateInput,
  bootstrapBundle: AgentBootstrapBundle,
): CreateCloudAgentInput {
  return {
    name: input.name,
    configId: input.configId,
    color: input.color,
    claudeMd: input.claudeMd,
    bootstrapBundle,
    ...(input.provider && isConfigProvider(input.provider)
      ? { provider: input.provider }
      : {}),
    model: input.model,
  };
}

type ProviderCredentialSyncResult = Pick<
  CloudAgentCreateResult,
  "credentialSync" | "credentialSyncError"
>;

async function postCreateCredentialSync(
  agent: Agent,
  provider: string,
): Promise<ProviderCredentialSyncResult> {
  try {
    await syncProviderCredentialsToCloudAgent(agent, provider);
    return { credentialSync: "success" };
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return { credentialSync: "failed", credentialSyncError: message };
  }
}

export async function createCloudAgentWithBootstrap(
  input: CloudAgentCreateInput,
): Promise<CloudAgentCreateResult> {
  const bootstrapBundle = await buildCloudBootstrapBundle(input);
  const createInput = baseCreateInput(input, bootstrapBundle);
  const syncProvider = input.provider;
  const providerSyncAction = cloudCredentialSyncAction(
    input.syncProviderCredentials,
    syncProvider,
  );
  const composioSyncAction = composioCredentialSyncAction(input.syncComposioCredentials);

  const agent = await createCloudAgent(createInput);

  let credentialSync: CloudCredentialSyncOutcome = "skipped";
  let credentialSyncError: string | undefined;

  if (providerSyncAction === "sync" && syncProvider) {
    // Import session must live on the cloud engine (pod). Pre-create inline
    // sync cannot work: the client cannot encrypt for a session the pod has
    // not opened yet. Post-create uses cloud session + local export.
    const providerResult = await postCreateCredentialSync(agent, syncProvider);
    ({ credentialSync, credentialSyncError } = providerResult);
  }

  let composioCredentialSync: ComposioCredentialSyncOutcome = "skipped";
  let composioCredentialSyncError: string | undefined;

  if (composioSyncAction === "sync") {
    const composioResult = await syncComposioCredentialsToCloudAgentSafe(agent);
    composioCredentialSync = resolveComposioCredentialSyncOutcome(
      composioSyncAction,
      composioResult,
    );
    if (composioCredentialSync === "failed") {
      composioCredentialSyncError = composioResult.ok ? undefined : composioResult.message;
    }
  }

  return {
    agent,
    credentialSync,
    ...(credentialSyncError ? { credentialSyncError } : {}),
    composioCredentialSync,
    ...(composioCredentialSyncError ? { composioCredentialSyncError } : {}),
  };
}
