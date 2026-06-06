import type { Agent } from "./types";
import { agentFromPath } from "./agent-lookup";
import { isCloudAgent } from "./runtime-router";
import { looksLikeUrl } from "./open-href-detect";
import { tauriFiles, tauriSystem } from "./tauri";
import { logger } from "./logger";
import i18n from "./i18n";
import { useUIStore } from "../stores/ui";

export { looksLikeUrl, shouldBlockCloudFileOpen } from "./open-href-detect";

export function isCloudAgentAtPath(agentPath: string): boolean {
  const agent = agentFromPath(agentPath);
  return agent != null && isCloudAgent(agent);
}

export function resolveIsCloudAgent(
  agentPath: string,
  options?: { agent?: Agent; isCloud?: boolean },
): boolean {
  if (options?.isCloud !== undefined) return options.isCloud;
  if (options?.agent) return isCloudAgent(options.agent);
  return isCloudAgentAtPath(agentPath);
}

function showCloudOpenUnavailableToast(): void {
  useUIStore.getState().addToast({
    title: i18n.t("agents:files.cloudOpenUnavailable"),
    variant: "info",
  });
}

export function openAgentFile(
  agentPath: string,
  filePath: string,
  options?: { agent?: Agent; isCloud?: boolean },
): void {
  const trimmed = filePath.trim();
  if (!trimmed) return;
  if (resolveIsCloudAgent(agentPath, options)) {
    showCloudOpenUnavailableToast();
    return;
  }
  tauriFiles.open(agentPath, trimmed).catch((e) => {
    logger.warn(`[open-href] openFile(${trimmed}) failed: ${e}`);
  });
}

/**
 * Open a link the agent emitted in chat. Two shapes land here:
 *
 *   1. Absolute URLs — `https://...`, `http://...`, `mailto:...`, `houston://...`,
 *      `composio.dev/#houston_toolkit=...`, etc. These go to the system
 *      browser via `tauriSystem.openUrl`.
 *
 *   2. Relative or bare paths — e.g. `perfil.md`, `subfolder/output.docx`,
 *      `./report.pdf`. The agent's prompt structure encourages it to drop
 *      these straight after writing a file. They are NOT URLs; calling
 *      `openUrl("perfil.md")` on Windows silently does nothing
 *      (a real user reported "perfil.md pill doesn't open"). Resolve
 *      them against the current agent's working directory via
 *      `tauriFiles.open`, which goes through the engine's
 *      `open_file_in_agent` route and ends up at the OS's
 *      default-app handler.
 *
 * The detection is "does it look like a URL" — anything with a scheme
 * (`<word>:`) or starting with `//` is treated as a URL. Everything
 * else is a path.
 */
export function openAgentHref(
  href: string,
  agentPath: string,
  options?: { agent?: Agent; isCloud?: boolean },
): void {
  const trimmed = href.trim();
  if (!trimmed) return;
  if (looksLikeUrl(trimmed)) {
    tauriSystem.openUrl(trimmed).catch((e) => {
      logger.warn(`[open-href] openUrl(${trimmed}) failed: ${e}`);
    });
    return;
  }
  openAgentFile(agentPath, trimmed, options);
}
