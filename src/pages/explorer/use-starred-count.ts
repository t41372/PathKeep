/**
 * Loads the total starred-entity count for the Explorer's Starred-hub entry badge.
 *
 * Separate from `use-starred-hub` (the paginated hub list) and `use-desktop-stars`
 * (the optimistic per-row toggle cache): this hook owns the single *aggregate
 * count* the discoverable "Open starred" affordance shows so the user can see how
 * much is kept without opening the hub.
 *
 * ## Why a count read, not the list
 * - The hub list is only loaded on the `?surface=starred` surface; the entry
 *   badge lives in the Browse filter strip where that list is empty. Reusing the
 *   list there would always read 0 — dishonest. `get_star_counts` is the bounded
 *   aggregate read model (a `COUNT`, never a scan) built exactly for this.
 *
 * ## Performance notes
 * - Reads `get_star_counts` ONCE on mount and only again when the caller bumps
 *   `reloadToken` (i.e. after a star toggle). It NEVER fetches on every render and
 *   NEVER lists the archive. At 14.4M visits the cost is a single indexed COUNT,
 *   independent of archive size.
 * - In browser-preview (no desktop transport) it stays at 0 without calling the
 *   backend, matching a fresh install.
 */

import { useEffect, useState } from 'react'
import { backend } from '../../lib/backend-client'
import { hasDesktopCommandTransport } from '../../lib/runtime'

export interface StarredCount {
  /** Total starred entities (urls + domains). `0` until the first read lands. */
  total: number
  /** True while the count is loaded and trustworthy (a real read completed). */
  loaded: boolean
}

// Browser-preview has no stars backend, so the count is a trustworthy empty
// straight away — matching a fresh install — without ever touching the effect.
const PREVIEW_EMPTY: StarredCount = { total: 0, loaded: true }

export function useStarredCount(reloadToken: number = 0): StarredCount {
  const desktop = hasDesktopCommandTransport()
  // Async read result for the desktop path. The browser-preview path never
  // resolves this (the effect bails), so we derive its value below instead of
  // setting state synchronously inside the effect (forbidden by the lint gate).
  const [fetched, setFetched] = useState<StarredCount>({
    total: 0,
    loaded: false,
  })

  useEffect(() => {
    if (!desktop) return
    let cancelled = false
    backend
      .getStarCounts()
      .then((counts) => {
        if (cancelled) return
        setFetched({ total: counts.urls + counts.domains, loaded: true })
      })
      .catch(() => {
        // A failed count must not crash the toolbar; keep the entry usable and
        // simply suppress the badge by reporting an untrustworthy count.
        if (cancelled) return
        setFetched({ total: 0, loaded: false })
      })
    return () => {
      cancelled = true
    }
  }, [desktop, reloadToken])

  return desktop ? fetched : PREVIEW_EMPTY
}
