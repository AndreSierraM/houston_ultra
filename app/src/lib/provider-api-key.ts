/**
 * Provider API-key connect. Keys are stored on the active agent's engine and
 * injected into CLI spawns (terminal-manager). This does not bypass the harness.
 */
import type { ProviderInfo } from "./providers";
import { tauriProvider } from "./tauri";

const DUAL_PATH_CONNECT_PROVIDER_IDS = new Set(["anthropic", "openai", "gemini"]);

/** CLI OAuth primary with optional API-key advanced section (Anthropic, OpenAI, Gemini). */
export function isDualPathConnectProvider(
  provider: ProviderInfo | null | undefined,
): boolean {
  return provider != null && DUAL_PATH_CONNECT_PROVIDER_IDS.has(provider.id);
}

/** API-key paste flow without a CLI OAuth path (OpenRouter today). */
export function isApiKeyOnlyProvider(provider: ProviderInfo | null | undefined): boolean {
  return provider?.loginKind === "apiKey" && !isDualPathConnectProvider(provider);
}

/** Whether `saveProviderApiKey` is wired for this provider today. */
export function supportsProviderApiKeySave(providerId: string): boolean {
  switch (providerId) {
    case "gemini":
    case "openrouter":
    case "anthropic":
    case "openai":
      return true;
    default:
      return false;
  }
}

/** Reconnect / error cards may offer API-key connect when a console URL exists. */
export function providerSupportsApiKeyConnect(
  provider: ProviderInfo | null | undefined,
): boolean {
  if (!provider) return false;
  if (isApiKeyOnlyProvider(provider)) return true;
  return isDualPathConnectProvider(provider) && !!provider.apiKeyConsoleUrl;
}

export async function saveProviderApiKey(providerId: string, apiKey: string): Promise<void> {
  switch (providerId) {
    case "gemini":
      await tauriProvider.setGeminiApiKey(apiKey);
      return;
    case "openrouter":
      await tauriProvider.setOpenRouterApiKey(apiKey);
      return;
    case "anthropic":
      await tauriProvider.setAnthropicApiKey(apiKey);
      return;
    case "openai":
      await tauriProvider.setOpenAiApiKey(apiKey);
      return;
    default:
      throw new Error(`Provider "${providerId}" does not support API key connect`);
  }
}
