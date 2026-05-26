/**
 * @file use-explorer-infinite-pages.ts
 * @description Accumulating page buffer for the paper Browse infinite-scroll surface.
 * @module pages/explorer/hooks
 *
 * ## Responsibilities
 * - Issue paginated `queryHistory` requests for page 2, 3, … on demand
 *   (page 1 stays the responsibility of `useExplorerData` so the existing
 *   blocked-request / fallback gates still apply).
 * - Maintain the accumulated items array so scrolling down lengthens the
 *   contact sheet instead of replacing the current page.
 * - Reset the buffer + page counter when the query signature changes
 *   (new search term, profile scope, sort order, date filter) so the
 *   user never sees stale rows after a filter switch.
 *
 * ## Not responsible for
 * - Fetching page 1 — the existing `useExplorerData` query state is the
 *   authoritative first-page source. This hook composes pages 2+ onto
 *   that head.
 * - Search-surface or grouped (session / trail) views. Those have their
 *   own pagination grammar and the route flips `disabled = true` to keep
 *   this hook dormant.
 *
 * ## Why this hook exists
 * The previous paper Browse fetched a single page and rendered nothing
 * else, so a 12-month archive whose page 1 held only today's visits
 * looked like an "only today" view. This hook turns the contact sheet
 * into a true scrollable timeline backed by the existing `queryHistory`
 * pagination contract.
 *
 * ## Performance notes
 * - Each page fetch reuses the existing `backend.queryHistory` cursor /
 *   page contract, so the worker bridge + Rust archive don't change.
 * - Inflight requests are deduped per page so rapid scroll events
 *   cannot stampede the worker pool.
 */

import { useEffect, useMemo, useRef, useState } from 'react'
import { backend } from '@/lib/backend-client'
import { describeError } from '@/lib/errors'
import type {
  HistoryEntry,
  HistoryQuery,
  HistoryQueryResponse,
} from '@/lib/types'
import type { ScrollDirection } from './use-scroll-direction'

export interface ExplorerInfinitePagesState {
  /** Entries from pages 2..accumulatedPages (page 1 is owned by the caller). */
  extraItems: HistoryEntry[]
  /** Last successfully loaded page index (`1` until a page-2 fetch resolves). */
  loadedPageCount: number
  /** True while a page-N fetch is inflight. */
  loadingMore: boolean
  /** True when the head response signalled more pages and the buffer hasn't reached the end yet. */
  canLoadMore: boolean
  /**
   * True when `canLoadMore` is false specifically because the in-memory
   * page cap was hit and the archive still has more pages to load.
   * Lets the footer render a "jump deeper via search / calendar" hint
   * instead of the misleading "end of archive" copy.
   */
  capReached: boolean
  /** Last error from a page-N fetch, surfaced for the UI to render. */
  error: string | null
  /** Bumps the accumulated page count by one (advances the timeline). */
  loadMore: () => void
}

interface UseExplorerInfinitePagesOptions {
  /** Base query without `page` / `cursor` — those are supplied per fetch. */
  query: HistoryQuery
  /**
   * Page-1 response from `useExplorerData`. The hook reads total /
   * pageCount / hasNext off it to know whether to attempt page 2+.
   */
  headResults: HistoryQueryResponse | null
  /** When true (search mode, date-filtered, archive locked) the hook is dormant. */
  disabled: boolean
  /**
   * Bumped by the route whenever the underlying explorer cache token
   * advances — primarily so a successful archive refresh wipes the
   * accumulated buffer.
   */
  cacheToken: number
  /**
   * Current scroll direction signal. The hook warms one extra page
   * ahead of the sentinel when the user is scrolling `'down'`
   * (the dominant Browse pattern — moving back through history),
   * so a fast scroller never sees the load-more skeleton on the
   * second hop. `'up'` / `'idle'` fall back to the single
   * background prefetch we always do regardless of direction.
   * Optional so older callers (and the dev preview that doesn't
   * track scroll) stay valid.
   */
  scrollDirection?: ScrollDirection
}

/**
 * Hard cap on how many pages we accumulate in memory before stopping
 * infinite scroll. After the BROWSE-VIRT viewport-driven mounting
 * landed (2026-05-25), DOM cost is no longer linear with page count —
 * off-screen day blocks recycle to a placeholder div — so the cap is
 * memory-driven instead of DOM-driven.
 *
 * The original spike doc proposed raising the cap to 1000 (≈50 000
 * entries / 30 MB of HistoryEntry objects). The 2026-05-25 code
 * review §3 flagged that JS-side aggregation is still O(N) per page
 * fetch (the `extraItems` memo iterates every buffered page on every
 * setPageItems, the explorer route then spreads it into
 * `combinedTimeResults`, then maps it again for favicon / og
 * hydration), so the effective work at the cap is ~150 000 array
 * ops per fetch on top of the IPC round-trip. We settle on 500 (~25
 * 000 entries / ~15 MB) as the trade-off between deep-scroll reach
 * and per-fetch aggregation cost on the target 4-core / 8 GB box.
 * Raising further requires an incremental aggregator (append-only,
 * not rebuild-from-scratch); revisit that as a separate work block
 * before bumping again.
 *
 * See `docs/plan/program/browse-virt-spike-2026-05-25.md` for the
 * cap derivation + the §3 follow-up note.
 */
