import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Badge, cn } from "@houston-ai/core";
import {
  getCloudBaseUrl,
  hasCloudToken,
  isCloudConfigured,
  pingCloudServer,
} from "../../lib/cloud-client";
import type { AgentRuntimeMode } from "../../lib/cloud-client";
import { useSession } from "../../hooks/use-session";
import { isAuthConfigured } from "../../lib/supabase";
import { useConnections, useConnectedToolkits } from "../../hooks/queries";
import { CloudSyncToggle } from "./cloud-sync-toggle";
import { ComposioCarryPreview } from "./composio-carry-preview";

interface RuntimeModeSelectorProps {
  value: AgentRuntimeMode;
  onChange: (mode: AgentRuntimeMode) => void;
  syncConnection?: boolean;
  onSyncConnectionChange?: (value: boolean) => void;
  syncComposioConnection?: boolean;
  onSyncComposioConnectionChange?: (value: boolean) => void;
}

export function RuntimeModeSelector({
  value,
  onChange,
  syncConnection = true,
  onSyncConnectionChange,
  syncComposioConnection = true,
  onSyncComposioConnectionChange,
}: RuntimeModeSelectorProps) {
  const { t } = useTranslation("shell");
  const { data: session } = useSession();
  const signedIn = Boolean(session);
  const authRequired = isAuthConfigured();
  const [pingOk, setPingOk] = useState<boolean | null>(null);
  const [latencyMs, setLatencyMs] = useState<number | undefined>();
  const { data: localComposio } = useConnections();
  const localComposioOk = localComposio?.status === "ok";
  const { data: localToolkits } = useConnectedToolkits(localComposioOk);

  useEffect(() => {
    if (value !== "cloud_24_7" || !isCloudConfigured()) {
      return;
    }
    let cancelled = false;
    setPingOk(null);
    setLatencyMs(undefined);
    pingCloudServer().then((result) => {
      if (cancelled) return;
      setPingOk(result.ok);
      setLatencyMs(result.latencyMs);
    });
    return () => {
      cancelled = true;
    };
  }, [value]);

  const composioCount = localToolkits?.length ?? 0;
  const composioSyncDescription = localComposioOk
    ? composioCount > 0
      ? t("runtimeMode.syncComposioDescriptionWithApps", { count: composioCount })
      : t("runtimeMode.syncComposioDescriptionSignedIn")
    : t("runtimeMode.syncComposioUnavailable");

  const showCarryPreview =
    value === "cloud_24_7" &&
    syncComposioConnection &&
    localComposioOk &&
    composioCount > 0 &&
    localToolkits;

  return (
    <div className="space-y-2">
      <p className="text-sm font-medium text-center">{t("runtimeMode.label")}</p>
      <div className="grid grid-cols-2 gap-2">
        <button
          type="button"
          onClick={() => onChange("local")}
          className={cn(
            "rounded-xl border px-3 py-2.5 text-left transition-colors",
            value === "local"
              ? "border-foreground/25 bg-secondary"
              : "border-border hover:border-foreground/15 hover:bg-accent/50",
          )}
        >
          <div className="text-sm font-medium">{t("runtimeMode.localTitle")}</div>
          <div className="text-xs text-muted-foreground mt-0.5">
            {t("runtimeMode.localDescription")}
          </div>
        </button>
        <button
          type="button"
          onClick={() => onChange("cloud_24_7")}
          className={cn(
            "rounded-xl border px-3 py-2.5 text-left transition-colors",
            value === "cloud_24_7"
              ? "border-foreground/25 bg-secondary"
              : "border-border hover:border-foreground/15 hover:bg-accent/50",
          )}
        >
          <div className="text-sm font-medium">{t("runtimeMode.cloudTitle")}</div>
          <div className="text-xs text-muted-foreground mt-0.5">
            {t("runtimeMode.cloudDescription")}
          </div>
        </button>
      </div>
      {value === "cloud_24_7" && onSyncConnectionChange && (
        <CloudSyncToggle
          checked={syncConnection}
          onCheckedChange={onSyncConnectionChange}
          title={t("runtimeMode.syncConnectionTitle")}
          description={t("runtimeMode.syncConnectionDescription")}
          ariaLabel={t("runtimeMode.syncConnectionTitle")}
        />
      )}
      {value === "cloud_24_7" && onSyncComposioConnectionChange && (
        <CloudSyncToggle
          checked={syncComposioConnection}
          onCheckedChange={onSyncComposioConnectionChange}
          disabled={!localComposioOk}
          title={t("runtimeMode.syncComposioTitle")}
          description={composioSyncDescription}
          ariaLabel={t("runtimeMode.syncComposioTitle")}
        />
      )}
      {showCarryPreview && (
        <ComposioCarryPreview toolkits={localToolkits} context="create" />
      )}
      {value === "cloud_24_7" && !hasCloudToken() && !signedIn && (
        <p className="text-center text-xs text-muted-foreground">
          {t(
            authRequired
              ? "runtimeMode.cloudRequiresSignIn"
              : "runtimeMode.cloudRequiresToken",
          )}
        </p>
      )}
      {value === "cloud_24_7" && isCloudConfigured() && (
        <div className="space-y-1 text-center">
          <p className="text-xs text-muted-foreground">
            {t("runtimeMode.cloudServer", { server: getCloudBaseUrl() })}
          </p>
          <div className="flex items-center justify-center gap-2">
            <Badge
              variant={
                pingOk === true
                  ? "secondary"
                  : pingOk === false
                    ? "destructive"
                    : "outline"
              }
            >
              {pingOk === null
                ? t("runtimeMode.cloudPingChecking")
                : pingOk
                  ? t("runtimeMode.cloudPingOk")
                  : t("runtimeMode.cloudPingFail")}
            </Badge>
            {pingOk === true && latencyMs !== undefined && (
              <span className="text-xs text-muted-foreground tabular-nums">
                {latencyMs} ms
              </span>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
