import { useQuery, useQueryClient } from "@tanstack/react-query";
import { queryKeys } from "../../lib/query-keys";
import { tauriConnections } from "../../lib/tauri";
import { normalizeToolkitSlugs } from "../../lib/composio-toolkits";

export function useConnections(agentPath?: string) {
  return useQuery({
    queryKey: queryKeys.connections(agentPath),
    queryFn: () => tauriConnections.list(agentPath),
    // Auth-state-sensitive query: its result depends on whether a valid
    // Composio token exists. If we refetch on window focus, a fetch can
    // start mid-OAuth (before the token is stored) and resolve with a stale
    // `needs_auth` that then races with — and overwrites — our explicit
    // post-auth reset. We invalidate explicitly after auth, connect, etc.
    refetchOnWindowFocus: false,
  });
}

export function useComposioApps(agentPath?: string) {
  return useQuery({
    queryKey: queryKeys.composioApps(agentPath),
    queryFn: () => tauriConnections.listApps(agentPath),
    staleTime: 1000 * 60 * 60,
  });
}

/**
 * List all connected toolkit slugs in the consumer namespace.
 * Uses `composio connections list` (single CLI call, no probing).
 */
export function useConnectedToolkits(enabled: boolean, agentPath?: string) {
  return useQuery({
    queryKey: queryKeys.connectedToolkits(agentPath),
    queryFn: async () =>
      normalizeToolkitSlugs(await tauriConnections.listConnectedToolkits(agentPath)),
    enabled,
    staleTime: 1000 * 60,
    refetchOnWindowFocus: false,
  });
}

export function useInvalidateConnections(agentPath?: string) {
  const qc = useQueryClient();
  return async () => {
    await Promise.all([
      qc.invalidateQueries({ queryKey: queryKeys.connections(agentPath) }),
      qc.invalidateQueries({ queryKey: queryKeys.composioApps(agentPath) }),
      qc.invalidateQueries({ queryKey: queryKeys.connectedToolkits(agentPath) }),
    ]);
  };
}

export function useResetConnections(agentPath?: string) {
  const qc = useQueryClient();
  return async () => {
    await Promise.all([
      qc.cancelQueries({ queryKey: queryKeys.connections(agentPath) }),
      qc.cancelQueries({ queryKey: queryKeys.composioApps(agentPath) }),
      qc.cancelQueries({ queryKey: queryKeys.connectedToolkits(agentPath) }),
    ]);
    await qc.resetQueries({ queryKey: queryKeys.connections(agentPath) });
    await Promise.all([
      qc.invalidateQueries({ queryKey: queryKeys.composioApps(agentPath) }),
      qc.invalidateQueries({ queryKey: queryKeys.connectedToolkits(agentPath) }),
    ]);
  };
}
