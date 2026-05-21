/**
 * @file use-explorer-og-images.ts
 * @description Lazy og:image hydration owner for card-mode Browse rows.
 * @module pages/explorer
 *
 * ## Responsibilities
 * - Keep the main Explorer history query payload free of og:image bytes.
 * - Batch-load missing og:image payloads for already-visible URLs.
 * - Enqueue a bounded background fetch (`triggerOgImageRefetch`) for any
 *   visible URL the cache has no `ok` row for, so cards eventually pick up
 *   the real social-card image instead of being stuck on the favicon
 *   fallback forever. Both literal cache-miss URLs and existing-but-not-ok
 *   rows are eligible — the backend dedupes via `og_image_blobs` and
 *   honours per-host rate limit + blocked-host policy.
 * - Bump `last_shown_at` (LRU eviction signal) in a debounced batch when
 *   cards land in the viewport.
 * - Cache resolved og:image lookups for the current Explorer data epoch.
 *
 * ## Not responsible for
 * - Settings → Storage cleanup / blocklist UI; those call
 *   `triggerOgImageRefetch` directly for a Settings-driven refetch sweep.
 * - Rendering placeholders or precedence between og:image / favicon /
 *   swatch — that belongs to the card frame component.
 *
 * ## Dependencies
 * - Depends on `backend.loadHistoryOgImages` + `backend.markOgImagesShown`.
 * - Reuses Explorer helper keys so row + cache identity stay aligned.
 *
 * ## Performance notes
 * - Mirrors `useExplorerFavicons` for the dedup + inflight + cache-token
 *   invalidation behaviour, just without the visit-time scoping (og:image
 *   is page-level).
 */

import { useEffect, useMemo, useRef, useState } from 'react'
import { backend } from '../../../lib/backend-client'
import type { HistoryEntry, HistoryQueryResponse } from '../../../lib/types'
import { historyOgImageLookupKey } from '../helpers'

const MARK_SHOWN_DEBOUNCE_MS = 1000
// Bound the per-render enqueue batch so a 200-row Explorer page can't
// stampede the worker pool. The Rust side rate-limits per host on top
// of this so the absolute ceiling is 2 concurrent worker threads × the
// host rate limit, not this number.
const FETCH_ENQUEUE_BATCH_CAP = 20

interface UseExplorerOgImagesOptions {
  cacheToken: number
  loading: boolean
  results: HistoryQueryResponse | null
  /** When false the hook skips the load + mark-shown calls (list mode). */
  enabled?: boolean
}

/**
 * Loads card-mode og:image bytes after the row window has settled.
 *
 * Returns a Map keyed by page URL so callers can look up entries in O(1)
 * during render. Entries that resolve to a non-`ok` fetch_status are stored
 * as `null` in the cache, which lets the card frame fall back to the
 * favicon overlay without re-issuing the lookup.
 */
