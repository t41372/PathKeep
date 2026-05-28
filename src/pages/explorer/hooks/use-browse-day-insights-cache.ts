/**
 * Per-day Browse insights fetcher + cache.
 *
 * ## Why this hook exists
 * The paper Browse contact sheet's day-insights strip (top domains,
 * top URLs, 24-hour sparkline, activity tallies, session stats) used
 * to be aggregated client-side from whatever cards the user had
 * already scrolled into view (`aggregateDayInsights(day)` in
 * `paper-day-insights-helpers.ts`). That meant a partially-loaded day
 * silently rendered a partially-empty sparkline + half-correct
 * top-domains list — which feedback-2026-05-25 §3.1 flagged as a
 * Trust & Transparency violation: the panel made it look like the day
 * had less activity than it actually did.
 *
 * This hook moves the aggregation to the backend. It fetches
 * `BrowseDayInsights` for each visible day via
 * `backend.getBrowseDayInsights`, caches the result per
 * `(profileId, date)` for the lifetime of the current refresh cycle,
 * and exposes a `resolve(date)` lookup that the contact sheet uses to
 * override `aggregateDayInsights(day)` when the backend reply has
 * landed. Until the reply lands, the client-side aggregator continues
 * to render so the panel never blinks empty.
 *
 * ## Caching contract
 * - The cache is keyed by `(profileId ?? '*all*', date)`. When the
 *   route's `refreshKey` changes (manual backup, import, etc.), the
 *   cache resets so stale aggregates don't outlast their archive
 *   state. When `profileId` changes, the cache also resets.
 * - In-flight requests are deduped by the same key, so two adjacent
 *   re-renders that both call `request(date)` only fire one backend
 *   call.
 * - Errors are remembered too, so a missing-archive failure doesn't
 *   spam the backend on every scroll tick.
 */

import { useCallback, useMemo, useState } from 'react'
import { backend } from '@/lib/backend-client'
import type { BrowseDayInsights } from '@/lib/backend-client/explorer'
import type { DayInsights } from '@/components/explorer-paper/paper-day-insights-helpers'

interface CachedEntry {
  state: 'pending' | 'ready' | 'error'
  insights?: DayInsights
}

export interface BrowseDayInsightsCache {
  /**
   * Returns the most recent backend-aggregated insights for `date`, or
   * `null` if the backend reply has not landed yet (in which case the
   * contact sheet falls back to its client-side aggregator). Calling
   * `resolve` for a date the cache has not seen before will also fire
   * the backend fetch as a side effect, so the contact sheet can ask
   * for insights from inside its day render without the route having
   * to enumerate visible days separately. Subsequent calls during the
   * same refresh cycle are no-ops thanks to the in-flight dedup.
   */
  resolve: (date: string) => DayInsights | null
}

export interface BrowseDayInsightsCacheOptions {
  /** Profile filter, or `null` for archive-wide aggregation. */
  profileId?: string | null
  /**
   * Route-level cache token. Bumping this clears every cached entry —
   * the route owns when the cache should evict (e.g. after a manual
   * backup, a rekey, an import revert).
   */
  refreshKey: number
}

/**
 * Strips the backend-only `date` field off the wire shape, leaving a
 * `DayInsights`-compatible payload the existing PaperDayInsights
 * component already understands.
 */
function adaptInsights(raw: BrowseDayInsights): DayInsights {
  return {
    totalPages: raw.totalPages,
    typedCount: raw.typedCount,
    linkCount: raw.linkCount,
    searchCount: raw.searchCount,
    distinctDomains: raw.distinctDomains,
    sessionCount: raw.sessionCount,
    topDomains: raw.topDomains,
    hourBuckets: raw.hourBuckets,
    hourPeak: raw.hourPeak,
    firstVisitMs: raw.firstVisitMs,
    lastVisitMs: raw.lastVisitMs,
    peakHour: raw.peakHour,
    longestSessionMs: raw.longestSessionMs,
    topUrls: raw.topUrls,
    topSearchQueries: raw.topSearchQueries,
  }
}

export function useBrowseDayInsightsCache(
  options: BrowseDayInsightsCacheOptions,
): BrowseDayInsightsCache {
  const profileKey = options.profileId ?? '*all*'
  const token = `${options.refreshKey}::${profileKey}`
  // Single state holder keyed by token. When `token` changes (refresh
  // bump or profile switch) we mint a fresh state object inline — the
  // canonical React "derived state from props" pattern. The render
  // that detects the change schedules an immediate re-render, so any
  // `resolve(date)` calls from the same render observe the empty
  // cache.
  const [cacheState, setCacheState] = useState<{
    token: string
    cache: Map<string, CachedEntry>
    version: number
  }>(() => ({ token, cache: new Map(), version: 0 }))
  if (cacheState.token !== token) {
    setCacheState({ token, cache: new Map(), version: 0 })
  }

  const request = useCallback(
    (date: string) => {
      // Mutate the in-place Map without bumping `version` — pending
      // marker only needs to dedupe future requests during this
      // render cycle; we'll bump `version` once the backend reply
      // lands so consumers re-resolve and see the new payload.
      cacheState.cache.set(date, { state: 'pending' })
      backend
        .getBrowseDayInsights({
          date,
          profileId: options.profileId ?? null,
        })
        .then((insights) => {
          setCacheState((current) => {
            // Discard the reply if the route's token rotated mid-flight.
            if (current.token !== token) return current
            const nextCache = new Map(current.cache)
            nextCache.set(date, {
              state: 'ready',
              insights: adaptInsights(insights),
            })
            return {
              ...current,
              cache: nextCache,
              version: current.version + 1,
            }
          })
        })
        .catch(() => {
          setCacheState((current) => {
            if (current.token !== token) return current
            const nextCache = new Map(current.cache)
            nextCache.set(date, { state: 'error' })
            // Don't bump `version` — `resolve` keeps returning null
            // for error entries so the contact sheet's client-side
            // fallback aggregator keeps rendering. Mutating the Map
            // shape via clone is still required so future requests
            // for the same date dedupe correctly.
            return { ...current, cache: nextCache }
          })
        })
    },
    [cacheState.cache, options.profileId, token],
  )

  const resolve = useCallback(
    (date: string): DayInsights | null => {
      const entry = cacheState.cache.get(date)
      if (!entry) {
        // First time we've heard of this date in the current refresh
        // cycle — trigger the backend fetch as a side effect so the
        // contact sheet does not have to call a separate `request`.
        request(date)
        return null
      }
      if (entry.state === 'ready' && entry.insights) {
        return entry.insights
      }
      return null
    },
    [cacheState, request],
  )

  return useMemo(() => ({ resolve }), [resolve])
}
