/**
 * Block sends when the resolved model is chat-only under the CLI harness.
 * Prevents models that hallucinate fake tool calls instead of using Codex bash.
 */

import { agentForEngine, resolveEngine } from "./engine-for-agent";
import { getDefaultModel, getModel, modelSupportsAgenticTools, normalizeLegacyModel, validModelOrNull } from "./providers";
import type { Agent } from "./types";

const CONFIG_REL_PATH = ".houston/config/config.json";

async function resolveSendModel(
  agentPath: string,
  provider: string,
  modelOverride?: string,
  agentOverride?: Agent | null,
): Promise<{ provider: string; model: string; label: string }> {
  const trimmedOverride = modelOverride?.trim();
  if (trimmedOverride) {
    const model = getModel(provider, trimmedOverride);
    return {
      provider,
      model: trimmedOverride,
      label: model?.label ?? trimmedOverride,
    };
  }

  const engine = await resolveEngine(agentOverride ?? agentForEngine(agentPath), agentPath);
  const raw = await engine.readAgentFile(agentPath, CONFIG_REL_PATH);
  if (raw) {
    try {
      const cfg = JSON.parse(raw) as { model?: string };
      const fromConfig = normalizeLegacyModel(cfg.model?.trim() ?? null);
      const valid = validModelOrNull(provider, fromConfig);
      if (valid) {
        const model = getModel(provider, valid);
        return { provider, model: valid, label: model?.label ?? valid };
      }
    } catch {
      /* fall through */
    }
  }

  const fallback = getDefaultModel(provider);
  const model = getModel(provider, fallback);
  return { provider, model: fallback, label: model?.label ?? fallback };
}

/** Throws when the resolved model cannot run agent tools under the CLI harness. */
export async function assertAgenticModelPreflight(
  agentPath: string,
  providerOverride?: string,
  modelOverride?: string,
  agentOverride?: Agent | null,
): Promise<void> {
  const provider = providerOverride?.trim();
  if (!provider) return;

  const resolved = await resolveSendModel(agentPath, provider, modelOverride, agentOverride);
  if (modelSupportsAgenticTools(resolved.provider, resolved.model)) return;

  throw new Error(
    `${resolved.label} is chat-only and cannot run bash, web search, or file tools. ` +
      `Switch to Claude Sonnet 4 or GPT-4.1 in the model selector.`,
  );
}
