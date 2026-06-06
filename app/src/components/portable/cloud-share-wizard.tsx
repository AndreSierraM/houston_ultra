/**
 * Share access to a cloud_24_7 agent via the control plane.
 */
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Button,
  Dialog,
  DialogContent,
  Input,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@houston-ai/core";
import {
  listCloudAgentShares,
  revokeCloudAgentShare,
  upsertCloudAgentShare,
  type CloudAgentShare,
  type CloudShareRole,
} from "../../lib/cloud-client";
import { useUIStore } from "../../stores/ui";
import type { Agent } from "../../lib/types";

const UUID_RE =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

const ROLES: CloudShareRole[] = ["viewer", "operator", "admin"];

interface Props {
  agent: Agent;
  open: boolean;
  onClose: () => void;
}

export function CloudShareWizard({ agent, open, onClose }: Props) {
  const { t } = useTranslation("portable");
  const addToast = useUIStore((s) => s.addToast);

  const [shares, setShares] = useState<CloudAgentShare[]>([]);
  const [loading, setLoading] = useState(false);
  const [userId, setUserId] = useState("");
  const [role, setRole] = useState<CloudShareRole>("viewer");
  const [submitting, setSubmitting] = useState(false);
  const [revokingId, setRevokingId] = useState<string | null>(null);

  const loadShares = useCallback(async () => {
    setLoading(true);
    try {
      const rows = await listCloudAgentShares(agent.id);
      setShares(rows);
    } catch {
      addToast({
        variant: "error",
        title: t("cloudShare.errors.loadFailed"),
      });
      onClose();
    } finally {
      setLoading(false);
    }
  }, [agent.id, addToast, onClose, t]);

  useEffect(() => {
    if (!open) {
      setUserId("");
      setRole("viewer");
      setShares([]);
      return;
    }
    void loadShares();
  }, [open, loadShares]);

  const handleGrant = async () => {
    const trimmed = userId.trim();
    if (!UUID_RE.test(trimmed)) {
      addToast({
        variant: "error",
        title: t("cloudShare.errors.invalidUserId"),
      });
      return;
    }
    setSubmitting(true);
    try {
      const row = await upsertCloudAgentShare(agent.id, {
        userId: trimmed,
        role,
      });
      setShares((prev) => {
        const rest = prev.filter((s) => s.userId !== row.userId);
        return [...rest, row];
      });
      setUserId("");
      addToast({
        variant: "success",
        title: t("cloudShare.toasts.grantedTitle"),
        description: t("cloudShare.toasts.grantedDescription"),
      });
    } catch {
      /* cloudFetch surfaces toast */
    } finally {
      setSubmitting(false);
    }
  };

  const handleRevoke = async (targetId: string) => {
    setRevokingId(targetId);
    try {
      await revokeCloudAgentShare(agent.id, targetId);
      setShares((prev) => prev.filter((s) => s.userId !== targetId));
      addToast({
        variant: "success",
        title: t("cloudShare.toasts.revokedTitle"),
        description: t("cloudShare.toasts.revokedDescription"),
      });
    } catch {
      /* cloudFetch surfaces toast */
    } finally {
      setRevokingId(null);
    }
  };

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-[520px] flex flex-col p-0 gap-0 overflow-hidden">
        <header className="shrink-0 px-8 pt-6 pb-2">
          <p className="text-xs text-muted-foreground">
            {t("cloudShare.eyebrow", { name: agent.name })}
          </p>
        </header>

        <div className="flex-1 min-h-0 overflow-y-auto px-8 pt-2 pb-6 space-y-8">
          <header>
            <h1 className="text-[28px] font-normal leading-tight">
              {t("cloudShare.title")}
            </h1>
            <p className="mt-3 text-base text-muted-foreground">
              {t("cloudShare.body")}
            </p>
          </header>

          <section className="space-y-3">
            <h2 className="text-sm font-medium">{t("cloudShare.currentShares")}</h2>
            {loading ? (
              <p className="text-sm text-muted-foreground">
                {t("cloudShare.loading")}
              </p>
            ) : shares.length === 0 ? (
              <p className="text-sm text-muted-foreground">
                {t("cloudShare.noShares")}
              </p>
            ) : (
              <ul className="space-y-2">
                {shares.map((share) => (
                  <li
                    key={share.userId}
                    className="flex items-center justify-between gap-3 rounded-xl border border-black/5 px-4 py-3"
                  >
                    <div className="min-w-0">
                      <p className="text-sm font-mono truncate">{share.userId}</p>
                      <p className="text-xs text-muted-foreground">
                        {t(`cloudShare.roles.${share.role}`)}
                      </p>
                    </div>
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      disabled={revokingId === share.userId}
                      onClick={() => void handleRevoke(share.userId)}
                    >
                      {revokingId === share.userId
                        ? t("cloudShare.revoking")
                        : t("cloudShare.revoke")}
                    </Button>
                  </li>
                ))}
              </ul>
            )}
          </section>

          <section className="space-y-4">
            <div className="space-y-2">
              <label htmlFor="cloud-share-user-id" className="text-sm font-medium">
                {t("cloudShare.userIdLabel")}
              </label>
              <Input
                id="cloud-share-user-id"
                value={userId}
                onChange={(e) => setUserId(e.target.value)}
                placeholder={t("cloudShare.userIdPlaceholder")}
                className="rounded-xl font-mono text-sm"
                autoComplete="off"
              />
            </div>
            <div className="space-y-2">
              <label htmlFor="cloud-share-role" className="text-sm font-medium">
                {t("cloudShare.roleLabel")}
              </label>
              <Select value={role} onValueChange={(v) => setRole(v as CloudShareRole)}>
                <SelectTrigger id="cloud-share-role" className="w-full rounded-xl">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {ROLES.map((r) => (
                    <SelectItem key={r} value={r}>
                      {t(`cloudShare.roles.${r}`)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <Button
              type="button"
              className="rounded-full"
              disabled={submitting || !userId.trim()}
              onClick={() => void handleGrant()}
            >
              {submitting ? t("cloudShare.submitting") : t("cloudShare.submit")}
            </Button>
          </section>
        </div>

        <footer className="shrink-0 px-8 py-4 flex justify-end">
          <Button type="button" variant="ghost" onClick={onClose}>
            {t("cloudShare.close")}
          </Button>
        </footer>
      </DialogContent>
    </Dialog>
  );
}
