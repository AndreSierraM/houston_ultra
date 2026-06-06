import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import { Loader2 } from "lucide-react";
import { Button } from "@houston-ai/core";
import { useConnections, useResetConnections } from "../../hooks/queries";
import { agentForEngine } from "../../lib/engine-for-agent";
import { syncComposioCredentialsToCloudAgentSafe } from "../../lib/cloud-agent-create";
import { isCloudAgent } from "../../lib/runtime-router";
import { useUIStore } from "../../stores/ui";

interface CloudComposioSyncBannerProps {
  agentPath: string;
}

/** Offer to copy local Composio session into a cloud agent (same as create-time sync). */
export function CloudComposioSyncBanner({ agentPath }: CloudComposioSyncBannerProps) {
  const { t } = useTranslation("integrations");
  const addToast = useUIStore((s) => s.addToast);
  const agent = agentForEngine(agentPath);
  const { data: cloudStatus } = useConnections(agentPath);
  const { data: localStatus } = useConnections();
  const resetCloud = useResetConnections(agentPath);
  const [pending, setPending] = useState(false);

  const handleSync = useCallback(async () => {
    if (!agent) return;
    setPending(true);
    try {
      const result = await syncComposioCredentialsToCloudAgentSafe(agent);
      if (result.ok) {
        await resetCloud();
        addToast({ title: t("cloudSync.success"), variant: "success" });
      } else if (result.reason === "no_local_credentials") {
        addToast({
          title: t("cloudSync.needsLocalTitle"),
          description: t("cloudSync.needsLocalDescription"),
          variant: "error",
        });
      } else {
        addToast({
          title: t("cloudSync.failed"),
          description: result.message,
          variant: "error",
        });
      }
    } finally {
      setPending(false);
    }
  }, [addToast, agent, resetCloud, t]);

  if (!agent || !isCloudAgent(agent)) {
    return null;
  }
  if (cloudStatus?.status === "ok") {
    return null;
  }
  if (localStatus?.status !== "ok") {
    return null;
  }

  return (
    <div className="mb-6 rounded-xl border border-border bg-secondary px-4 py-3 flex flex-col sm:flex-row sm:items-center gap-3">
      <div className="flex-1 min-w-0">
        <p className="text-sm font-medium">{t("cloudSync.bannerTitle")}</p>
        <p className="text-xs text-muted-foreground mt-0.5">{t("cloudSync.bannerDescription")}</p>
      </div>
      <Button
        type="button"
        size="sm"
        className="shrink-0 rounded-full"
        disabled={pending}
        onClick={() => void handleSync()}
      >
        {pending ? <Loader2 className="size-4 animate-spin" /> : t("cloudSync.bannerAction")}
      </Button>
    </div>
  );
}