const MAX_ACCUMULATED_PAGES = 500

const SIGNATURE_FIELDS = [
  'q',
  'profileId',
  'browserKind',
  'domain',
  'startTimeMs',
  'endTimeMs',
  'sort',
  'limit',
  'regexMode',
] as const

function querySignature(query: HistoryQuery): string {
  const snapshot: Record<string, unknown> = {}
  for (const field of SIGNATURE_FIELDS) {
    const value = (query as Record<string, unknown>)[field]
    if (value !== undefined && value !== null) snapshot[field] = value
  }
  return JSON.stringify(snapshot)
}

/**
 * Drives the accumulating pagination buffer for paper Browse.
 *
 * Returns a stable state object the route can hand to the contact sheet;
 * the contact sheet uses an `IntersectionObserver` sentinel to call
 * `loadMore()` when the user scrolls past the visible window.
 */
export function useExplorerInfinitePages({
  query,
  headResults,
  disabled,
  cacheToken,
  scrollDirection,
}: UseExplorerInfinitePagesOptions): ExplorerInfinitePagesState {
  const [accumulatedPages, setAccumulatedPages] = useState(1)
  const [pageItems, setPageItems] = useState<Map<number, HistoryEntry[]>>(
    new Map(),
  )
  const [loadingMore, setLoadingMore] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const inflightRef = useRef<Set<number>>(new Set())
  const signatureRef = useRef<string>('')

  // Reset whenever the query signature OR the cache token changes — that's
  // the only honest moment to drop accumulated state without leaving stale
  // rows on screen. The cache-token bump is what fires after an import /
  // backup / explicit refresh, so it must clear the buffer even when the
  // query signature is unchanged; otherwise old page 2..N entries get
  // concatenated onto a freshly reloaded page 1.
  const signature = useMemo(() => querySignature(query), [query])
  const resetKey = `${signature}|${cacheToken}`
  useEffect(() => {
    if (resetKey === signatureRef.current) return
    signatureRef.current = resetKey
    setAccumulatedPages(1)
    setPageItems(new Map())
    setLoadingMore(false)
    setError(null)
    inflightRef.current.clear()
  }, [resetKey])

  // Fetch any page in [2..accumulatedPages] that we don't have yet.
  // Also opportunistically warm the next page so the IntersectionObserver
  // sentinel only ever has to render an already-buffered slice — the
  // visible skeleton stops popping in for fast scrollers on the
  // populated 264k-row archive.
  //
  // `scrollDirection` is intentionally NOT in this effect's dep list:
  // a direction flip would otherwise tear down the foreground fetch in
  // flight (setting `cancelled = true` short-circuits the `.then`,
  // silently dropping the page; the user then sees the loading
  // skeleton disappear with no result and has to re-trigger the
  // sentinel). The directional +2 prefetch lives in its own effect
  // below where the cancellation only kills the opportunistic
  // background fetch.
  useEffect(() => {
    if (disabled) return
    if (!headResults) return
    if (accumulatedPages <= 1) return
    const target = accumulatedPages
    const targetAlreadyBuffered = pageItems.has(target)
    if (targetAlreadyBuffered) setLoadingMore(false)

    const totalPages = headResults.pageCount ?? 0
    const fetchPage = (page: number, options: { background: boolean }) => {
      if (page <= 1) return
      if (page > totalPages) return
      if (pageItems.has(page)) return
      if (inflightRef.current.has(page)) return
      inflightRef.current.add(page)
      if (!options.background) setLoadingMore(true)
      void backend
        .queryHistory({ ...query, page, cursor: null })
        .then((response) => {
          if (cancelled) return
          setPageItems((prev) => {
            const next = new Map(prev)
            next.set(page, response.items)
            return next
          })
          setError(null)
        })
        .catch((reason: unknown) => {
          if (cancelled) return
          // Background prefetch failures stay silent — the user can still
          // scroll, the foreground request will retry next time the
          // sentinel re-fires. Foreground failures roll back the counter
          // so the sentinel retries cleanly instead of skipping the page.
          if (!options.background) {
            setError(describeError(reason, 'query_history_page'))
            setAccumulatedPages((current) =>
              current === page ? current - 1 : current,
            )
          }
        })
        .finally(() => {
          inflightRef.current.delete(page)
          if (!options.background && !cancelled) setLoadingMore(false)
        })
    }

    let cancelled = false
    if (!targetAlreadyBuffered) {
      fetchPage(target, { background: false })
    }
    // Warm one page ahead in the background unconditionally — this is
    // the original prefetch budget and stays polite to the worker
    // pool.
    fetchPage(target + 1, { background: true })

    return () => {
      // Cancellation clears `loadingMore` explicitly. The .finally below
      // skips its `setLoadingMore(false)` when `cancelled === true`
      // (to avoid racing the next effect's `setLoadingMore(true)`), so
      // without this line the loading flag stays stuck whenever a
      // foreground fetch is cancelled mid-flight by a filter / query /
      // cache-token change. Symptom: applying a Browse filter while a
      // page-N fetch is in flight makes the bottom sentinel spin forever
      // even though every subsequent effect run early-returns.
      cancelled = true
      setLoadingMore(false)
    }
    // signature is in the dep list so a switch resets the buffer above
    // before this effect runs; pageItems is intentionally absent so we
    // don't re-trigger on every set.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [accumulatedPages, disabled, headResults, signature])

  // Directional +2 prefetch. Lives in its own effect so a scroll
  // direction flip does NOT cancel the main fetch effect's in-flight
  // foreground request. When the user sustains a downward scroll
  // (`scrollDirection === 'down'`), warm a second page beyond the
  // unconditional +1 so a fast scroller doesn't see the load-more
  // skeleton on the next hop. We cap at +2 to stay polite to the
  // encrypted-SQLite reader; the spike doc documents the budget at
  // `docs/plan/program/browse-virt-spike-2026-05-25.md`.
  useEffect(() => {
    if (disabled) return
    if (!headResults) return
    if (scrollDirection !== 'down') return
    if (accumulatedPages <= 1) return
    const totalPages = headResults.pageCount ?? 0
    const ahead = accumulatedPages + 2
    if (ahead > totalPages || ahead <= 1) return
    if (pageItems.has(ahead) || inflightRef.current.has(ahead)) return

    let cancelled = false
    inflightRef.current.add(ahead)
    void backend
      .queryHistory({ ...query, page: ahead, cursor: null })
      .then((response) => {
        if (cancelled) return
        setPageItems((prev) => {
          const next = new Map(prev)
          next.set(ahead, response.items)
          return next
        })
      })
      .catch(() => {
        // Background-only prefetch — failures stay silent. The
        // foreground request will retry through the main effect if
        // the user keeps scrolling.
      })
      .finally(() => {
        inflightRef.current.delete(ahead)
      })

    return () => {
      // Only the opportunistic +2 prefetch is dropped here; the
      // foreground page still resolves through the main effect above.
      cancelled = true
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [accumulatedPages, disabled, headResults, signature, scrollDirection])

  const extraItems = useMemo(() => {
    const out: HistoryEntry[] = []
    for (let page = 2; page <= accumulatedPages; page += 1) {
      const items = pageItems.get(page)
      if (items) out.push(...items)
    }
    return out
  }, [accumulatedPages, pageItems])

  const canLoadMore = useMemo(() => {
    if (disabled || !headResults) return false
    if (accumulatedPages >= MAX_ACCUMULATED_PAGES) return false
    if (accumulatedPages < (headResults.pageCount ?? 0)) return true
    return Boolean(headResults.hasNext) && accumulatedPages === 1
  }, [accumulatedPages, disabled, headResults])

  // Distinguishes the hard cap from a genuine end-of-archive. The
  // contact-sheet footer uses this to render "Showing first N rows · use
  // search / a date pill to jump deeper" instead of the misleading "End
  // of archive" copy when the user has merely hit the in-memory cap on a
  // truly large archive.
  const capReached = useMemo(() => {
    if (disabled || !headResults) return false
    if (accumulatedPages < MAX_ACCUMULATED_PAGES) return false
    return accumulatedPages < (headResults.pageCount ?? 0)
  }, [accumulatedPages, disabled, headResults])

  const loadMore = useMemo(
    () => () => {
      if (disabled) return
      if (!headResults) return
      if (loadingMore) return
      if (accumulatedPages >= MAX_ACCUMULATED_PAGES) return
      if (accumulatedPages >= (headResults.pageCount ?? 0)) return
      setAccumulatedPages((current) => current + 1)
    },
    [accumulatedPages, disabled, headResults, loadingMore],
  )

  return {
    extraItems,
    loadedPageCount: accumulatedPages,
    loadingMore,
    canLoadMore,
    capReached,
    error,
    loadMore,
  }
}
