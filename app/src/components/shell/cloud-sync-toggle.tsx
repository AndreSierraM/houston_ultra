import { Switch, cn } from "@houston-ai/core";

export function CloudSyncToggle({
  checked,
  onCheckedChange,
  disabled,
  title,
  description,
  ariaLabel,
}: {
  checked: boolean;
  onCheckedChange: (value: boolean) => void;
  disabled?: boolean;
  title: string;
  description: string;
  ariaLabel: string;
}) {
  return (
    <label
      className={cn(
        "flex items-start gap-3 rounded-xl border border-border px-3 py-2.5",
        disabled ? "opacity-60 cursor-not-allowed" : "cursor-pointer",
      )}
    >
      <Switch
        checked={checked}
        onCheckedChange={onCheckedChange}
        disabled={disabled}
        aria-label={ariaLabel}
      />
      <span className="min-w-0 text-left">
        <span className="block text-sm font-medium">{title}</span>
        <span className="block text-xs text-muted-foreground mt-0.5">{description}</span>
      </span>
    </label>
  );
}
