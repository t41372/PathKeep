/**
 * Pure helpers for the detail-panel enriched-content section (W-ENRICH-1, 06 §6).
 *
 * The detail surface renders whatever the backend already fetched. These helpers
 * keep two fiddly, edge-case-heavy concerns testable away from React:
 * - Parsing the opaque `metadataJson` string into the few display fields the
 *   panel shows (GitHub topics / description), tolerating any malformed or
 *   unexpected shape without throwing on the render path.
 * - Mapping the backend's honest `fetchStatus` taxonomy onto a small set of
 *   FE status categories so a paywall / non-HTML / rate-limited / blocked fetch
 *   shows a truthful message instead of a fake success.
 *
 * ## Not responsible for
 * - Transport, i18n, or layout (the component owns those).
 */

import type { VisitEnrichmentRecord } from '@/lib/types'

/** FE-facing source category derived from a record's `contentSource`. */
export type EnrichmentSourceKind = 'github' | 'generic' | 'unknown'

/**
 * The small, render-ready view of one enrichment record.
 *
 * `topics` is always an array (possibly empty); `description`/`summary` are the
 * human text the panel shows. `statusKind` is a closed set so the panel can map
 * each to one honest i18n string.
 */
export interface EnrichmentView {
  sourceKind: EnrichmentSourceKind
  /** Whether the fetch produced usable content (drives showing body vs status). */
  ok: boolean
  statusKind:
    | 'success'
    | 'empty'
    | 'blocked'
    | 'error'
    | 'login'
    | 'unsupported'
    | 'rate-limited'
  title?: string
  summary?: string
  description?: string
  topics: string[]
  fetchedAt: string
  finalUrl?: string
}

/** Maps a record's `contentSource` to the FE source category. */
export function enrichmentSourceKind(
  contentSource: string,
): EnrichmentSourceKind {
  if (contentSource === 'github-repo') return 'github'
  if (contentSource === 'generic-readable') return 'generic'
  return 'unknown'
}

/**
 * Normalizes the backend `fetchStatus` string to one of the FE status kinds.
 *
 * The backend taxonomy is honest but open-ended; this collapses the known
 * markers (login walls, non-HTML, rate-limit, SSRF/blocklist, transient errors,
 * empty extractions) into the closed set the UI has copy for. Anything
 * unrecognized but non-success is treated as a generic error rather than
 * silently rendering as success.
 */
export function enrichmentStatusKind(
  fetchStatus: string,
): EnrichmentView['statusKind'] {
  const status = fetchStatus.trim().toLowerCase()
  if (status === 'success' || status === 'ok' || status === 'fetched') {
    return 'success'
  }
  if (
    status === 'empty' ||
    status === 'no-content' ||
    status === 'no_content'
  ) {
    return 'empty'
  }
  if (status === 'blocked' || status === 'disallowed' || status === 'ssrf') {
    return 'blocked'
  }
  if (
    status === 'login' ||
    status === 'login-required' ||
    status === 'login_required' ||
    status === 'paywall' ||
    status === 'forbidden' ||
    status === 'unauthorized'
  ) {
    return 'login'
  }
  if (
    status === 'unsupported' ||
    status === 'non-html' ||
    status === 'non_html' ||
    status === 'unsupported-type' ||
    status === 'unsupported_type'
  ) {
    return 'unsupported'
  }
  if (
    status === 'rate-limited' ||
    status === 'rate_limited' ||
    status === 'ratelimited' ||
    status === 'throttled'
  ) {
    return 'rate-limited'
  }
  return 'error'
}

/**
 * Pull a string array out of an unknown value, dropping non-strings + blanks.
 *
 * GitHub topics arrive as a JSON array under any of a few likely keys; this
 * keeps the parse forgiving so a slightly different backend shape never throws.
 */
function asStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) return []
  return value
    .filter((item): item is string => typeof item === 'string')
    .map((item) => item.trim())
    .filter((item) => item.length > 0)
}

function asString(value: unknown): string | undefined {
  if (typeof value !== 'string') return undefined
  const trimmed = value.trim()
  return trimmed.length > 0 ? trimmed : undefined
}

/**
 * Parse the opaque `metadataJson` for the display fields the panel renders.
 *
 * Tolerates `null`, malformed JSON, or an unexpected shape by returning empty
 * fields — the render path must never throw on stored metadata. Recognizes the
 * common GitHub keys (`topics`, `description`/`repoDescription`).
 */
export function parseEnrichmentMetadata(
  metadataJson: string | null | undefined,
): { description?: string; topics: string[] } {
  if (!metadataJson) return { topics: [] }
  let parsed: unknown
  try {
    parsed = JSON.parse(metadataJson)
  } catch {
    return { topics: [] }
  }
  if (typeof parsed !== 'object' || parsed === null) {
    return { topics: [] }
  }
  const record = parsed as Record<string, unknown>
  const topics =
    asStringArray(record.topics).length > 0
      ? asStringArray(record.topics)
      : asStringArray(record.tags)
  const description =
    asString(record.description) ?? asString(record.repoDescription)
  return { description, topics }
}

/**
 * Build the render-ready view from one stored enrichment record.
 *
 * Centralizes the source + status + metadata derivation so the component stays a
 * thin renderer. `ok` is true only on a success status — every other status
 * routes to an honest message and suppresses the (absent) body.
 */
export function toEnrichmentView(
  record: VisitEnrichmentRecord,
): EnrichmentView {
  const statusKind = enrichmentStatusKind(record.fetchStatus)
  const { description, topics } = parseEnrichmentMetadata(record.metadataJson)
  return {
    sourceKind: enrichmentSourceKind(record.contentSource),
    ok: statusKind === 'success',
    statusKind,
    title: record.readableTitle ?? undefined,
    summary: record.summary ?? undefined,
    description,
    topics,
    fetchedAt: record.fetchedAt,
    finalUrl: record.finalUrl ?? undefined,
  }
}

/**
 * Choose the single best record to render for one visit.
 *
 * The backend may return more than one enrichment row (e.g. a structured
 * GitHub row + a generic fallback). Prefer a successful row, then the most
 * recent by `fetchedAt`, so the panel shows real content over a stale failure
 * marker. Returns null for an empty list (the caller shows the empty state).
 */
export function pickBestEnrichment(
  records: readonly VisitEnrichmentRecord[],
): VisitEnrichmentRecord | null {
  if (records.length === 0) return null
  const ranked = [...records].sort((a, b) => {
    const aOk = enrichmentStatusKind(a.fetchStatus) === 'success' ? 1 : 0
    const bOk = enrichmentStatusKind(b.fetchStatus) === 'success' ? 1 : 0
    if (aOk !== bOk) return bOk - aOk
    return b.fetchedAt.localeCompare(a.fetchedAt)
  })
  // `records.length > 0` (checked above) guarantees ranked[0]; the `?? null` is
  // a defensive fallback the type system can't prove away.
  /* v8 ignore next */
  return ranked[0] ?? null
}
