import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Activity, Bot, Layers, LayoutGrid, Server, Workflow } from "lucide-react";
import { Button, Spinner } from "@houston-ai/core";
import { SidebarSectionNav } from "../shared/sidebar-section-nav";
import { useCloudDebugSnapshot } from "../../hooks/use-cloud-debug-snapshot";
import {
  buildOrchestrationFlow,
  CLOUD_DEBUG_SEED_AGENTS,
} from "../../lib/cloud-orchestration-debug";
import { useAgentStore } from "../../stores/agents";
import { useWorkspaceStore } from "../../stores/workspaces";
import { useUIStore } from "../../stores/ui";
import { CloudDebugStatusChip } from "./cloud-debug-status-chip";
import { CloudDebugAgentsTable } from "./cloud-debug-agents-table";
import { CloudDebugBurstPanel } from "./cloud-debug-burst-panel";
import { isCloudAgent } from "../../lib/runtime-router";

type DebugSection = "overview" | "agents" | "burst" | "runtime" | "flow";

const FLOW_STEP_KEYS = [
  "cloud-config",
  "cloud-auth",
  "control-plane",
  "select-agent",
  "activate-runtime",
  "cloud-ws",
  "engine-proxy",
] as const;

export function CloudOrchestrationDebugView() {
  const { t } = useTranslation("shell");
  const snapshot = useCloudDebugSnapshot();
  const currentAgent = useAgentStore((s) => s.current);
  const createAgent = useAgentStore((s) => s.create);
  const loadAgents = useAgentStore((s) => s.loadAgents);
  const setCurrent = useAgentStore((s) => s.setCurrent);
  const workspace = useWorkspaceStore((s) => s.current);
  const addToast = useUIStore((s) => s.addToast);
  const [section, setSection] = useState<DebugSection>("overview");
  const [seeding, setSeeding] = useState(false);

  const selectedRow = snapshot.agents.find((a) => a.selected) ?? null;

  const flowSteps = useMemo(
    () =>
      buildOrchestrationFlow({
        cloudConfigured: snapshot.cloudConfigured,
        cloudAuthReady: snapshot.cloudAuthReady,
        controlPlaneOk: snapshot.controlPlaneOk,
        controlPlaneLatencyMs: snapshot.controlPlaneLatencyMs,
        currentAgent,
        wsConnected: selectedRow?.wsConnected ?? false,
        engineProxyOk: selectedRow?.engineOk ?? false,
        engineProxyLatencyMs: selectedRow?.engineLatencyMs,
      }),
    [
      snapshot.cloudConfigured,
      snapshot.cloudAuthReady,
      snapshot.controlPlaneOk,
      snapshot.controlPlaneLatencyMs,
      currentAgent,
      selectedRow?.wsConnected,
      selectedRow?.engineOk,
      selectedRow?.engineLatencyMs,
    ],
  );

  const navItems = [
    { id: "overview" as const, label: t("cloudDebug.nav.overview"), icon: Server },
    { id: "agents" as const, label: t("cloudDebug.nav.agents"), icon: Bot },
    { id: "burst" as const, label: t("cloudDebug.nav.burst"), icon: LayoutGrid },
    { id: "runtime" as const, label: t("cloudDebug.nav.runtime"), icon: Activity },
    { id: "flow" as const, label: t("cloudDebug.nav.flow"), icon: Workflow },
  ];

  async function handleSeedFour() {
    if (!workspace) return;
    setSeeding(true);
    try {
      for (const seed of CLOUD_DEBUG_SEED_AGENTS) {
        const exists = useAgentStore
          .getState()
          .agents.some((a) => isCloudAgent(a) && a.configId === seed.configId);
        if (exists) continue;
        await createAgent(
          workspace.id,
          seed.name,
          seed.configId,
          undefined,
          undefined,
          undefined,
          undefined,
          undefined,
          "cloud_24_7",
          "anthropic",
          "claude-sonnet-4-6",
          false,
        );
      }
      await loadAgents(workspace.id, { silent: true });
      addToast({ title: t("cloudDebug.seedDone"), variant: "success" });
    } catch (err) {
      addToast({
        title: t("cloudDebug.seedFailed"),
        description: err instanceof Error ? err.message : String(err),
        variant: "error",
      });
    } finally {
      setSeeding(false);
    }
  }

  return (
    <div className="flex h-full min-h-0">
      <SidebarSectionNav
        ariaLabel={t("cloudDebug.navAria")}
        items={navItems}
        active={section}
        onSelect={setSection}
        footer={
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="w-full"
            disabled={snapshot.loading || seeding || !workspace}
            onClick={() => void handleSeedFour()}
          >
            {seeding ? <Spinner className="size-3.5" /> : <Layers className="size-3.5" />}
            {t("cloudDebug.seedFour")}
          </Button>
        }
      />
      <div className="flex min-w-0 flex-1 flex-col overflow-hidden">
        <header className="border-b border-border px-6 py-4">
          <div>
            <h1 className="text-lg font-semibold">{t("cloudDebug.title")}</h1>
            <p className="text-sm text-muted-foreground">{t("cloudDebug.subtitle")}</p>
          </div>
        </header>

        <div className="flex-1 overflow-y-auto px-6 py-5">
          {section === "burst" ? (
            <CloudDebugBurstPanel />
          ) : snapshot.loading ? (
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <Spinner className="size-4" />
              {t("cloudDebug.loading")}
            </div>
          ) : section === "overview" ? (
            <div className="grid max-w-2xl gap-3">
              <CloudDebugStatusChip
                state={snapshot.cloudConfigured ? "ok" : "error"}
                label={t("cloudDebug.overview.configured")}
              />
              <CloudDebugStatusChip
                state={snapshot.cloudAuthReady ? "ok" : "error"}
                label={t("cloudDebug.overview.auth")}
              />
              <CloudDebugStatusChip
                state={snapshot.controlPlaneOk ? "ok" : "error"}
                label={t("cloudDebug.overview.controlPlane")}
                detail={
                  snapshot.controlPlaneLatencyMs != null
                    ? `${snapshot.controlPlaneLatencyMs}ms`
                    : undefined
                }
              />
              {snapshot.orgId ? (
                <CloudDebugStatusChip
                  state="ok"
                  label={t("cloudDebug.overview.org")}
                  detail={`${snapshot.orgId} (${snapshot.orgRole ?? "?"})`}
                />
              ) : null}
              {snapshot.maxCloudAgents != null ? (
                <CloudDebugStatusChip
                  state="ok"
                  label={t("cloudDebug.overview.quota")}
                  detail={t("cloudDebug.overview.quotaDetail", {
                    count: snapshot.maxCloudAgents,
                  })}
                />
              ) : null}
            </div>
          ) : section === "agents" ? (
            <CloudDebugAgentsTable
              rows={snapshot.agents}
              onSelect={(agentId) => {
                const agent = useAgentStore.getState().agents.find((a) => a.id === agentId);
                if (agent) setCurrent(agent);
              }}
            />
          ) : section === "runtime" ? (
            <div className="grid max-w-2xl gap-3">
              <CloudDebugStatusChip
                state={selectedRow ? "ok" : "idle"}
                label={t("cloudDebug.runtime.current")}
                detail={selectedRow?.name}
              />
              <CloudDebugStatusChip
                state="ok"
                label={t("cloudDebug.runtime.wsSlots")}
                detail={t("cloudDebug.runtime.wsSlotsDetail", {
                  used: snapshot.wsSlots.connectedAgentIds.length,
                  max: snapshot.wsSlots.max,
                })}
              />
              {snapshot.wsSlots.connectedAgentIds.length > 0 ? (
                <CloudDebugStatusChip
                  state="ok"
                  label={t("cloudDebug.runtime.wsConnected")}
                  detail={snapshot.wsSlots.connectedAgentIds.join(", ")}
                />
              ) : (
                <CloudDebugStatusChip
                  state="idle"
                  label={t("cloudDebug.runtime.wsConnected")}
                  detail={t("cloudDebug.runtime.wsNone")}
                />
              )}
            </div>
          ) : section === "flow" ? (
            <div className="grid max-w-2xl gap-2">
              {FLOW_STEP_KEYS.map((key, index) => {
                const step = flowSteps[index];
                if (!step) return null;
                return (
                  <CloudDebugStatusChip
                    key={key}
                    state={step.state}
                    label={t(`cloudDebug.flow.${key}`)}
                    detail={step.detail}
                  />
                );
              })}
            </div>
          ) : null}
        </div>
      </div>
    </div>
  );
}
