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
  getOgImageStorageStats: () =>
    call<OgImageStorageStats>('get_og_image_storage_stats', {}),
  clearOgImageCache: () =>
    call<OgImageCleanupReport>('clear_og_image_cache', {}),
  runOgImageCleanup: () =>
    call<OgImageCleanupReport>('run_og_image_cleanup', {}),
}
