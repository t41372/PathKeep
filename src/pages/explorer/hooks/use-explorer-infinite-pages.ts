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
 * - Date-narrowed views (`?date=YYYY-MM-DD`). Those skip accumulation
 *   and pagination handles the rare case where a single day overflows
 *   the page size.
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
import type {
  HistoryEntry,
  HistoryQuery,
  HistoryQueryResponse,
} from '@/lib/types'

export interface ExplorerInfinitePagesState {
  /** Entries from pages 2..accumulatedPages (page 1 is owned by the caller). */
  extraItems: HistoryEntry[]
  /** Last successfully loaded page index (`1` until a page-2 fetch resolves). */
  loadedPageCount: number
  /** True while a page-N fetch is inflight. */
  loadingMore: boolean
  /** True when the head response signalled more pages and the buffer hasn't reached the end yet. */
  canLoadMore: boolean
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
}

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
}: UseExplorerInfinitePagesOptions): ExplorerInfinitePagesState {
  const [accumulatedPages, setAccumulatedPages] = useState(1)
  const [pageItems, setPageItems] = useState<Map<number, HistoryEntry[]>>(
    new Map(),
  )
  const [loadingMore, setLoadingMore] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const inflightRef = useRef<Set<number>>(new Set())
  const signatureRef = useRef<string>('')

  // Reset whenever the query signature or cache token changes — that's the
  // only honest moment to drop accumulated state without leaving stale
  // rows on screen.
  const signature = useMemo(() => querySignature(query), [query])
  useEffect(() => {
    if (signature === signatureRef.current) return
    signatureRef.current = signature
    setAccumulatedPages(1)
    setPageItems(new Map())
    setLoadingMore(false)
    setError(null)
    inflightRef.current.clear()
  }, [signature, cacheToken])

  // Fetch any page in [2..accumulatedPages] that we don't have yet.
  // Also opportunistically warm the next page so the IntersectionObserver
  // sentinel only ever has to render an already-buffered slice — the
  // visible skeleton stops popping in for fast scrollers on the
  // populated 264k-row archive.
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
            const message =
              reason instanceof Error ? reason.message : String(reason)
            setError(message)
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
    // Warm the next page in the background. We only warm one page ahead
    // to stay polite to the worker pool — encrypted SQLite reads on the
    // 264k-row archive are not free even when nobody is watching.
    fetchPage(target + 1, { background: true })

    return () => {
      cancelled = true
    }
    // signature is in the dep list so a switch resets the buffer above
    // before this effect runs; pageItems is intentionally absent so we
    // don't re-trigger on every set.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [accumulatedPages, disabled, headResults, signature])

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
    if (accumulatedPages < (headResults.pageCount ?? 0)) return true
    return Boolean(headResults.hasNext) && accumulatedPages === 1
  }, [accumulatedPages, disabled, headResults])

  const loadMore = useMemo(
    () => () => {
      if (disabled) return
      if (!headResults) return
      if (loadingMore) return
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
    error,
    loadMore,
  }
}
