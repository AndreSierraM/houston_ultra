import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Button,
  Dialog,
  DialogDescription,
  DialogTitle,
} from "@houston-ai/core";
import type { ProviderInfo } from "../../lib/providers";
import { tauriProvider } from "../../lib/tauri";
import { useUIStore } from "../../stores/ui";
import { ApiKeyAdvancedSection } from "./api-key-advanced-section";
import { ConnectDialogShell } from "./connect-dialog-layout";

/**
 * Connect dialog for Gemini.
 *
 * Primary: OAuth via `tauriProvider.launchLogin("gemini")` (gemini-cli ACP flow).
 * Advanced: API key via `ApiKeyForm` → `saveProviderApiKey`.
 */

interface Props {
  provider: ProviderInfo | null;
  onOpenChange: (open: boolean) => void;
  onSaved: (providerId: string) => void;
  onLoginStarted: (providerId: string) => void;
}

export function GeminiConnectDialog({
  provider,
  onOpenChange,
  onSaved,
  onLoginStarted,
}: Props) {
  const { t } = useTranslation("providers");
  const addToast = useUIStore((s) => s.addToast);
  const [apiKeyExpanded, setApiKeyExpanded] = useState(false);
  const [signingIn, setSigningIn] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (provider) {
      setApiKeyExpanded(false);
      setSigningIn(false);
      setError(null);
    }
  }, [provider]);

  if (!provider || provider.id !== "gemini") return null;

  const handleSignInWithGoogle = async () => {
    setError(null);
    setSigningIn(true);
    try {
      await tauriProvider.launchLogin(provider.id);
      onLoginStarted(provider.id);
      onOpenChange(false);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg);
      addToast({
        title: t("geminiConnect.signInFailed", { name: provider.name }),
        description: msg,
        variant: "error",
      });
      setSigningIn(false);
    }
  };

  return (
    <Dialog
      open={provider !== null}
      onOpenChange={(open) => {
        if (!open) onOpenChange(false);
      }}
    >
      <ConnectDialogShell
        header={
          <>
            <DialogTitle>{t("geminiConnect.title", { name: provider.name })}</DialogTitle>
            <DialogDescription>
              {t("geminiConnect.description", { name: provider.name })}
            </DialogDescription>
          </>
        }
        footer={
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
            {t("geminiConnect.cancel")}
          </Button>
        }
      >
        <div className="space-y-4">
          <Button
            type="button"
            size="lg"
            className="w-full justify-center gap-2"
            onClick={handleSignInWithGoogle}
            disabled={signingIn}
          >
            {signingIn
              ? t("geminiConnect.signingIn")
              : t("geminiConnect.signInWithGoogle")}
          </Button>
          <p className="text-center text-[12px] text-muted-foreground">
            {t("geminiConnect.signInRecommended")}
          </p>
          {error ? (
            <p className="text-center text-[12px] text-destructive" role="alert">
              {error}
            </p>
          ) : null}
          <ApiKeyAdvancedSection
            provider={provider}
            expanded={apiKeyExpanded}
            onExpandedChange={setApiKeyExpanded}
            onSaved={() => {
              onSaved(provider.id);
              onOpenChange(false);
            }}
          />
        </div>
      </ConnectDialogShell>
    </Dialog>
  );
}
