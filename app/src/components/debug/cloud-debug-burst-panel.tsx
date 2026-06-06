import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Octagon, Play, Square } from "lucide-react";
import { Button, Input, Spinner, cn } from "@houston-ai/core";
import {
  CLOUD_DEBUG_BURST_CONFIG_IDS,
  CLOUD_DEBUG_BURST_DEFAULT,
  DEFAULT_CLOUD_DEBUG_BURST_SCENARIO,
  burstSkillUserText,
  listBurstAgentsForCleanup,
} from "../../lib/cloud-debug-burst";
import { useCloudBurstRun } from "../../hooks/use-cloud-burst-run";
import { isCloudAgent } from "../../lib/runtime-router";
import { getDefaultModel } from "../../lib/providers";
import { tauriProvider } from "../../lib/tauri";
import { useAgentStore } from "../../stores/agents";
import { useWorkspaceStore } from "../../stores/workspaces";
import { useUIStore } from "../../stores/ui";
import { ChatModelSelector } from "../chat-model-selector";
import { CloudDebugAgentChatCard } from "./cloud-debug-agent-chat-card";

export function CloudDebugBurstPanel() {
  const { t } = useTranslation("shell");
  const workspace = useWorkspaceStore((s) => s.current);
  const addToast = useUIStore((s) => s.addToast);
  const agents = useAgentStore((s) => s.agents);
  const { running, slots, summary, runBurst, stopBurst, reset } = useCloudBurstRun();

  const [count, setCount] = useState(CLOUD_DEBUG_BURST_DEFAULT);
  const [configId, setConfigId] = useState<string>(
    CLOUD_DEBUG_BURST_CONFIG_IDS[0] ?? "support",
  );
  const [greeting, setGreeting] = useState(DEFAULT_CLOUD_DEBUG_BURST_SCENARIO.greeting);
  const [skillUserText, setSkillUserText] = useState(
    burstSkillUserText(CLOUD_DEBUG_BURST_CONFIG_IDS[0] ?? "support"),
  );
  const [provider, setProvider] = useState("anthropic");
  const [model, setModel] = useState(getDefaultModel("anthropic"));
  const [stopping, setStopping] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void tauriProvider.getLastUsed().then(({ provider: p, model: m }) => {
      if (cancelled) return;
      const nextProvider = p ?? "anthropic";
      setProvider(nextProvider);
      setModel(m ?? getDefaultModel(nextProvider));
    });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    setSkillUserText(burstSkillUserText(configId));
  }, [configId]);

  const burstCleanupCount = listBurstAgentsForCleanup(
    agents.filter(isCloudAgent),
    configId,
  ).length;

  async function handleRun() {
    if (!workspace) return;
    if (summary.isActive) return;
    try {
      const result = await runBurst({
        workspaceId: workspace.id,
        count,
        configId,
        scenario: { greeting, skillUserText },
        provider,
        model,
      });

      if (result.kind === "already_running") {
        addToast({
          title: t("cloudDebug.burst.alreadyRunning"),
          description: t("cloudDebug.burst.runSummary", {
            running: result.summary.running,
            done: result.summary.done,
            error: result.summary.error,
            total: result.summary.total,
          }),
          variant: "info",
        });
        return;
      }

      if (result.kind === "stopped") {
        addToast({
          title: t("cloudDebug.burst.stopped"),
          description: t("cloudDebug.burst.runSummary", {
            running: result.summary.running,
            done: result.summary.done,
            error: result.summary.error,
            total: result.summary.total,
          }),
          variant: "info",
        });
        return;
      }

      if (result.kind === "partial") {
        addToast({
          title: t("cloudDebug.burst.partialDone"),
          description: t("cloudDebug.burst.runSummary", {
            running: result.summary.running,
            done: result.summary.done,
            error: result.summary.error,
            total: result.summary.total,
          }),
          variant: "info",
        });
        return;
      }

      addToast({ title: t("cloudDebug.burst.done"), variant: "success" });
    } catch (err) {
      addToast({
        title: t("cloudDebug.burst.failed"),
        description: err instanceof Error ? err.message : String(err),
        variant: "error",
      });
    }
  }

  async function handleStop() {
    setStopping(true);
    try {
      await stopBurst();
    } catch (err) {
      addToast({
        title: t("cloudDebug.burst.failed"),
        description: err instanceof Error ? err.message : String(err),
        variant: "error",
      });
    } finally {
      setStopping(false);
    }
  }

  const showSummary = summary.total > 0;
  const canStop = running || summary.isActive;

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4">
      <section className="grid max-w-3xl gap-3 rounded-lg border border-border p-4">
        <p className="text-sm text-muted-foreground">{t("cloudDebug.burst.hint")}</p>
        {showSummary ? (
          <div
            className={cn(
              "rounded-md border px-3 py-2 text-xs",
              summary.isActive
                ? "border-primary/30 bg-primary/5 text-foreground"
                : "border-border bg-muted/30 text-muted-foreground",
            )}
          >
            <p className="font-medium">
              {summary.isActive
                ? t("cloudDebug.burst.statusRunning")
                : t("cloudDebug.burst.statusIdle")}
            </p>
            <p>
              {t("cloudDebug.burst.runSummary", {
                running: summary.running,
                done: summary.done,
                error: summary.error,
                total: summary.total,
              })}
            </p>
          </div>
        ) : null}
        {burstCleanupCount > 0 && !summary.isActive ? (
          <p className="text-xs text-muted-foreground">
            {t("cloudDebug.burst.cleanupHint", { count: burstCleanupCount })}
          </p>
        ) : null}
        <p className="text-xs text-muted-foreground">{t("cloudDebug.burst.scenarioHint")}</p>
        <div className="grid gap-3 sm:grid-cols-2">
          <div className="space-y-1.5">
            <label htmlFor="burst-count" className="text-sm font-medium">
              {t("cloudDebug.burst.countLabel")}
            </label>
            <Input
              id="burst-count"
              type="number"
              min={1}
              value={count}
              disabled={canStop}
              onChange={(e) => setCount(Number(e.target.value))}
            />
          </div>
          <div className="space-y-1.5">
            <label htmlFor="burst-config" className="text-sm font-medium">
              {t("cloudDebug.burst.configLabel")}
            </label>
            <select
              id="burst-config"
              className="flex h-9 w-full rounded-md border border-input bg-background px-3 text-sm"
              value={configId}
              disabled={canStop}
              onChange={(e) => setConfigId(e.target.value)}
            >
              {CLOUD_DEBUG_BURST_CONFIG_IDS.map((id) => (
                <option key={id} value={id}>
                  {id}
                </option>
              ))}
            </select>
          </div>
        </div>
        <div
          className={cn(
            "space-y-1.5",
            canStop && "pointer-events-none opacity-60",
          )}
        >
          <span className="text-sm font-medium">{t("cloudDebug.burst.modelLabel")}</span>
          <ChatModelSelector
            provider={provider}
            model={model}
            onSelect={(nextProvider, nextModel) => {
              setProvider(nextProvider);
              setModel(nextModel);
            }}
          />
        </div>
        <div className="space-y-1.5">
          <label htmlFor="burst-greeting" className="text-sm font-medium">
            {t("cloudDebug.burst.greetingLabel")}
          </label>
          <textarea
            id="burst-greeting"
            className="min-h-[72px] w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            value={greeting}
            disabled={canStop}
            onChange={(e) => setGreeting(e.target.value)}
          />
        </div>
        <div className="space-y-1.5">
          <label htmlFor="burst-skill" className="text-sm font-medium">
            {t("cloudDebug.burst.skillLabel")}
          </label>
          <textarea
            id="burst-skill"
            className="min-h-[72px] w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            value={skillUserText}
            disabled={canStop}
            onChange={(e) => setSkillUserText(e.target.value)}
          />
        </div>
        <div className="flex flex-wrap gap-2">
          <Button
            type="button"
            disabled={!workspace || canStop}
            onClick={() => void handleRun()}
          >
            <Play className="size-3.5" />
            {t("cloudDebug.burst.run")}
          </Button>
          <Button
            type="button"
            variant="destructive"
            disabled={!canStop || stopping}
            onClick={() => void handleStop()}
          >
            {stopping ? <Spinner className="size-3.5" /> : <Octagon className="size-3.5" />}
            {t("cloudDebug.burst.stop")}
          </Button>
          <Button
            type="button"
            variant="outline"
            disabled={canStop || slots.length === 0}
            onClick={reset}
          >
            <Square className="size-3.5" />
            {t("cloudDebug.burst.clear")}
          </Button>
        </div>
      </section>

      {slots.length > 0 ? (
        <div className="grid min-h-0 flex-1 gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
          {slots.map((slot) => (
            <CloudDebugAgentChatCard key={slot.sessionKey} slot={slot} />
          ))}
        </div>
      ) : null}
    </div>
  );
}
