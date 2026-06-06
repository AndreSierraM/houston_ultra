import { create } from "zustand";
import { registerAgentLookup } from "../lib/agent-lookup";
import {
  createCloudAgentWithBootstrap,
  type CloudAgentCreateResult,
} from "../lib/cloud-agent-create";
import {
  deleteCloudAgent,
  getCloudBaseUrl,
  isCloudConfigured,
  listCloudAgents,
  patchCloudAgent,
  pingCloudServer,
} from "../lib/cloud-client";
import { showErrorToast } from "../lib/error-toast";
import i18n from "../lib/i18n";
import {
  activateAgentRuntime,
  deactivateAgentRuntime,
} from "../lib/activate-agent-runtime";
import { isCloudAgent } from "../lib/runtime-router";
import { tauriAgents, tauriAttachments, tauriPreferences } from "../lib/tauri";
import type { AgentRuntimeMode } from "../lib/types";
import { useFeedStore } from "./feeds";
import { useDraftStore } from "./drafts";
import { analytics } from "../lib/analytics";
import type { Agent } from "../lib/types";

export interface CreatedAgent {
  agent: Agent;
}

interface AgentState {
  agents: Agent[];
  current: Agent | null;
  loading: boolean;
  loadAgents: (workspaceId: string, options?: { silent?: boolean }) => Promise<void>;
  setCurrent: (agent: Agent) => void;
  create: (
    workspaceId: string,
    name: string,
    configId: string,
    color?: string,
    claudeMd?: string,
    installedPath?: string,
    seeds?: Record<string, string>,
    existingPath?: string,
    runtime?: AgentRuntimeMode,
    provider?: string,
    model?: string,
    syncProviderCredentials?: boolean,
    syncComposioCredentials?: boolean,
  ) => Promise<
    CreatedAgent &
      Pick<
        CloudAgentCreateResult,
        | "credentialSync"
        | "credentialSyncError"
        | "composioCredentialSync"
        | "composioCredentialSyncError"
      >
  >;
  delete: (workspaceId: string, id: string) => Promise<void>;
  rename: (workspaceId: string, id: string, newName: string) => Promise<void>;
  updateColor: (workspaceId: string, id: string, color: string) => Promise<void>;
}

