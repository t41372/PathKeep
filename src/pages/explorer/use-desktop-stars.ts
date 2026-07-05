/**
 * Backend-backed stars (favorites / 加星) hook for the Explorer surfaces.
 *
 * Mirrors `use-desktop-annotations`: an in-memory cache keyed by entity key,
 * lazy batched hydration for the currently-visible rows only, and optimistic
 * toggles that write through to `set_star` / `unset_star`.
 *
 * ## Responsibilities
 * - `isStarred(kind, key)` reads the optimistic cache (defaults to false).
 * - `hydrate(kind, keys)` batches a single `get_star_status` for the supplied
 *   visible keys, skipping keys already known, so a render window never fans
 *   out across the whole archive.
 * - `toggle(kind, key)` flips the cache immediately (<100 ms, no await) and
 *   writes through; on failure it rolls the optimistic value back and records
 *   `lastError` so the surface can show the user it didn't save.
 *
 * ## Not responsible for
 * - The Starred hub list / counts — those read `list_stars` / `get_star_counts`
 *   directly (a different, paginated read model).
 * - Conflict resolution across sessions — last write wins, like annotations.
 *
 * ## Performance notes
 * - Hydration is batched per visible window. The hook NEVER lists every star on
 *   render; status comes from a bounded IN-list keyed by the visible rows. At
 *   14.4M visits the lookup cost scales with the render window, not the archive.
 */

import { useCallback, useMemo, useRef, useState } from 'react'
import { backend } from '../../lib/backend-client'
import type { StarEntityKind } from '../../lib/backend-client'
import { describeError } from '../../lib/errors'

/** Composite cache key so url + domain stars never collide. */
function cacheKey(kind: StarEntityKind, key: string): string {
  return `${kind}::${key}`
}

export interface DesktopStars {
  isStarred(kind: StarEntityKind, key: string | null | undefined): boolean
  /** Batch-hydrate the supplied visible keys for one kind. Skips known keys. */
  hydrate(kind: StarEntityKind, keys: readonly string[]): void
  toggle(kind: StarEntityKind, key: string): void
  lastError: string | null
}

export function useDesktopStars(): DesktopStars {
  const [cache, setCache] = useState<Record<string, boolean>>({})
  // Keys whose status we have already fetched (or optimistically set), so the
  // batched hydrate never re-requests them.
  const knownRef = useRef<Set<string>>(new Set())
  const [lastError, setLastError] = useState<string | null>(null)

  const isStarred = useCallback(
    (kind: StarEntityKind, key: string | null | undefined) => {
      if (!key) return false
      return cache[cacheKey(kind, key)] ?? false
    },
    [cache],
  )

  const hydrate = useCallback(
    (kind: StarEntityKind, keys: readonly string[]) => {
      // Only ask about keys we don't already know — the IN-list stays bounded by
      // the newly-visible rows, not the whole window each render.
      const pending = Array.from(
        new Set(
          keys.filter(
            (key) =>
              key.length > 0 && !knownRef.current.has(cacheKey(kind, key)),
          ),
        ),
      )
      if (pending.length === 0) return
      // Mark as known up-front so overlapping renders don't double-fetch.
      for (const key of pending) knownRef.current.add(cacheKey(kind, key))
      backend
        .getStarStatus({ entityKind: kind, entityKeys: pending })
        .then((statuses) => {
          setCache((prev) => {
            const next = { ...prev }
            for (const key of pending) {
              next[cacheKey(kind, key)] = statuses[key] ?? false
            }
            return next
          })
        })
        .catch((error: unknown) => {
          // Allow a later render to retry the failed keys.
          for (const key of pending)
            knownRef.current.delete(cacheKey(kind, key))
          setLastError(formatError(error, 'hydrate'))
        })
    },
    [],
  )

  const toggle = useCallback(
    (kind: StarEntityKind, key: string) => {
      if (!key) return
      const composite = cacheKey(kind, key)
      const previous = cache[composite] ?? false
      const nextValue = !previous
      knownRef.current.add(composite)
      setCache((prev) => ({ ...prev, [composite]: nextValue }))
      const request = { entityKind: kind, entityKey: key }
      const write = nextValue
        ? backend.setStar(request)
        : backend.unsetStar(request)
      write.then(
        () => setLastError(null),
        (error: unknown) => {
          // Roll back the optimistic flip so the UI matches the archive.
          setCache((prev) => ({ ...prev, [composite]: previous }))
          setLastError(formatError(error, 'toggle'))
        },
      )
    },
    [cache],
  )

  return useMemo(
    () => ({ isStarred, hydrate, toggle, lastError }),
    [isStarred, hydrate, toggle, lastError],
  )
}

function formatError(error: unknown, scope: 'hydrate' | 'toggle'): string {
  return `${scope}: ${describeError(error, scope)}`
}
