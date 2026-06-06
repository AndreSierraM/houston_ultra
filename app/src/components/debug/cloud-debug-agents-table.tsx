import { useTranslation } from "react-i18next";
import { Button } from "@houston-ai/core";
import type { CloudAgentDebugRow } from "../../hooks/use-cloud-debug-snapshot";
import { useAgentStore } from "../../stores/agents";

interface CloudDebugAgentsTableProps {
  rows: CloudAgentDebugRow[];
  onSelect: (agentId: string) => void;
}

export function CloudDebugAgentsTable({ rows, onSelect }: CloudDebugAgentsTableProps) {
  const { t } = useTranslation("shell");

  return (
    <div className="overflow-x-auto rounded-lg border border-border">
      <table className="w-full min-w-[720px] text-left text-sm">
        <thead className="bg-muted/40 text-xs uppercase tracking-wide text-muted-foreground">
          <tr>
            <th className="px-3 py-2">{t("cloudDebug.agents.name")}</th>
            <th className="px-3 py-2">{t("cloudDebug.agents.front")}</th>
            <th className="px-3 py-2">{t("cloudDebug.agents.back")}</th>
            <th className="px-3 py-2">{t("cloudDebug.agents.actions")}</th>
          </tr>
        </thead>
        <tbody>
          {rows.length === 0 ? (
            <tr>
              <td colSpan={4} className="px-3 py-6 text-center text-muted-foreground">
                {t("cloudDebug.agents.empty")}
              </td>
            </tr>
          ) : (
            rows.map((row) => (
              <tr key={row.agentId} className="border-t border-border/60">
                <td className="px-3 py-2.5">
                  <p className="font-medium">{row.name}</p>
                  <p className="text-xs text-muted-foreground">{row.configId}</p>
                </td>
                <td className="px-3 py-2.5 text-xs text-muted-foreground">
                  <p>
                    {row.selected
                      ? t("cloudDebug.agents.selected")
                      : t("cloudDebug.agents.notSelected")}
                  </p>
                  <p>
                    {row.wsConnected
                      ? t("cloudDebug.agents.wsOn")
                      : t("cloudDebug.agents.wsOff")}
                  </p>
                </td>
                <td className="px-3 py-2.5 text-xs text-muted-foreground">
                  <p>
                    {row.provisionStatus ??
                      row.provisionError ??
                      t("cloudDebug.agents.unknown")}
                  </p>
                  <p>
                    {row.engineOk === true
                      ? t("cloudDebug.agents.engineOk", {
                          ms: row.engineLatencyMs ?? 0,
                        })
                      : row.engineOk === false
                        ? row.engineError ?? t("cloudDebug.agents.engineDown")
                        : t("cloudDebug.agents.unknown")}
                  </p>
                </td>
                <td className="px-3 py-2.5">
                  <Button
                    type="button"
                    size="sm"
                    variant="outline"
                    onClick={() => {
                      const agent = useAgentStore
                        .getState()
                        .agents.find((a) => a.id === row.agentId);
                      if (agent) onSelect(agent.id);
                    }}
                  >
                    {t("cloudDebug.agents.select")}
                  </Button>
                </td>
              </tr>
            ))
          )}
        </tbody>
      </table>
    </div>
  );
}
