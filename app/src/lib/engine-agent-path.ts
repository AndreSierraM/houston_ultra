import type { Agent } from "./types";

const CLOUD_URI_PREFIX = "cloud://";

/** Cloud pod layout: `HOME=/data`, agent roots under `/data/workspace/…`. */
const CLOUD_ENGINE_FS_PREFIX = "/data/";

/** True when folderPath is a desktop-side placeholder, not a real filesystem path. */
export function isSyntheticCloudPath(path: string): boolean {
  return path.startsWith(CLOUD_URI_PREFIX);
}

/** True when `path` is a cloud engine path (never valid on the desktop sidecar). */
export function isCloudEngineFilesystemPath(path: string): boolean {
  return path.startsWith(CLOUD_ENGINE_FS_PREFIX);
}

/** Real engine filesystem path, or null when folderPath is synthetic (cloud://). */
export function engineAgentPath(agent: Agent): string | null {
  if (isSyntheticCloudPath(agent.folderPath)) {
    return null;
  }
  return agent.folderPath;
}

/** Real engine path for watcher/scheduler calls; throws when only cloud:// is available. */
export function resolveEngineAgentPath(agent: Agent): string {
  const path = engineAgentPath(agent);
  if (!path) {
    throw new Error(
      `Agent "${agent.name}" (${agent.id}) has no filesystem path; folderPath is synthetic (${agent.folderPath})`,
    );
  }
  return path;
}
