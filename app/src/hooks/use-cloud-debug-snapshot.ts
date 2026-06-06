import { useEffect, useRef, useState } from "react";
import {
  fetchCloudEntitlements,
  fetchCloudMe,
  hasCloudToken,
  isCloudConfigured,
  pingCloudAgentEngine,
  pingCloudServer,
  fetchCloudAgentStatus,
  type CloudAgentProvisionStatus,
} from "../lib/cloud-client";
import { isCloudAgent, cloudWsDebugSnapshot } from "../lib/runtime-router";
import { useAgentStore } from "../stores/agents";
import { isAuthConfigured } from "../lib/supabase";

export interface CloudAgentDebugRow {
  agentId: string;
  name: string;
  configId: string;
  folderPath: string;
  selected: boolean;
  wsConnected: boolean;
  provisionStatus: CloudAgentProvisionStatus | null;
  provisionError: string | null;
  engineOk: boolean | null;
  engineLatencyMs?: number;
  engineError: string | null;
}

export interface CloudDebugSnapshot {
  loading: boolean;
  cloudConfigured: boolean;
  cloudAuthReady: boolean;
  controlPlaneOk: boolean;
  controlPlaneLatencyMs?: number;
  orgId: string | null;
  orgRole: string | null;
  maxCloudAgents: number | null;
  wsSlots: { max: number; connectedAgentIds: string[] };
  agents: CloudAgentDebugRow[];
}

async function resolveCloudAuthReady(): Promise<boolean> {
  if (hasCloudToken()) return true;
  if (!isAuthConfigured()) return false;
  const { supabase } = await import("../lib/supabase");
  const { data } = await supabase.auth.getSession();
  return Boolean(data.session?.access_token);
}

export function useCloudDebugSnapshot(): CloudDebugSnapshot {
  const agents = useAgentStore((s) => s.agents);
  const current = useAgentStore((s) => s.current);
  const [loading, setLoading] = useState(true);
  const [cloudConfigured] = useState(isCloudConfigured());
  const [cloudAuthReady, setCloudAuthReady] = useState(false);
  const [controlPlaneOk, setControlPlaneOk] = useState(false);
  const [controlPlaneLatencyMs, setControlPlaneLatencyMs] = useState<number>();
  const [orgId, setOrgId] = useState<string | null>(null);
  const [orgRole, setOrgRole] = useState<string | null>(null);
  const [maxCloudAgents, setMaxCloudAgents] = useState<number | null>(null);
  const [rows, setRows] = useState<CloudAgentDebugRow[]>([]);
  const initializedRef = useRef(false);

  useEffect(() => {
    let cancelled = false;
    const delayMs = initializedRef.current ? 2_000 : 0;

    const timer = setTimeout(() => {
      void (async () => {
        const showSpinner = !initializedRef.current;
        if (showSpinner) setLoading(true);
        const wsSlots = cloudWsDebugSnapshot();
        const cloudAgents = agents.filter(isCloudAgent);

        if (!cloudConfigured) {
          if (!cancelled) {
            setRows([]);
            setLoading(false);
          }
          return;
        }

        const authReady = await resolveCloudAuthReady();
        if (cancelled) return;
        setCloudAuthReady(authReady);

        const ping = await pingCloudServer();
        if (cancelled) return;
        setControlPlaneOk(ping.ok);
        setControlPlaneLatencyMs(ping.latencyMs);

        if (ping.ok && authReady) {
          try {
            const [me, ent] = await Promise.all([fetchCloudMe(), fetchCloudEntitlements()]);
            if (!cancelled) {
              setOrgId(me.orgId);
              setOrgRole(me.orgRole);
              setMaxCloudAgents(ent.maxCloudAgents);
            }
          } catch {
            if (!cancelled) {
              setOrgId(null);
              setOrgRole(null);
              setMaxCloudAgents(null);
            }
          }
        }

        const nextRows: CloudAgentDebugRow[] = [];
        for (const agent of cloudAgents) {
          const row: CloudAgentDebugRow = {
            agentId: agent.id,
            name: agent.name,
            configId: agent.configId,
            folderPath: agent.folderPath,
            selected: current?.id === agent.id,
            wsConnected: wsSlots.connectedAgentIds.includes(agent.id),
            provisionStatus: null,
            provisionError: null,
            engineOk: null,
            engineError: null,
          };

          if (ping.ok && authReady) {
            try {
              const status = await fetchCloudAgentStatus(agent.id, { silent: true });
              row.provisionStatus = status.status;
            } catch (err) {
              row.provisionError = err instanceof Error ? err.message : String(err);
            }

            try {
              const enginePing = await pingCloudAgentEngine(agent.id);
              row.engineOk = enginePing.ok;
              row.engineLatencyMs = enginePing.latencyMs;
            } catch (err) {
              row.engineOk = false;
              row.engineError = err instanceof Error ? err.message : String(err);
            }
          }

          nextRows.push(row);
        }

        if (!cancelled) {
          setRows(nextRows);
          setLoading(false);
          initializedRef.current = true;
        }
      })();
    }, delayMs);

    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [agents, cloudConfigured, current?.id]);

  useEffect(() => {
    const timer = setInterval(() => {
      const ws = cloudWsDebugSnapshot();
      setRows((prev) => {
        if (prev.length === 0) return prev;
        let changed = false;
        const next = prev.map((row) => {
          const wsConnected = ws.connectedAgentIds.includes(row.agentId);
          const selected = current?.id === row.agentId;
          if (row.wsConnected === wsConnected && row.selected === selected) {
            return row;
          }
          changed = true;
          return { ...row, wsConnected, selected };
        });
        return changed ? next : prev;
      });
    }, 1_000);
    return () => clearInterval(timer);
  }, [current?.id]);

  return {
    loading,
    cloudConfigured,
    cloudAuthReady,
    controlPlaneOk,
    controlPlaneLatencyMs,
    orgId,
    orgRole,
    maxCloudAgents,
    wsSlots: {
      max: cloudWsDebugSnapshot().maxSlots,
      connectedAgentIds: cloudWsDebugSnapshot().connectedAgentIds,
    },
    agents: rows,
  };
}
