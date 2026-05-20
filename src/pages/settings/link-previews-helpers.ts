/**
 * Pure helpers for the Settings → Link previews section.
 *
 * Splitting these out of `link-previews-section.tsx` keeps that file a
 * pure component module so React Fast Refresh works cleanly during dev,
 * and makes the parsing / clamp logic directly unit-testable without
 * spinning up the full section render tree.
 */

/**
 * Parses a newline-separated blocklist editor value into an OgImageSettings
 * `blockedHosts` array. Trims whitespace, drops empty lines and `#` comment
 * lines, de-duplicates, and lowercases host strings so storage stays
 * canonical regardless of how the user types them.
 */
export function parseBlocklist(value: string): string[] {
  const seen = new Set<string>()
  const out: string[] = []
  for (const raw of value.split(/\r?\n/)) {
    const trimmed = raw.trim().toLowerCase()
    if (!trimmed || trimmed.startsWith('#') || seen.has(trimmed)) continue
    seen.add(trimmed)
    out.push(trimmed)
  }
  return out
}

/** Clamps `raw` into [min, max]; returns `fallback` for NaN/empty. */
export function clampNumber(
  raw: number | string,
  min: number,
  max: number,
  fallback: number,
): number {
  const numeric = typeof raw === 'number' ? raw : Number.parseInt(raw, 10)
  if (!Number.isFinite(numeric)) return fallback
  if (numeric < min) return min
  if (numeric > max) return max
  return Math.trunc(numeric)
}
