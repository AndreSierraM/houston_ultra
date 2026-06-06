import { useTranslation } from "react-i18next";
import { Button, Dialog, DialogDescription, DialogTitle } from "@houston-ai/core";
import type { ProviderInfo } from "../../lib/providers";
import { isApiKeyOnlyProvider } from "../../lib/provider-api-key";
import { ApiKeyForm } from "./api-key-form";
import { ConnectDialogShell } from "./connect-dialog-layout";

interface Props {
  provider: ProviderInfo | null;
  onOpenChange: (open: boolean) => void;
  onSaved: (providerId: string) => void;
}

export function ApiKeyConnectDialog({ provider, onOpenChange, onSaved }: Props) {
  const { t } = useTranslation("providers");

  if (!provider || !isApiKeyOnlyProvider(provider)) return null;

  const descriptionKey =
    provider.id === "openrouter"
      ? "openrouterConnect.description"
      : "apiKeyConnect.description";

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
            <DialogTitle>{t("apiKeyConnect.title", { name: provider.name })}</DialogTitle>
            <DialogDescription>{t(descriptionKey, { name: provider.name })}</DialogDescription>
          </>
        }
        footer={
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
            {t("apiKeyConnect.cancel")}
          </Button>
        }
      >
        <ApiKeyForm
          providerName={provider.name}
          providerId={provider.id}
          apiKeyConsoleUrl={provider.apiKeyConsoleUrl ?? ""}
          onSaved={() => {
            onSaved(provider.id);
            onOpenChange(false);
          }}
        />
      </ConnectDialogShell>
    </Dialog>
  );
}
