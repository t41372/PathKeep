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
 * - Every backend call here is a *delta*, never the accumulated visible list.
 *   `visibleUrls` spans head + all infinite-scroll pages, so both the
 *   `loadHistoryOgImages` batch (diffed against `ogImageCache`) and the
 *   `markOgImagesShown` batch (diffed against `markedShownRef`) are bounded by
 *   what is newly seen, and each flush is additionally capped.
 */

import { useEffect, useMemo, useRef, useState } from 'react'
import { backend } from '../../../lib/backend-client'
import type {
  HistoryEntry,
  HistoryQueryResponse,
  OgImageFetchModeConfig,
} from '../../../lib/types'
import { historyOgImageLookupKey } from '../helpers'

const MARK_SHOWN_DEBOUNCE_MS = 1000
// Bound the per-render enqueue batch so a 200-row Explorer page can't
// stampede the worker pool. The Rust side rate-limits per host on top
// of this so the absolute ceiling is 2 concurrent worker threads × the
// host rate limit, not this number.
const FETCH_ENQUEUE_BATCH_CAP = 20
// Bound one mark-shown flush. `visibleUrls` grows without bound as the user
// infinite-scrolls (up to `MAX_ACCUMULATED_PAGES` × page size), so the flush
// must not be proportional to the accumulated list. Anything beyond the cap is
// picked up by the next scroll tick.
const MARK_SHOWN_BATCH_CAP = 200

interface UseExplorerOgImagesOptions {
  cacheToken: number
  loading: boolean
  results: HistoryQueryResponse | null
  /** When false the hook skips the load + mark-shown calls (list mode). */
  enabled?: boolean
  /**
   * Effective fetch policy from `AppConfig.ogImage` (with the legacy
   * `fetchEnabled` kill switch already folded in by the caller). When
   * `'off'`, the hook still reads existing cached rows from local
   * SQLite via `loadHistoryOgImages` but skips the
   * `triggerOgImageRefetch` enqueue branch entirely — Settings copy
   * promises Off = "No fetching anywhere." Defaults to `'background'`
   * so older callers / tests that don't thread the config through still
   * behave the way they used to.
   */
  fetchMode?: OgImageFetchModeConfig
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
  fetchMode = 'background',
}: UseExplorerOgImagesOptions) {
  const [cacheState, setCacheState] = useState(() => ({
    token: cacheToken,
    entries: new Map<string, HistoryEntry['ogImage'] | null>(),
  }))
  const inflightKeysRef = useRef(new Set<string>())
  const markShownTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const pendingMarkShownRef = useRef<Set<string>>(new Set())
  // Lookup keys already reported to `markOgImagesShown` for the current cache
  // token. Reset alongside the other per-token refs below.
  const markedShownRef = useRef<Set<string>>(new Set())
  const emptyCache = useMemo(
    () => new Map<string, HistoryEntry['ogImage'] | null>(),
    [],
  )
  const ogImageCache =
    cacheState.token === cacheToken ? cacheState.entries : emptyCache

  useEffect(() => {
    inflightKeysRef.current.clear()
    pendingMarkShownRef.current.clear()
    markedShownRef.current.clear()
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
            enqueueCandidates.push(url)
          }
        }
        if (enqueueCandidates.length > 0 && fetchMode !== 'off') {
          // `fetchMode === 'off'` skips this whole enqueue branch: cached
          // rows still hydrate via `loadHistoryOgImages` above, but no
          // new outbound HTTP is requested. The backend
          // `refetch_og_images_impl` also gates on the effective mode for
          // defense in depth, so a buggy caller can't bypass the user's
          // choice. See Codex review finding C1 (data-sovereignty hole).
          const batch = enqueueCandidates.slice(0, FETCH_ENQUEUE_BATCH_CAP)
          // Refetch is async on the worker; without polling for completion,
          // the cache stays on the favicon fallback even when the worker
          // already wrote an `ok` row to the archive. Wait for the worker
          // to finish, then re-read `loadHistoryOgImages` so the next
          // setCacheState picks up the new bytes and the cards swap from
          // favicon to social-card image without requiring a page refresh.
          void backend
            .triggerOgImageRefetch(batch)
            .then(() => {
              if (cancelled) return
              return backend.loadHistoryOgImages(batch.map((url) => ({ url })))
            })
            .then((refreshed) => {
              if (cancelled || !refreshed) return
              setCacheState((current) => {
                const next = new Map(current.entries)
                for (const row of refreshed) {
                  next.set(
                    historyOgImageLookupKey(row.url),
                    row.ogImage ?? null,
                  )
                }
                return { token: cacheToken, entries: next }
              })
            })
            .catch(() => {
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
  }, [cacheToken, enabled, fetchMode, loading, ogImageCache, visibleUrls])

  // Debounced "mark these URLs as shown" so user-configured LRU eviction
  // can prefer rows the user actually looked at.
  //
  // `visibleUrls` is the head page PLUS every infinite-scroll page accumulated
  // so far (`MAX_ACCUMULATED_PAGES = 500` × a 50-row page = 25 000 URLs at the
  // ceiling). Enqueueing the whole list on every page load re-sent the entire
  // accumulated array across IPC once per scroll tick, re-marking rows that
  // were already marked seconds earlier. `markedShownRef` keeps the set we
  // have already reported for this cache token so the enqueue is a pure delta
  // — the same diff-against-what-we-know discipline `loadHistoryOgImages`
  // above already uses against `ogImageCache`.
  useEffect(() => {
    if (!enabled || loading || visibleUrls.length === 0) {
      return
    }
    let queued = 0
    for (const url of visibleUrls) {
      const key = historyOgImageLookupKey(url)
      if (markedShownRef.current.has(key)) continue
      if (pendingMarkShownRef.current.has(url)) continue
      // Bound one flush the way the fetch enqueue is bounded. Anything over
      // the cap is simply left unmarked for now; the next scroll tick re-runs
      // this effect and picks the remainder up, so no URL is lost — the work
      // is just spread across ticks instead of one jumbo payload.
      if (queued >= MARK_SHOWN_BATCH_CAP) break
      pendingMarkShownRef.current.add(url)
      queued += 1
    }
    if (pendingMarkShownRef.current.size === 0) {
      return
    }
    markShownTimerRef.current = setTimeout(() => {
      const urls = Array.from(pendingMarkShownRef.current)
      pendingMarkShownRef.current.clear()
      if (urls.length === 0) return
      for (const url of urls) {
        markedShownRef.current.add(historyOgImageLookupKey(url))
      }
      void backend.markOgImagesShown(urls).catch(() => {
        // mark-shown is best-effort; an LRU signal that drops a single
        // batch isn't worth surfacing as a user-visible error. The keys stay
        // in `markedShownRef` so a failing backend can't turn into a retry
        // storm on every subsequent scroll tick — the LRU hint is advisory.
      })
    }, MARK_SHOWN_DEBOUNCE_MS)
    return () => {
      clearTimeout(markShownTimerRef.current as ReturnType<typeof setTimeout>)
      markShownTimerRef.current = null
    }
  }, [enabled, loading, visibleUrls])

  return { ogImageCache }
}
