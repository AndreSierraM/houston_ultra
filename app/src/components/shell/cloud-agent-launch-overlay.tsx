import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { cn, Spinner } from "@houston-ai/core";

const CLOUD_STAGE_KEYS = [
  "cloudCreate.stagePrepare",
  "cloudCreate.stageLaunch",
  "cloudCreate.stagePod",
  "cloudCreate.stageReady",
] as const;

const STAGE_MS = 2800;

interface CloudAgentLaunchOverlayProps {
  active: boolean;
  cloud: boolean;
}

function fluentRocketUrl(): string {
  return "https://cdn.jsdelivr.net/gh/microsoft/fluentui-emoji@main/assets/Rocket/3D/rocket_3d.png";
}

export function CloudAgentLaunchOverlay({ active, cloud }: CloudAgentLaunchOverlayProps) {
  const { t } = useTranslation("shell");
  const [stage, setStage] = useState(0);

  useEffect(() => {
    if (!active) {
      setStage(0);
      return;
    }
    if (!cloud) return;
    const id = window.setInterval(() => {
      setStage((s) => (s + 1) % CLOUD_STAGE_KEYS.length);
    }, STAGE_MS);
    return () => window.clearInterval(id);
  }, [active, cloud]);

  if (!active) return null;

  return (
    <div
      className="absolute inset-0 z-50 flex flex-col items-center justify-center bg-background/95 backdrop-blur-sm px-6"
      role="status"
      aria-live="polite"
      aria-busy="true"
    >
      {cloud ? (
        <>
          <div className="relative mb-8 flex h-28 w-28 items-end justify-center">
            <span
              aria-hidden
              className="absolute bottom-0 h-8 w-24 rounded-full bg-orange-500/20 blur-xl animate-pulse"
            />
            <img
              src={fluentRocketUrl()}
              alt=""
              className={cn(
                "h-20 w-20 object-contain drop-shadow-md",
                "motion-safe:animate-[houston-rocket-launch_2.4s_ease-in-out_infinite]",
              )}
            />
          </div>
          <p className="text-base font-medium text-foreground text-center">
            {t("cloudCreate.title")}
          </p>
          <p className="mt-2 text-sm text-muted-foreground text-center min-h-[1.25rem]">
            {t(CLOUD_STAGE_KEYS[stage])}
          </p>
        </>
      ) : (
        <>
          <Spinner className="size-8 mb-4" />
          <p className="text-sm font-medium text-foreground">{t("cloudCreate.localTitle")}</p>
        </>
      )}
    </div>
  );
}
