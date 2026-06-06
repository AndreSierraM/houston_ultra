import { useTranslation } from "react-i18next";
import { Spinner, cn } from "@houston-ai/core";
import type { FeedItem } from "@houston-ai/chat";
import { feedPreviewLines } from "../../lib/cloud-debug-burst";
import type { BurstAgentSlot } from "../../lib/cloud-debug-burst";
import { useFeedStore } from "../../stores/feeds";
import {
  isActiveSessionStatus,
  useSessionStatus,
} from "../../stores/session-status";

interface CloudDebugAgentChatCardProps {
  slot: BurstAgentSlot;
}

const EMPTY_FEED_ITEMS: FeedItem[] = [];

function roleForLine(line: string, items: FeedItem[]): "user" | "assistant" | "meta" {
  const last = items[items.length - 1];
  if (line.startsWith("[tool") || line.startsWith("[error") || line.startsWith("[files")) {
    return "meta";
  }
  if (line.startsWith("[skill]")) {
    return "user";
  }
  if (last?.feed_type === "user_message") {
    const text = last.data.trim();
    if (line === text || line.startsWith("[skill]")) {
      return "user";
    }
  }
  return "assistant";
}

export function CloudDebugAgentChatCard({ slot }: CloudDebugAgentChatCardProps) {
  const { t } = useTranslation("shell");
  const agentPath = slot.agentPath ?? "";
  const items = useFeedStore((s) =>
    agentPath
      ? (s.items[agentPath]?.[slot.sessionKey] ?? EMPTY_FEED_ITEMS)
      : EMPTY_FEED_ITEMS,
  );
  const status = useSessionStatus(agentPath, slot.sessionKey);
  const active = isActiveSessionStatus(status);
  const lines = feedPreviewLines(items);

  return (
    <article
      className={cn(
        "flex min-h-[220px] flex-col rounded-lg border border-border bg-background shadow-sm",
        slot.phase === "error" && "border-destructive/40",
      )}
    >
      <header className="flex items-center justify-between gap-2 border-b border-border px-3 py-2">
        <div className="min-w-0">
          <p className="truncate text-sm font-medium">{slot.name}</p>
          <p className="truncate text-xs text-muted-foreground">
            {slot.agentId ?? t("cloudDebug.burst.pendingId")}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-1.5 text-xs text-muted-foreground">
          {active ? <Spinner className="size-3" /> : null}
          <span>{t(`cloudDebug.burst.phase.${slot.phase}`)}</span>
        </div>
      </header>

      <div className="flex-1 space-y-1.5 overflow-y-auto px-3 py-2 font-mono text-[11px] leading-relaxed">
        {slot.error ? (
          <p className="text-destructive">{slot.error}</p>
        ) : lines.length === 0 ? (
          <p className="text-muted-foreground">{t("cloudDebug.burst.emptyChat")}</p>
        ) : (
          lines.map((line, i) => {
            const role = roleForLine(line, items);
            return (
              <p
                key={`${slot.sessionKey}-${i}`}
                className={cn(
                  "whitespace-pre-wrap break-words",
                  role === "user" && "text-foreground",
                  role === "assistant" && "text-muted-foreground",
                  role === "meta" && "text-muted-foreground/80 italic",
                )}
              >
                {line}
              </p>
            );
          })
        )}
      </div>

      {status ? (
        <footer className="border-t border-border px-3 py-1.5 text-[10px] text-muted-foreground">
          {t("cloudDebug.burst.sessionStatus", { status })}
        </footer>
      ) : null}
    </article>
  );
}