export const useAgentStore = create<AgentState>((set, get) => ({
  agents: [],
  current: null,
  loading: false,

  loadAgents: async (workspaceId, options) => {
    const silent = options?.silent ?? false;
    if (!silent) set({ loading: true });
    try {
      const cloudConfigured = isCloudConfigured();
      const [localAgents, cloudAgentsResult] = await Promise.all([
        tauriAgents.list(workspaceId),
        cloudConfigured
          ? listCloudAgents().catch(() => get().agents.filter((a) => isCloudAgent(a)))
          : Promise.resolve([] as Agent[]),
      ]);
      if (cloudConfigured && !silent) {
        void pingCloudServer().then((ping) => {
          if (!ping.ok) {
            showErrorToast(
              "cloud_ping",
              i18n.t("shell:runtimeMode.cloudUnreachable", {
                server: getCloudBaseUrl(),
              }),
            );
          }
        });
      }
      const cloudAgents = cloudAgentsResult;
      const cloudIds = new Set(cloudAgents.map((a) => a.id));
      const agents = [
        ...localAgents.filter((a) => !cloudIds.has(a.id) && !isCloudAgent(a)),
        ...cloudAgents,
      ];
      const prev = get().current;
      // If the previously-selected agent was deleted server-side it won't be in
      // the new list; fall back to the first agent (or null) instead of pinning
      // a ghost agent that no longer exists.
      const selected =
        agents.find((a) => a.id === prev?.id) ?? agents[0] ?? null;
      set({ agents, current: selected, loading: false });
      if (
        selected &&
        (prev?.id !== selected.id ||
          prev?.runtime !== selected.runtime ||
          prev?.folderPath !== selected.folderPath)
      ) {
        activateAgentRuntime(selected).catch((e) =>
          console.error("[runtime] Failed to re-activate after agent load:", e),
        );
      }
    } catch (e) {
      console.error("[agents] Failed to load:", e);
      set({ loading: false });
    }
  },

  setCurrent: (agent) => {
    const prev = get().current;
    if (prev && prev.id !== agent.id) {
      deactivateAgentRuntime(prev).catch((e) =>
        console.error("[runtime] Failed to deactivate agent harness:", e),
      );
    }
    set({ current: agent });
    tauriPreferences.set("last_agent_id", agent.id);
    activateAgentRuntime(agent).catch((e) =>
      console.error("[runtime] Failed to activate agent harness:", e),
    );
  },

  create: async (
    workspaceId: string,
    name: string,
    configId: string,
    color?: string,
    claudeMd?: string,
    installedPath?: string,
    seeds?: Record<string, string>,
    existingPath?: string,
    runtime: AgentRuntimeMode = "local",
    provider?: string,
    model?: string,
    syncProviderCredentials = false,
    syncComposioCredentials = false,
  ) => {
    const result =
      runtime === "cloud_24_7"
        ? await createCloudAgentWithBootstrap({
            name,
            configId,
            color,
            claudeMd,
            installedPath,
            seeds,
            provider,
            model,
            syncProviderCredentials,
            syncComposioCredentials,
          })
        : {
            agent: (
              await tauriAgents.create(
                workspaceId,
                name,
                configId,
                color,
                claudeMd,
                installedPath,
                seeds,
                existingPath,
              )
            ).agent,
            credentialSync: "skipped" as const,
            composioCredentialSync: "skipped" as const,
          };
    analytics.track("agent_created", { config_id: configId });
    const { agent } = result;
    const prev = get().current;
    if (prev && prev.id !== agent.id) {
      deactivateAgentRuntime(prev).catch((e) =>
        console.error("[runtime] Failed to deactivate agent harness:", e),
      );
    }
    set((s) => ({
      agents: [...s.agents, agent],
      current: agent,
    }));
    tauriPreferences.set("last_agent_id", agent.id);
    try {
      await activateAgentRuntime(agent);
    } catch (e) {
      console.error("[runtime] Failed to activate agent harness:", e);
    }
    return {
      agent,
      credentialSync: result.credentialSync,
      credentialSyncError: result.credentialSyncError,
      composioCredentialSync: result.composioCredentialSync,
      composioCredentialSyncError: result.composioCredentialSyncError,
    };
  },

  delete: async (workspaceId, id) => {
    const agent = get().agents.find((a) => a.id === id);
    const agentPath = agent?.folderPath;
    if (agent && isCloudAgent(agent)) {
      await deleteCloudAgent(id);
    } else {
      await tauriAgents.delete(workspaceId, id);
      await tauriAttachments.delete(`agent-${id}`).catch(() => {});
    }
    // Drop the feed store bucket for this agent so stale messages don't
    // linger in memory.
    if (agentPath) {
      useFeedStore.getState().clearAgent(agentPath);
    }
    // Clear the free-form chat draft for this agent.
    useDraftStore.getState().clearDraft(`chat-${id}`);
    set((s) => {
      const agents = s.agents.filter((a) => a.id !== id);
      const current =
        s.current?.id === id ? agents[0] ?? null : s.current;
      return { agents, current };
    });
  },

  rename: async (workspaceId, id, newName) => {
    const agent = get().agents.find((a) => a.id === id);
    if (agent && isCloudAgent(agent)) {
      const updated = await patchCloudAgent(id, { name: newName });
      set((s) => ({
        agents: s.agents.map((a) => (a.id === id ? updated : a)),
        current: s.current?.id === id ? updated : s.current,
      }));
      return;
    }
    // The engine renames the folder on disk, so folderPath changes too. Use
    // the returned record instead of patching only `name`, or the stale path
    // later reaches tauriWatcher.start and the watch fails with a "neither a
    // file nor a directory" error toast (#298).
    const updated = await tauriAgents.rename(workspaceId, id, newName);
    set((s) => ({
      agents: s.agents.map((a) => (a.id === id ? updated : a)),
    }));
    // If we renamed the agent we're viewing, re-select it so the file watcher
    // and routine scheduler repoint at the new folder (the old one is gone).
    if (get().current?.id === id) {
      get().setCurrent(updated);
    }
  },

  updateColor: async (workspaceId, id, color) => {
    const agent = get().agents.find((a) => a.id === id);
    const updated =
      agent && isCloudAgent(agent)
        ? await patchCloudAgent(id, { color })
        : await tauriAgents.updateColor(workspaceId, id, color);
    set((s) => ({
      agents: s.agents.map((a) => (a.id === id ? updated : a)),
      current: s.current?.id === id ? updated : s.current,
    }));
  },
}));

registerAgentLookup({
  agentFromPath(agentPath) {
    const { agents, current } = useAgentStore.getState();
    const byPath = agents.find((a) => a.folderPath === agentPath);
    if (byPath) return byPath;
    if (agentPath.startsWith("cloud://")) {
      const id = agentPath.slice("cloud://".length);
      return agents.find((a) => a.id === id) ?? (current?.id === id ? current : null);
    }
    if (current?.folderPath === agentPath) return current;
    return null;
  },
  currentAgent() {
    return useAgentStore.getState().current;
  },
});
