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
 * ## Request/read split (why `resolve` is pure)
 * `resolve` used to fire `request(date)` on a cache miss so the contact
 * sheet could ask for insights from inside its day render. That made the
 * render phase issue IPC and mutate a state `Map` in place — an
 * AGENTS.md "no heavy work / no side effects on the render path"
 * violation, and it made React's render phase non-idempotent (a
 * discarded concurrent render still sent the command).
 *
 * The two halves are now separate:
 * - `request(date)` is a side-effecting command. Callers invoke it from
 *   an effect — the contact sheet reports each day block as it mounts
 *   into the viewport, so the fan-out is bounded by what is on screen,
 *   not by the accumulated infinite-scroll day list.
 * - `resolve(date)` is a pure lookup. It never fetches and never
 *   mutates; a miss simply returns `null` and the caller falls back to
 *   its (memoised) client-side aggregator.
 *
 * ## Caching contract
 * - The cache is keyed by `(profileId ?? '*all*', date)`. When the
 *   route's `refreshKey` changes (manual backup, import, etc.), the
 *   cache resets so stale aggregates don't outlast their archive
 *   state. When `profileId` changes, the cache also resets. `request`
 *   is re-created on that rotation, which is what re-arms the callers'
 *   effects.
 * - In-flight requests are deduped by the same key, so two adjacent
 *   day mounts that both call `request(date)` only fire one backend
 *   call.
 * - Failures are retried a bounded number of times (`MAX_ATTEMPTS`)
 *   across subsequent mounts of the same day. A permanent failure
 *   settles into the error state and the caller keeps its client-side
 *   fallback; a transient one (archive briefly locked mid-import) is no
 *   longer a one-shot that pins the day to client aggregation for the
 *   rest of the refresh cycle.
 */

import { useCallback, useMemo, useRef, useState } from 'react'
import { backend } from '@/lib/backend-client'
import type { BrowseDayInsights } from '@/lib/backend-client/explorer'
import type { DayInsights } from '@/components/explorer-paper/paper-day-insights-helpers'

/**
 * How many times a single `(token, date)` may be re-attempted after a
 * failure. Bounded so a systematically failing backend (locked archive,
 * missing profile) settles instead of re-issuing a command every time the
 * user scrolls the day back into view.
 */
const MAX_ATTEMPTS = 3

/**
 * A cache slot. Modelled as a discriminated union so `insights` exists exactly
 * when the state says it does — `resolve` then has no "ready but empty" case
 * to defend against.
 */
type CachedEntry = { attempts: number } & (
  | { state: 'pending' }
  | { state: 'ready'; insights: DayInsights }
  | { state: 'error' }
)

interface CacheBox {
  token: string
  entries: Map<string, CachedEntry>
}

export interface BrowseDayInsightsCache {
  /**
   * Pure lookup. Returns the backend-aggregated insights for `date`, or
   * `null` when nothing has landed (not requested yet, still in flight,
   * or failed) — in which case the caller renders its own client-side
   * aggregate. Never fetches and never mutates, so it is safe to call
   * from render.
   */
  resolve: (date: string) => DayInsights | null
  /**
   * Side-effecting command: ensure `date` is being fetched. Call from an
   * effect, never from render. Idempotent per `(token, date)` apart from
   * the bounded error retry. Its identity changes only when the cache
   * token rotates, so it is a safe effect dependency.
   */
  request: (date: string) => void
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
  const profileId = options.profileId ?? null
  // The cache lives in a ref, not in state, because `request` must be able to
  // read and write it from an effect without the write becoming a new state
  // object that re-arms every caller's effect. `version` exists only to
  // schedule the re-render that lets consumers observe a landed reply.
  const boxRef = useRef<CacheBox>({ token, entries: new Map() })
  const [version, setVersion] = useState(0)

  /**
   * Returns the cache box for the *current* token, rotating a stale one.
   * Only ever called from `request` (i.e. from a caller's effect), never
   * during render — rotating in render would mutate a ref from a phase React
   * is allowed to discard.
   */
  const takeBox = useCallback((): CacheBox => {
    if (boxRef.current.token !== token) {
      boxRef.current = { token, entries: new Map() }
    }
    return boxRef.current
  }, [token])

  const request = useCallback(
    (date: string) => {
      const box = takeBox()
      const existing = box.entries.get(date)
      if (existing) {
        // Pending / ready are terminal for request purposes. Errors get a
        // bounded number of further attempts.
        if (existing.state !== 'error') return
        if (existing.attempts >= MAX_ATTEMPTS) return
      }
      const attempts = (existing?.attempts ?? 0) + 1
      box.entries.set(date, { state: 'pending', attempts })
      backend
        .getBrowseDayInsights({ date, profileId })
        .then((insights) => {
          // Discard the reply if the route's token rotated mid-flight.
          if (boxRef.current !== box) return
          box.entries.set(date, {
            state: 'ready',
            insights: adaptInsights(insights),
            attempts,
          })
          setVersion((current) => current + 1)
        })
        .catch(() => {
          if (boxRef.current !== box) return
          box.entries.set(date, { state: 'error', attempts })
          // Bump anyway: the strip has to re-read so an exhausted-retry day
          // settles onto the client-side fallback deterministically rather
          // than depending on some other render happening to come along.
          setVersion((current) => current + 1)
        })
    },
    [profileId, takeBox],
  )

  const resolve = useCallback(
    (date: string): DayInsights | null => {
      const box = boxRef.current
      // A stale box means the token rotated and nothing has re-requested yet;
      // report a miss rather than serving aggregates from the previous
      // archive state.
      if (box.token !== token) return null
      const entry = box.entries.get(date)
      return entry?.state === 'ready' ? entry.insights : null
    },
    [token],
  )

  // `version` is threaded through the returned identity on purpose. `resolve`
  // reads a mutable ref, so its own identity cannot signal "a reply landed";
  // consumers that memoise on the cache object need something that changes
  // when one does. `request` stays stable across replies (see its test) so a
  // landed reply never re-arms a caller's fetch effect.
  return useMemo(
    () => ({ resolve, request }),
    // eslint-disable-next-line react-hooks/exhaustive-deps -- see note above.
    [resolve, request, version],
  )
}
