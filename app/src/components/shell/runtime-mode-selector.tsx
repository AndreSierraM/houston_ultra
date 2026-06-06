import { useTranslation } from "react-i18next";
import { cn } from "@houston-ai/core";
import { getCloudBaseUrl, hasCloudToken, isCloudConfigured } from "../../lib/cloud-client";
import type { AgentRuntimeMode } from "../../lib/cloud-client";
import { useSession } from "../../hooks/use-session";
import { isAuthConfigured } from "../../lib/supabase";

interface RuntimeModeSelectorProps {
  value: AgentRuntimeMode;
  onChange: (mode: AgentRuntimeMode) => void;
}

export function RuntimeModeSelector({ value, onChange }: RuntimeModeSelectorProps) {
  const { t } = useTranslation("shell");
  const { data: session } = useSession();
  const signedIn = Boolean(session);
  const authRequired = isAuthConfigured();

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
        <p className="text-center text-xs text-muted-foreground">
          {t("runtimeMode.cloudServer", { server: getCloudBaseUrl() })}
        </p>
      )}
    </div>
  );
}
