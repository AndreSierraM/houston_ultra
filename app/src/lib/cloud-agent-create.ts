import type {
  AgentBootstrapBundle,
  BuildAgentBootstrapRequest,
} from "@houston-ai/engine-client";
import { getEngine } from "./engine";
import { getAgentEngineClient } from "./runtime-router";
import {
  createCloudAgent,
  type CreateCloudAgentInput,
} from "./cloud-client";
import { isConfigProvider } from "../data/config";
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
}

export interface CloudAgentCreateResult {
  agent: Agent;
  credentialSync: CloudCredentialSyncOutcome;
  credentialSyncError?: string;
}

export async function buildCloudBootstrapBundle(
  input: Omit<CloudAgentCreateInput, "syncProviderCredentials">,
): Promise<AgentBootstrapBundle> {
  const body: BuildAgentBootstrapRequest = {
    configId: input.configId,
    name: input.name,
    color: input.color,
    claudeMd: input.claudeMd,
    installedPath: input.installedPath,
    seeds: input.seeds,
    ...(input.provider && isConfigProvider(input.provider)
      ? { provider: input.provider }
      : {}),
    model: input.model,
  };
  return getEngine().buildAgentBootstrapBundle(body);
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

export async function createCloudAgentWithBootstrap(
  input: CloudAgentCreateInput,
): Promise<CloudAgentCreateResult> {
  const bootstrapBundle = await buildCloudBootstrapBundle(input);
  const createInput: CreateCloudAgentInput = {
    name: input.name,
    configId: input.configId,
    color: input.color,
    claudeMd: input.claudeMd,
    bootstrapBundle,
    syncProviderCredentials: input.syncProviderCredentials,
    ...(input.provider && isConfigProvider(input.provider)
      ? { provider: input.provider }
      : {}),
    model: input.model,
  };
  const agent = await createCloudAgent(createInput);

  if (!input.syncProviderCredentials || !input.provider || !isConfigProvider(input.provider)) {
    return { agent, credentialSync: "skipped" };
  }

  try {
    await syncProviderCredentialsToCloudAgent(agent, input.provider);
    return { agent, credentialSync: "success" };
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return { agent, credentialSync: "failed", credentialSyncError: message };
  }
}
