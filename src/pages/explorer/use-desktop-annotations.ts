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

  const ensureHydrated = useCallback((url: string) => {
    if (hydratedRef.current.has(url)) return
    hydratedRef.current.add(url)
    backend
      .getUrlAnnotation(url)
      .then((annotation) => {
        if (!annotation) return
        setCache((prev) => ({
          ...prev,
          [url]: {
            notes: annotation.notes,
            tags: annotation.tags ?? [],
          },
        }))
      })
      .catch(() => {
        // Hydration errors leave the cache empty; the user keeps writing
        // optimistically and the next mutation tries the backend again.
        hydratedRef.current.delete(url)
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
    setCache((prev) => ({
      ...prev,
      [key]: { notes: next, tags: prev[key]?.tags ?? [] },
    }))
    backend.setUrlNotes({ url: key, notes: next }).catch(() => {
      // The cache already holds the optimistic value; users can retry by
      // editing again. A future pass can surface a toast or status pill.
    })
  }, [])

  const updateTags = useCallback((key: string, next: string[]) => {
    setCache((prev) => ({
      ...prev,
      [key]: { notes: prev[key]?.notes ?? '', tags: next },
    }))
    backend.replaceUrlTags({ url: key, tags: next }).catch(() => {
      // Same optimistic policy as updateNotes — keep the cache value.
    })
  }, [])

  return useMemo(
    () => ({ notesFor, tagsFor, updateNotes, updateTags }),
    [notesFor, tagsFor, updateNotes, updateTags],
  )
}
