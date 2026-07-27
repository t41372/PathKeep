/**
 * Loads the Starred hub list for the Explorer `?surface=starred` mode.
 *
 * Separate from `use-desktop-stars` (the optimistic per-row toggle cache):
 * this hook owns the read model the hub renders — `list_stars` ordered by the
 * chosen sort. It re-fetches when the sort changes or when the caller bumps
 * the reload key (e.g. after an un-star removes an item).
 *
 * ## This is a bounded FIRST PAGE, not a paginated read model
 * The header used to call this "the *paginated read model*". It never was:
 * the call passed no limit, so it silently took the backend's
 * `DEFAULT_LIST_LIMIT` of 500 and dropped everything past it with no signal.
 * A user with 700 stars saw a 700 badge, a 500-row hub, and a search facet
 * reporting 500 — three numbers contradicting each other on one screen.
 *
 * Until `list_stars` grows a real cursor, this hook is honest about what it
 * is: it asks for the backend's hard maximum (`MAX_LIST_LIMIT`) and reports
 * `truncated` when the reply came back full, so the surfaces can tell the user
 * they are looking at a prefix instead of pretending it is everything. Result
 * TOTALS must come from `useStarredCount` (a real aggregate), never from
 * `items.length`.
 *
 * ## Performance notes
 * - Bounded by `STARRED_HUB_LIMIT` rows, independent of archive size. This
 *   hook never lists the archive; it only ever holds the starred set.
 *
 * ## Loading model
 * - `loading` is *derived*, not a synchronously-set effect flag: it is true
 *   whenever the resolved snapshot key trails the requested key. This keeps the
 *   effect free of the cascading synchronous `setState` the lint gate forbids
 *   while still showing the skeleton on the very first paint of a new request.
 */

import { useCallback, useEffect, useMemo, useState } from 'react'
import { backend } from '../../lib/backend-client'
import type { StarListItem, StarSort } from '../../lib/backend-client'
import { describeError } from '../../lib/errors'

/**
 * Rows requested per hub load. Mirrors `MAX_LIST_LIMIT` in
 * `vault-core::stars` — the backend clamps to it, so asking for more would be
 * a lie about what we can receive, and asking for less (the old implicit
 * `DEFAULT_LIST_LIMIT` of 500) truncated four times sooner than necessary.
 */
export const STARRED_HUB_LIMIT = 2_000

export interface StarredHub {
  items: StarListItem[]
  loading: boolean
  sort: StarSort
  setSort: (sort: StarSort) => void
  reload: () => void
  lastError: string | null
  /**
   * True when the reply filled the request exactly, i.e. the backend may be
   * holding more stars than `items` shows. Surfaces MUST tell the user rather
   * than presenting the prefix as the whole set.
   */
  truncated: boolean
  /** The row cap that produced `items`. Rendered in the truncation notice. */
  limit: number
}

interface StarredSnapshot {
  key: string
  items: StarListItem[]
  error: string | null
}

const EMPTY_SNAPSHOT: StarredSnapshot = { key: '', items: [], error: null }

export function useStarredHub(enabled: boolean): StarredHub {
  const [sort, setSort] = useState<StarSort>('recently_starred')
  const [reloadKey, setReloadKey] = useState(0)
  const [snapshot, setSnapshot] = useState<StarredSnapshot>(EMPTY_SNAPSHOT)

  // The key the current props *want* resolved. When the snapshot's key trails
  // this, we are mid-flight → loading.
  const requestKey = enabled ? `${sort}:${reloadKey}` : ''

  const reload = useCallback(() => setReloadKey((key) => key + 1), [])

  useEffect(() => {
    if (!enabled) return
    let cancelled = false
    backend
      .listStars(null, sort, STARRED_HUB_LIMIT)
      .then((rows) => {
        if (cancelled) return
        setSnapshot({ key: requestKey, items: rows, error: null })
      })
      .catch((error: unknown) => {
        if (cancelled) return
        setSnapshot({
          key: requestKey,
          items: [],
          error: describeError(error, 'starred-hub'),
        })
      })
    return () => {
      cancelled = true
    }
  }, [enabled, sort, requestKey])

  const loading = enabled && snapshot.key !== requestKey
  const items = useMemo(
    () => (snapshot.key === requestKey ? snapshot.items : []),
    [snapshot, requestKey],
  )

  return {
    items,
    loading,
    sort,
    setSort,
    reload,
    lastError: snapshot.error,
    truncated: items.length >= STARRED_HUB_LIMIT,
    limit: STARRED_HUB_LIMIT,
  }
}
