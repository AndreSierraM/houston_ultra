/**
 * Cloud-only send guard: block chat turns when the resolved provider is
 * confirmed unauthenticated on a remote engine (POST /sessions succeeds but
 * the CLI fails silently if WS drops the auth error).
 */

import { agentForEngine, resolveEngine, resolveEngineForPath } from "./engine-for-agent";
import { getProvider } from "./providers";
import { isCloudAgent } from "./runtime-router";
import { useUIStore } from "../stores/ui";
import type { Agent } from "./types";

const DEFAULT_PROVIDER_PREF_KEY = "default_provider";
const CONFIG_REL_PATH = ".houston/config/config.json";

async function resolveSendProvider(
  agentPath: string,
  providerOverride?: string,
  agentOverride?: Agent | null,
): Promise<{ engine: Awaited<ReturnType<typeof resolveEngineForPath>>; provider: string | null }> {
  const engine = await resolveEngine(agentOverride ?? agentForEngine(agentPath), agentPath);
  const trimmed = providerOverride?.trim();
  if (trimmed) return { engine, provider: trimmed };

  const raw = await engine.readAgentFile(agentPath, CONFIG_REL_PATH);
  if (raw) {
    try {
      const cfg = JSON.parse(raw) as { provider?: string };
      const fromConfig = cfg.provider?.trim();
      if (fromConfig) return { engine, provider: fromConfig };
    } catch {
      /* fall through */
    }
  }

  const pref = await engine.getPreference(DEFAULT_PROVIDER_PREF_KEY);
  return { engine, provider: pref?.trim() || null };
}

function providerNeedsAuth(status: {
  cliInstalled: boolean;
  authState: string;
}): boolean {
  return status.cliInstalled && status.authState === "unauthenticated";
}

/** Throws (and sets authRequired) when a cloud send would hit an unsigned-in provider. */
export async function assertCloudProviderAuthPreflight(
  agentPath: string,
  providerOverride?: string,
  agentOverride?: Agent | null,
): Promise<void> {
  const agent = agentOverride ?? agentForEngine(agentPath);
  if (!agent || !isCloudAgent(agent)) return;

  const { engine, provider } = await resolveSendProvider(agentPath, providerOverride, agent);
  if (!provider) return;

  const status = await engine.providerStatus(provider);
  if (!providerNeedsAuth(status)) return;

  useUIStore.getState().setAuthRequired(provider);
  const label = getProvider(provider)?.name ?? provider;
  throw new Error(`Sign in to ${label} before sending messages`);
}
