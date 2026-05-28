/**
 * This module wraps a focused slice of desktop commands behind a typed front-end client.
 *
 * Why this file exists:
 * - The `backend-client` layer keeps page components from having to know raw command names or transport details.
 * - If a route needs desktop data, start here before reaching for legacy preview helpers.
 *
 * Main declarations:
 * - `explorerClient`
 *
 * Source-of-truth notes:
 * - Transport boundaries are defined by `docs/architecture/desktop-command-surface.md`.
 * - This layer should stay typed, boring, and free of user-facing copy so routes can keep ownership of UX decisions.
 */

import type {
  HistoryFaviconLookupEntry,
  HistoryFaviconLookupResult,
  HistoryOgImageLookupEntry,
  HistoryOgImageLookupResult,
  HistoryQuery,
  HistoryQueryResponse,
  OgImageCleanupReport,
  OgImageStorageStats,
} from '../types'
import { call } from './shared'

/**
 * Exposes the focused client surface for explorer commands.
 *
 * The backend-client layer exists to keep command names, transport, and route code decoupled, so focused declarations here are intentional.
 */
/**
 * Per-domain row inside `BrowseDayInsights.topDomains`.
 */
export interface BrowseDayTopDomain {
  domain: string
  visits: number
}

/**
 * Per-URL row inside `BrowseDayInsights.topUrls`.
 */
export interface BrowseDayTopUrl {
  url: string
  title: string | null
  visits: number
}

/**
 * Per-search-query row inside `BrowseDayInsights.topSearchQueries`.
 */
export interface BrowseDaySearchQuery {
  query: string
  count: number
}

/**
 * One local-calendar day's Browse insights, computed from the full
 * archive (not the scroll-loaded subset). Returned by
 * `get_browse_day_insights`.
 */
export interface BrowseDayInsights {
  date: string
  totalPages: number
  typedCount: number
  linkCount: number
  searchCount: number
  distinctDomains: number
  sessionCount: number
  topDomains: BrowseDayTopDomain[]
  /** Visits per local hour bucket, length 24. */
  hourBuckets: number[]
  /** Highest single-hour count; ≥ 1 so callers can divide safely. */
  hourPeak: number
  firstVisitMs: number | null
  lastVisitMs: number | null
  peakHour: number | null
  longestSessionMs: number
  topUrls: BrowseDayTopUrl[]
  topSearchQueries: BrowseDaySearchQuery[]
}

export const explorerClient = {
  queryHistory: (query: HistoryQuery) =>
    call<HistoryQueryResponse>('query_history', { query }),
  loadHistoryFavicons: (entries: HistoryFaviconLookupEntry[]) =>
    call<HistoryFaviconLookupResult[]>('load_history_favicons', { entries }),
  loadHistoryOgImages: (entries: HistoryOgImageLookupEntry[]) =>
    call<HistoryOgImageLookupResult[]>('load_history_og_images', { entries }),
  markOgImagesShown: (urls: string[]) =>
    call<void>('mark_og_images_shown', { urls }),
  triggerOgImageRefetch: (urls: string[]) =>
    call<number>('trigger_og_image_refetch', { urls }),
  /**
   * User-initiated prefetch sweep — enqueues visited URLs without an
   * `og_images` row, capped at `budget`. Returns `[enqueued, succeeded]`.
   */
  prefetchOgImages: (budget: number) =>
    call<[number, number]>('prefetch_og_images', { budget }),
  getOgImageStorageStats: () =>
    call<OgImageStorageStats>('get_og_image_storage_stats', {}),
  clearOgImageCache: () =>
    call<OgImageCleanupReport>('clear_og_image_cache', {}),
  runOgImageCleanup: () =>
    call<OgImageCleanupReport>('run_og_image_cleanup', {}),
  /**
   * Aggregates one local-calendar day's Browse insights from the full
   * archive — replaces the previous client-side `aggregateDayInsights`
   * which only saw scroll-loaded cards. See feedback-2026-05-25 §3.1.
   */
  getBrowseDayInsights: (request: {
    date: string
    profileId?: string | null
  }) =>
    call<BrowseDayInsights>('get_browse_day_insights', {
      request: {
        date: request.date,
        profileId: request.profileId ?? null,
      },
    }),
}
