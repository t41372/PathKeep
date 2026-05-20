/**
 * Backend-backed annotations hook for the paper detail panel.
 *
 * When the desktop command transport is live, the Browse detail panel writes
 * notes and tags through to `vault-core::annotations` (migration 011 +
 * commands/annotations.rs) instead of localStorage. The hook mirrors the
 * `LocalAnnotations` shape so the route can pick between the two backings
 * without changing the panel.
 *
 * ## Responsibilities
 * - Keep an in-memory cache of `{ notes, tags }` keyed by URL.
 * - Lazily hydrate the cache from the backend the first time a URL is
 *   requested via `notesFor` or `tagsFor`.
 * - Optimistically apply local mutations, then write through to the
 *   `set_url_notes` / `replace_url_tags` commands; on failure the cache
 *   stays optimistic so the user keeps their typed-but-unsaved text.
 *
 * ## Not responsible for
 * - Conflict resolution if two sessions write the same URL — last write
 *   wins, same as the local hook.
 * - Surfacing transport errors to the user; the panel renders a "Saved ·
 *   local" pill which is honest enough for prototype use.
 *
 * ## Performance notes
 * - Hydration is per-URL on-demand. The detail panel renders one URL at a
 *   time, so the hook never fans out across the full archive.
 */

import { useCallback, useMemo, useRef, useState } from 'react'
import { backend } from '../../lib/backend-client'
import type { LocalAnnotations } from './use-local-annotations'

interface Bundle {
  notes: string
  tags: string[]
}

const EMPTY_BUNDLE: Bundle = { notes: '', tags: [] }

export function useDesktopAnnotations(): LocalAnnotations {
  const [cache, setCache] = useState<Record<string, Bundle>>({})
  const hydratedRef = useRef<Set<string>>(new Set())
  // Tracks URLs the user has mutated locally. Hydration responses for
  // these URLs are discarded, so an in-flight `get_url_annotation` cannot
  // overwrite a `set_url_notes` write the user just made. Without this,
  // typing while the GET is in flight would lose data.
  const locallyMutatedRef = useRef<Set<string>>(new Set())
  // Latest async error from a write or hydration call. Surfaced via the
  // `lastError` accessor so the panel (or a future toast) can show the
  // user that their edit did not reach the archive, instead of silently
  // pretending it did.
  const [lastError, setLastError] = useState<string | null>(null)

  const ensureHydrated = useCallback((url: string) => {
    if (hydratedRef.current.has(url)) return
    hydratedRef.current.add(url)
    backend
      .getUrlAnnotation(url)
      .then((annotation) => {
        if (!annotation) return
        // If the user has typed since we kicked off the GET, the backend
        // payload is stale relative to the optimistic value already in
        // the cache — drop it to avoid the hydration race.
        if (locallyMutatedRef.current.has(url)) return
        setCache((prev) => ({
          ...prev,
          [url]: {
            notes: annotation.notes,
            tags: annotation.tags ?? [],
          },
        }))
      })
      .catch((error: unknown) => {
        hydratedRef.current.delete(url)
        setLastError(formatError(error, 'hydrate'))
      })
  }, [])

  const notesFor = useCallback(
    (key: string | null | undefined) => {
      if (!key) return ''
      ensureHydrated(key)
      return cache[key]?.notes ?? ''
    },
    [cache, ensureHydrated],
  )
  const tagsFor = useCallback(
    (key: string | null | undefined) => {
      if (!key) return []
      ensureHydrated(key)
      return cache[key]?.tags ?? EMPTY_BUNDLE.tags
    },
    [cache, ensureHydrated],
  )

  const updateNotes = useCallback((key: string, next: string) => {
    locallyMutatedRef.current.add(key)
    setCache((prev) => ({
      ...prev,
      [key]: { notes: next, tags: prev[key]?.tags ?? [] },
    }))
    backend.setUrlNotes({ url: key, notes: next }).then(
      () => setLastError(null),
      (error: unknown) => {
        setLastError(formatError(error, 'notes'))
      },
    )
  }, [])

  const updateTags = useCallback((key: string, next: string[]) => {
    locallyMutatedRef.current.add(key)
    setCache((prev) => ({
      ...prev,
      [key]: { notes: prev[key]?.notes ?? '', tags: next },
    }))
    backend.replaceUrlTags({ url: key, tags: next }).then(
      () => setLastError(null),
      (error: unknown) => {
        setLastError(formatError(error, 'tags'))
      },
    )
  }, [])

  return useMemo(
    () => ({ notesFor, tagsFor, updateNotes, updateTags, lastError }),
    [notesFor, tagsFor, updateNotes, updateTags, lastError],
  )
}

function formatError(
  error: unknown,
  scope: 'hydrate' | 'notes' | 'tags',
): string {
  const reason = error instanceof Error ? error.message : String(error)
  return `${scope}: ${reason}`
}
