import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check } from "lucide-react";
import { useComposioApps } from "../../hooks/queries";
import { normalizeToolkitSlug } from "../../lib/composio-toolkits";

interface ComposioCarryPreviewProps {
  /** Composio toolkit slugs connected on this computer. */
  toolkits: string[];
  /** `create` = wizard / naming step. `integrations` = connect-apps step. */
  context?: "create" | "integrations";
}

function fallbackLogo(toolkit: string): string {
  return `https://www.google.com/s2/favicons?domain=${toolkit}.com&sz=128`;
}

function AppRow({
  name,
  logoUrl,
}: {
  name: string;
  logoUrl: string;
}) {
  const [imgError, setImgError] = useState(false);
  const initial = name.charAt(0).toUpperCase();

  return (
    <li className="flex items-center gap-2.5 min-w-0">
      {imgError || !logoUrl ? (
        <span className="size-7 rounded-lg bg-background flex items-center justify-center text-[10px] font-semibold text-muted-foreground shrink-0">
          {initial}
        </span>
      ) : (
        <img
          src={logoUrl}
          alt=""
          className="size-7 rounded-lg object-contain bg-background shrink-0"
          onError={() => setImgError(true)}
        />
      )}
      <span className="text-sm font-medium text-foreground truncate">{name}</span>
      <Check className="size-3.5 text-emerald-600 shrink-0 ml-auto" aria-hidden />
    </li>
  );
}

/**
 * Shows which Composio apps the user already connected locally and will not
 * need to connect again when copying to a cloud agent.
 */
export function ComposioCarryPreview({
  toolkits,
  context = "create",
}: ComposioCarryPreviewProps) {
  const { t } = useTranslation("shell");
  const { data: apiApps } = useComposioApps();

  const apps = useMemo(() => {
    const unique = [...new Set(toolkits.map(normalizeToolkitSlug))].sort();
    return unique.map((slug) => {
      const fromApi = apiApps?.find((a) => normalizeToolkitSlug(a.toolkit) === slug);
      return {
        slug,
        name: fromApi?.name ?? slug,
        logoUrl: fromApi?.logo_url || fallbackLogo(slug),
      };
    });
  }, [apiApps, toolkits]);

  if (apps.length === 0) {
    return null;
  }

  const leadKey =
    context === "integrations"
      ? "runtimeMode.carryPreviewIntegrationsLead"
      : "runtimeMode.carryPreviewLead";

  return (
    <div
      className="rounded-xl border border-emerald-200/70 bg-emerald-50/70 px-3 py-2.5 space-y-2"
      role="status"
    >
      <p className="text-xs text-emerald-950 leading-relaxed">{t(leadKey)}</p>
      <ul className="space-y-1.5" aria-label={t("runtimeMode.carryPreviewListAria")}>
        {apps.map((app) => (
          <AppRow key={app.slug} name={app.name} logoUrl={app.logoUrl} />
        ))}
      </ul>
    </div>
  );
}
