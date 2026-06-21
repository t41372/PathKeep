/**
 * Loads the Starred hub list for the Explorer `?surface=starred` mode.
 *
 * Separate from `use-desktop-stars` (the optimistic per-row toggle cache):
 * this hook owns the *paginated read model* the hub renders — `list_stars`
 * ordered by the chosen sort. It re-fetches when the sort changes or when the
 * caller bumps the reload key (e.g. after an un-star removes an item).
 *
 * ## Performance notes
 * - `list_stars` is already bounded server-side (the star table is tiny). This
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

export interface StarredHub {
  items: StarListItem[]
  loading: boolean
  sort: StarSort
  setSort: (sort: StarSort) => void
  reload: () => void
  lastError: string | null
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
      .listStars(null, sort)
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
  }
}
