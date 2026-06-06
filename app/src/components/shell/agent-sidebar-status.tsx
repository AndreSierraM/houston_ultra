import type { CSSProperties } from "react";
import { Cloud } from "lucide-react";
import { Badge, HoustonAvatar, cn, resolveAgentColor } from "@houston-ai/core";

interface AgentSidebarIconProps {
  color?: string;
  running: boolean;
  runningLabel: string;
  isCloud?: boolean;
  cloudLabel?: string;
}

export function AgentSidebarIcon({
  color,
  running,
  runningLabel,
  isCloud = false,
  cloudLabel,
}: AgentSidebarIconProps) {
  const avatar = (
    <HoustonAvatar color={resolveAgentColor(color)} diameter={20} />
  );

  const base = running ? (
    <span
      className={cn(
        "size-6 shrink-0 rounded-full flex items-center justify-center",
        "card-running-glow",
      )}
      style={{ "--glow-bg": "var(--color-sidebar)" } as CSSProperties}
      title={runningLabel}
    >
      {avatar}
    </span>
  ) : (
    avatar
  );

  if (!isCloud) return base;

  return (
    <span className="relative inline-flex shrink-0">
      {base}
      <span
        className={cn(
          "absolute -bottom-1 -right-1 flex size-3 items-center justify-center",
          "rounded-full bg-sidebar text-foreground/70",
        )}
        title={cloudLabel}
        aria-label={cloudLabel}
      >
        <Cloud className="size-2.5" aria-hidden />
      </span>
    </span>
  );
}

interface NeedsYouChipProps {
  count: number;
  label: string;
}

export function NeedsYouChip({ count, label }: NeedsYouChipProps) {
  if (count <= 0) return null;

  return (
    <Badge
      variant="outline"
      aria-label={label}
      title={label}
      className="h-5 min-w-7 bg-background/90 px-2 text-[11px] font-semibold leading-none text-foreground/80"
    >
      {count > 99 ? "99+" : count}
    </Badge>
  );
}
