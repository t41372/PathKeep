/**
 * Pure helpers for the content-fetch consent section (W-ENRICH-1).
 *
 * Why these are separate: the consent surface must keep its egress-affecting
 * state transitions (master toggle, per-extractor toggle, per-domain blocklist
 * parsing) testable in isolation — the rules that decide what is sent over the
 * network are exactly the ones that deserve direct unit coverage, away from the
 * React render path.
 *
 * ## Not responsible for
 * - Transport (the section calls the typed wrapper).
 * - i18n (the section owns user-facing copy).
 */

import type {
  ContentFetchDomainRule,
  ContentFetchExtractorPreference,
  ContentFetchSettings,
} from '@/lib/types'

/** Built-in extractor id for GitHub public repo metadata (mirrors the Rust const). */
export const CONTENT_FETCH_EXTRACTOR_GITHUB_REPO = 'github-repo'
/** Built-in extractor id for the deterministic generic-readable fallback (mirrors the Rust const). */
export const CONTENT_FETCH_EXTRACTOR_GENERIC_READABLE = 'generic-readable'

/** The built-in extractors the consent UI surfaces, in display order. */
export const CONTENT_FETCH_BUILTIN_EXTRACTORS = [
  CONTENT_FETCH_EXTRACTOR_GITHUB_REPO,
  CONTENT_FETCH_EXTRACTOR_GENERIC_READABLE,
] as const

/**
 * Whether one extractor is enabled in the current settings.
 *
 * Defaults to ENABLED when there is no stored preference — mirrors the backend
 * `content_extractor_enabled` rule (a newly added built-in works once the master
 * switch is on, without a config migration). Returns false when settings are
 * not yet loaded so a control never reads "on" before it has truth.
 */
export function extractorEnabled(
  settings: ContentFetchSettings | null,
  extractorId: string,
): boolean {
  if (!settings) return false
  const pref = settings.extractors.find(
    (item) => item.extractorId === extractorId,
  )
  return pref ? pref.enabled : true
}

/**
 * Returns a new settings object with the master switch flipped.
 *
 * Turning the master switch on is the consent gate; this helper only changes
 * `enabled` so the per-extractor / per-domain preferences the user already set
 * are preserved across an off→on→off cycle.
 */
export function applyContentFetchMasterToggle(
  settings: ContentFetchSettings,
  enabled: boolean,
): ContentFetchSettings {
  return { ...settings, enabled }
}

/**
 * Returns a new settings object with one extractor's preference set.
 *
 * Upserts the preference: an existing row is updated in place, an absent one is
 * appended, so the backend always receives an explicit on/off and never has to
 * guess from a missing row.
 */
export function applyContentFetchExtractorToggle(
  settings: ContentFetchSettings,
  extractorId: string,
  enabled: boolean,
): ContentFetchSettings {
  const existing = settings.extractors.some(
    (item) => item.extractorId === extractorId,
  )
  const extractors: ContentFetchExtractorPreference[] = existing
    ? settings.extractors.map((item) =>
        item.extractorId === extractorId ? { ...item, enabled } : item,
      )
    : [...settings.extractors, { extractorId, enabled }]
  return { ...settings, extractors }
}

/**
 * Serializes the per-domain blocklist (blocked rules only) to one host per line.
 *
 * Only `allowed = false` rules are shown — the MVP surface is a blocklist, and
 * the explicit-allow shape (reserved for a future allow-list-only mode) is not
 * editable here, so round-tripping the textarea never drops or mangles it.
 */
export function domainRulesToText(rules: ContentFetchDomainRule[]): string {
  return rules
    .filter((rule) => !rule.allowed)
    .map((rule) => rule.domain)
    .join('\n')
}

/**
 * Parses the blocklist textarea into per-domain block rules.
 *
 * Trims each line, drops blanks, lowercases hosts (the backend matches
 * case-insensitively so we normalize for a stable round-trip), and de-dupes so
 * a host typed twice yields one rule. Every parsed host becomes an
 * `allowed = false` block rule.
 */
export function buildContentFetchDomainRules(
  text: string,
): ContentFetchDomainRule[] {
  const seen = new Set<string>()
  const rules: ContentFetchDomainRule[] = []
  for (const rawLine of text.split('\n')) {
    const host = rawLine.trim().toLowerCase()
    if (!host || seen.has(host)) continue
    seen.add(host)
    rules.push({ domain: host, allowed: false })
  }
  return rules
}
