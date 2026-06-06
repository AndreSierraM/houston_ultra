export function looksLikeUrl(value: string): boolean {
  if (value.startsWith("//")) return true;
  const schemeMatch = /^([a-zA-Z][a-zA-Z0-9+\-.]*):(.+)/.exec(value);
  if (!schemeMatch) return false;
  const rest = schemeMatch[2];
  if (rest.startsWith("\\")) return false;
  if (rest.startsWith("/") && schemeMatch[1].length === 1) return false;
  return true;
}

/** Pure: true when cloud agents must not open this href as a local file. */
export function shouldBlockCloudFileOpen(href: string, isCloud: boolean): boolean {
  const trimmed = href.trim();
  if (!trimmed || !isCloud) return false;
  return !looksLikeUrl(trimmed);
}
