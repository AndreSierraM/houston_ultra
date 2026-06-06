import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { engineAgentPath } from "../../lib/engine-agent-path";
import { queryKeys } from "../../lib/query-keys";
import { tauriFiles } from "../../lib/tauri";
import type { Agent } from "../../lib/types";

export function useFiles(agent: Agent | undefined) {
  const path = agent ? engineAgentPath(agent) : null;
  return useQuery({
    queryKey: queryKeys.files(path ?? ""),
    queryFn: () => tauriFiles.list(path!, agent),
    enabled: !!path,
  });
}

export function useDeleteFile(agent: Agent | undefined) {
  const path = agent ? engineAgentPath(agent) : null;
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (relativePath: string) => tauriFiles.delete(path!, relativePath, agent),
    onSuccess: () => {
      if (path) qc.invalidateQueries({ queryKey: queryKeys.files(path) });
    },
  });
}

export function useRenameFile(agent: Agent | undefined) {
  const path = agent ? engineAgentPath(agent) : null;
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ relativePath, newName }: { relativePath: string; newName: string }) =>
      tauriFiles.rename(path!, relativePath, newName, agent),
    onSuccess: () => {
      if (path) qc.invalidateQueries({ queryKey: queryKeys.files(path) });
    },
  });
}

export function useCreateFolder(agent: Agent | undefined) {
  const path = agent ? engineAgentPath(agent) : null;
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (name: string) => tauriFiles.createFolder(path!, name, agent),
    onSuccess: () => {
      if (path) qc.invalidateQueries({ queryKey: queryKeys.files(path) });
    },
  });
}
