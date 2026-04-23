/**
 * @file use-explorer-favicons.ts
 * @description Lazy favicon hydration owner for Explorer time-view rows.
 * @module pages/explorer
 *
 * ## Responsibilities
 * - Keep the main Explorer history query free of favicon payload bytes.
 * - Batch-load missing favicon payloads only after a page of rows is already visible.
 * - Cache resolved favicon lookups for the current Explorer data epoch.
 *
 * ## Not responsible for
 * - Running the main history query or owning page-level pagination state.
 * - Rendering favicon placeholders or image elements.
 * - Persisting favicon caches across archive refreshes or app restarts.
 *
 * ## Dependencies
 * - Depends on the typed `backend.loadHistoryFavicons()` desktop command surface.
 * - Reuses Explorer helper keys so row and cache identity stay aligned.
 *
 * ## Performance notes
 * - This hook intentionally loads icons after first paint, and only for the
 *   visible page, so favicon bytes no longer dominate initial page query size.
 */

import { useEffect, useMemo, useRef, useState } from 'react'
import { backend } from '../../../lib/backend-client'
import type { HistoryEntry, HistoryQueryResponse } from '../../../lib/types'
import { historyFaviconLookupKey } from '../helpers'

interface UseExplorerFaviconsOptions {
  cacheToken: number
  loading: boolean
  results: HistoryQueryResponse | null
}

/**
 * Loads Explorer row favicons after the page content is already visible.
 *
 * This preserves the skeleton-first page reveal while still restoring stored
 * icons once the visible row window has settled.
 */
export function useExplorerFavicons({
  cacheToken,
  loading,
  results,
}: UseExplorerFaviconsOptions) {
  const [cacheState, setCacheState] = useState(() => ({
    token: cacheToken,
    entries: new Map<string, HistoryEntry['favicon'] | null>(),
  }))
  const inflightKeysRef = useRef(new Set<string>())
  const emptyCache = useMemo(
    () => new Map<string, HistoryEntry['favicon'] | null>(),
    [],
  )
  const faviconCache =
    cacheState.token === cacheToken ? cacheState.entries : emptyCache

  useEffect(() => {
    inflightKeysRef.current.clear()
  }, [cacheToken])

  const visibleEntries = useMemo(() => {
    if (!results?.items.length) {
      return [] as { cacheKey: string; profileId: string; url: string }[]
    }

    const seen = new Set<string>()
    const dedupedEntries: {
      cacheKey: string
      profileId: string
      url: string
    }[] = []
    for (const item of results.items) {
      const cacheKey = historyFaviconLookupKey(item.profileId, item.url)
      if (seen.has(cacheKey)) {
        continue
      }
      seen.add(cacheKey)
      dedupedEntries.push({
        cacheKey,
        profileId: item.profileId,
        url: item.url,
      })
    }
    return dedupedEntries
  }, [results])

  useEffect(() => {
    if (loading || visibleEntries.length === 0) {
      return
    }

    const missingEntries = visibleEntries.filter(
      (entry) =>
        !faviconCache.has(entry.cacheKey) &&
        !inflightKeysRef.current.has(entry.cacheKey),
    )
    if (missingEntries.length === 0) {
      return
    }

    let cancelled = false
    missingEntries.forEach((entry) =>
      inflightKeysRef.current.add(entry.cacheKey),
    )

    void backend
      .loadHistoryFavicons(
        missingEntries.map((entry) => ({
          profileId: entry.profileId,
          url: entry.url,
        })),
      )
      .then((loadedFavicons) => {
        if (cancelled) {
          return
        }

        setCacheState((current) => {
          const baseEntries =
            current.token === cacheToken
              ? current.entries
              : new Map<string, HistoryEntry['favicon'] | null>()
          const next = new Map(baseEntries)
          for (const entry of missingEntries) {
            next.set(entry.cacheKey, null)
          }
          for (const faviconEntry of loadedFavicons) {
            next.set(
              historyFaviconLookupKey(faviconEntry.profileId, faviconEntry.url),
              faviconEntry.favicon ?? null,
            )
          }
          return {
            token: cacheToken,
            entries: next,
          }
        })
      })
      .finally(() => {
        for (const entry of missingEntries) {
          inflightKeysRef.current.delete(entry.cacheKey)
        }
      })

    return () => {
      cancelled = true
    }
  }, [cacheToken, faviconCache, loading, visibleEntries])

  return {
    faviconCache,
  }
}
