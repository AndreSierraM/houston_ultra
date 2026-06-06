import { cn } from "@houston-ai/core";
import type { OrchestrationStepState } from "../../lib/cloud-orchestration-debug";

const STATE_CLASS: Record<OrchestrationStepState, string> = {
  idle: "bg-muted text-muted-foreground",
  active: "bg-amber-500/15 text-amber-700 dark:text-amber-400",
  ok: "bg-emerald-500/15 text-emerald-700 dark:text-emerald-400",
  error: "bg-destructive/15 text-destructive",
  skip: "bg-muted/60 text-muted-foreground",
};

interface CloudDebugStatusChipProps {
  state: OrchestrationStepState;
  label: string;
  detail?: string;
}

export function CloudDebugStatusChip({ state, label, detail }: CloudDebugStatusChipProps) {
  return (
    <div className="flex items-start justify-between gap-3 rounded-lg border border-border/60 bg-card px-3 py-2.5">
      <div className="min-w-0">
        <p className="text-sm font-medium text-foreground">{label}</p>
        {detail ? (
          <p className="mt-0.5 truncate text-xs text-muted-foreground">{detail}</p>
        ) : null}
      </div>
      <span
        className={cn(
          "shrink-0 rounded-full px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide",
          STATE_CLASS[state],
        )}
      >
        {state}
      </span>
    </div>
  );
}
