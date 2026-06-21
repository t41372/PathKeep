/**
 * Typed front-end client for the site-content-enrichment commands (W-ENRICH-1).
 *
 * Why this file exists:
 * - PathKeep can optionally fetch structured content for sites the user already
 *   visited (GitHub repo metadata, a generic readable summary) and store it as
 *   enrichment metadata. The Settings consent panel reads/writes the master
 *   switch + per-extractor + per-domain rules; the detail panel reads one
 *   visit's stored enrichment and can trigger a manual PME "fetch now".
 * - Keeping the typed client here means the Settings + detail surfaces never
 *   type raw command names ("content_fetch_now") and stay shielded against
 *   transport renames. Mirrors `stars.ts`.
 *
 * Privacy posture (06 §2):
 * - The master switch is HARD-DEFAULT-OFF. NO network egress happens until the
 *   user opts in. The read paths NEVER block on the network — `listVisitEnrichment`
 *   returns whatever was already fetched (or nothing), and the detail panel falls
 *   back to title/URL when empty.
 *
 * Main declarations:
 * - `contentEnrichmentClient`
 *
 * Source-of-truth notes:
 * - Transport contract: `docs/architecture/desktop-command-surface.md`.
 * - Shape contract: `vault_core::models::intelligence` (`ContentFetchSettings`,
 *   `ContentFetchExtractorPreference`, `ContentFetchDomainRule`,
 *   `VisitEnrichmentRecord`, `ContentFetchNowRequest`, `ContentFetchNowResult`).
 *   The TS mirrors live in `src/lib/types/intelligence.ts`.
 * - Backend design: `docs/plan/program/ai-redesign-2026/06-site-enrichment-design.md`.
 */

import { hasDesktopCommandTransport } from '../runtime'
import type {
  AppSnapshot,
  ContentFetchNowRequest,
  ContentFetchNowResult,
  ContentFetchSettings,
  VisitEnrichmentRecord,
} from '../types'
import { call } from './shared'

/**
 * Inert content-fetch settings used as the browser-preview fallback.
 *
 * The Vercel preview has no desktop backend, so the consent surface must show
 * the honest default — everything OFF, nothing fetched, no extractors — rather
 * than crashing on an unimplemented command. This mirrors how `stars.ts`
 * degrades (its hooks swallow the unimplemented-command throw); here we return
 * the safe default directly so the consent panel reads as "off, never fetched"
 * exactly as a fresh install would.
 */
const PREVIEW_CONTENT_FETCH_SETTINGS: ContentFetchSettings = {
  enabled: false,
  extractors: [],
  domains: [],
  queuedJobs: 0,
  runningJobs: 0,
  failedJobs: 0,
  storedRecords: 0,
}

export const contentEnrichmentClient = {
  /**
   * Reads the content-fetch consent + live status surface for Settings.
   *
   * In browser-preview (no desktop transport) this returns the inert default
   * so the consent panel degrades to "off / never fetched" instead of throwing.
   */
  getContentFetchSettings: (): Promise<ContentFetchSettings> => {
    if (!hasDesktopCommandTransport()) {
      return Promise.resolve(PREVIEW_CONTENT_FETCH_SETTINGS)
    }
    return call<ContentFetchSettings>('get_content_fetch_settings', {})
  },

  /**
   * Persists the content-fetch consent settings (master switch + per-extractor +
   * per-domain). Turning the master switch on is the consent gate. Returns the
   * updated app snapshot so the caller can re-sync shell state.
   *
   * This is a real consent mutation — it has no browser-preview fixture and the
   * caller surfaces the unimplemented-command error honestly rather than faking
   * a successful opt-in.
   */
  setContentFetchSettings: (settings: ContentFetchSettings) =>
    call<AppSnapshot>('set_content_fetch_settings', { settings }),

  /**
   * Lists the stored content enrichment for one visit (detail panel).
   *
   * Read-only and NEVER blocks on the network: an absent enrichment yields an
   * empty list and the detail panel falls back to title/URL. In browser-preview
   * there is nothing fetched, so this returns `[]`.
   */
  listVisitEnrichment: (
    historyId: number,
  ): Promise<VisitEnrichmentRecord[]> => {
    if (!hasDesktopCommandTransport()) {
      return Promise.resolve([])
    }
    return call<VisitEnrichmentRecord[]>('list_visit_enrichment', { historyId })
  },

  /**
   * Manual "fetch now" PME trigger for one URL's content enrichment. Honest
   * about consent: the backend returns a `disabled` result (without queuing)
   * when fetching is off for the URL, which the detail panel maps to a
   * "consent required" affordance. No preview fixture — the surface guards on
   * consent before ever calling this.
   */
  contentFetchNow: (request: ContentFetchNowRequest) =>
    call<ContentFetchNowResult>('content_fetch_now', { request }),

  /**
   * Enqueues the prioritized working set for content fetch (the bulk hook).
   * Returns the number of jobs enqueued (0 when fetching is disabled). In
   * browser-preview nothing is enqueued, so this returns 0.
   */
  enqueueContentFetchWorkingSet: (limit?: number): Promise<number> => {
    if (!hasDesktopCommandTransport()) {
      return Promise.resolve(0)
    }
    return call<number>('enqueue_content_fetch_working_set', {
      limit: limit ?? null,
    })
  },
}