export function useExplorerOgImages({
  cacheToken,
  loading,
  results,
  enabled = true,
}: UseExplorerOgImagesOptions) {
  const [cacheState, setCacheState] = useState(() => ({
    token: cacheToken,
    entries: new Map<string, HistoryEntry['ogImage'] | null>(),
  }))
  const inflightKeysRef = useRef(new Set<string>())
  const markShownTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const pendingMarkShownRef = useRef<Set<string>>(new Set())
  // Tracks URLs we've already kicked at the refetch worker in this cache
  // epoch so a re-render doesn't re-enqueue them. Cleared whenever the
  // explorer cacheToken bumps (new query, fresh load).
  const enqueuedFetchRef = useRef<Set<string>>(new Set())
  const emptyCache = useMemo(
    () => new Map<string, HistoryEntry['ogImage'] | null>(),
    [],
  )
  const ogImageCache =
    cacheState.token === cacheToken ? cacheState.entries : emptyCache

  useEffect(() => {
    inflightKeysRef.current.clear()
    pendingMarkShownRef.current.clear()
    enqueuedFetchRef.current.clear()
  }, [cacheToken])

  const visibleUrls = useMemo(() => {
    if (!results?.items.length) {
      return [] as string[]
    }
    const seen = new Set<string>()
    const deduped: string[] = []
    for (const item of results.items) {
      const key = historyOgImageLookupKey(item.url)
      if (seen.has(key)) continue
      seen.add(key)
      deduped.push(item.url)
    }
    return deduped
  }, [results])

  useEffect(() => {
    if (!enabled || loading || visibleUrls.length === 0) {
      return
    }

    const missing = visibleUrls.filter(
      (url) =>
        !ogImageCache.has(historyOgImageLookupKey(url)) &&
        !inflightKeysRef.current.has(historyOgImageLookupKey(url)),
    )
    if (missing.length === 0) {
      return
    }

    let cancelled = false
    for (const url of missing) {
      inflightKeysRef.current.add(historyOgImageLookupKey(url))
    }

    void backend
      .loadHistoryOgImages(missing.map((url) => ({ url })))
      .then((loaded) => {
        if (cancelled) return
        const loadedByKey = new Map(
          loaded.map((row) => [historyOgImageLookupKey(row.url), row]),
        )
        // Any URL we asked about that didn't come back with an `ok`
        // row needs a real network fetch. Bound the enqueue batch so a
        // 1440 M-row archive can't synchronously hand the worker pool
        // the entire visible window.
        const enqueueCandidates: string[] = []
        for (const url of missing) {
          const key = historyOgImageLookupKey(url)
          const row = loadedByKey.get(key)
          if (!row || row.fetchStatus !== 'ok') {
            if (!enqueuedFetchRef.current.has(key)) {
              enqueueCandidates.push(url)
              enqueuedFetchRef.current.add(key)
            }
          }
        }
        if (enqueueCandidates.length > 0) {
          const batch = enqueueCandidates.slice(0, FETCH_ENQUEUE_BATCH_CAP)
          void backend.triggerOgImageRefetch(batch).catch(() => {
            // refetch is best-effort: rate-limit, network failure, or
            // disabled-fetch settings all reject without poisoning the
            // cache load. Surface nothing to the user — the worker
            // persists a negative-cache row regardless.
          })
        }
        setCacheState((current) => {
          const next = new Map(current.entries)
          for (const url of missing) {
            next.set(historyOgImageLookupKey(url), null)
          }
          for (const row of loaded) {
            next.set(historyOgImageLookupKey(row.url), row.ogImage ?? null)
          }
          return { token: cacheToken, entries: next }
        })
      })
      .finally(() => {
        for (const url of missing) {
          inflightKeysRef.current.delete(historyOgImageLookupKey(url))
        }
      })

    return () => {
      cancelled = true
    }
  }, [cacheToken, enabled, loading, ogImageCache, visibleUrls])

  // Debounced "mark these URLs as shown" so user-configured LRU eviction
  // can prefer rows the user actually looked at.
  useEffect(() => {
    if (!enabled || loading || visibleUrls.length === 0) {
      return
    }
    for (const url of visibleUrls) {
      pendingMarkShownRef.current.add(url)
    }
    if (markShownTimerRef.current) {
      clearTimeout(markShownTimerRef.current)
    }
    markShownTimerRef.current = setTimeout(() => {
      const urls = Array.from(pendingMarkShownRef.current)
      pendingMarkShownRef.current.clear()
      if (urls.length === 0) return
      void backend.markOgImagesShown(urls).catch(() => {
        // mark-shown is best-effort; an LRU signal that drops a single
        // batch isn't worth surfacing as a user-visible error.
      })
    }, MARK_SHOWN_DEBOUNCE_MS)
    return () => {
      if (markShownTimerRef.current) {
        clearTimeout(markShownTimerRef.current)
      }
    }
  }, [enabled, loading, visibleUrls])

  return { ogImageCache }
}
