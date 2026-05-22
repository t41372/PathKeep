/**
 * @file use-explorer-archive-density.ts
 * @description Loads archive-wide per-day + per-year visit density for the
 * paper Browse calendar, year rail, and day-nav so they reflect the full
 * archive instead of only the rows on the currently-loaded history page.
 * @module pages/explorer/hooks
 *
 * ## Responsibilities
 * - Query `getDiscoveryTrend` once per profile scope with a wide range so
 *   the discovery_trend_day rollups for every archived year flow through.
 * - Aggregate the per-day rollups into `Map<YYYY-MM-DD, count>` and
 *   `Map<year, count>` shapes the paper view expects via `additionalDensity`.
 * - Derive the archive bounds (firstYear / lastYear / totalDays) from the
 *   `availableYears` field on the same response, so the year rail and
 *   calendar know what range to render.
 *
 * ## Not responsible for
 * - Fetching the visible history rows. That stays with `useExplorerData`.
 * - Authoritative day-by-day visit lists. The rollups only carry totals;
 *   for actual entries the user still pages through the history query.
 *
 * ## Why this hook exists
 * Without backend-side density, the calendar dot + per-year mini-map
 * tint were derived from `groupEntriesByDay(entries)` — i.e. only the
 * days that appeared on page 1 of the active query. A 12-month archive
 * that loaded page 1 = today's visits would therefore show "no activity"
 * on every other day. This hook layers the real rollups on top so the
 * calendar popover is honest about what's archived even before the user
 * scrolls deeper into history.
 *
 * ## Performance notes
 * - The discovery-trend cache in `coreIntelligenceApi` dedupes identical
 *   requests so navigating between routes that read the same range is a
 *   no-op fetch.
 * - The range is clamped to roughly 20 years to keep the response size
 *   bounded for hypothetical migration imports older than that — even
 *   on a 1440 M-row archive, that's at most ~7300 daily rollup rows.
 */

import { useEffect, useMemo, useRef, useState } from 'react'
import { getDiscoveryTrend } from '@/lib/core-intelligence'
import { isoDateOnly } from '@/pages/dashboard/dashboard-helpers'

export interface ExplorerArchiveDensity {
  /** Per-day total visits aggregated across all profiles or the scoped profile. */
  perDay: ReadonlyMap<string, number>
  /** Per-year total visits aggregated from the daily rollups. */
  perYear: ReadonlyMap<number, number>
  /** Bounds derived from `availableYears` — null while the first fetch is in flight. */
  bounds: {
    firstIso: string
    lastIso: string
    firstYear: number
    lastYear: number
    totalDays: number
  } | null
}

const EMPTY_PER_DAY: ReadonlyMap<string, number> = new Map()
const EMPTY_PER_YEAR: ReadonlyMap<number, number> = new Map()

/**
 * Maximum span the hook will request in years. A user with archives
 * stretching back to ~1995 still fits comfortably; anything beyond that
 * is treated as out-of-scope for the rollup query. The Rust side trims
 * to whatever `daily_summary_rollups` actually has, so this is just a
 * safety cap, not a hard archive limit.
 */
const ARCHIVE_DENSITY_YEARS = 20

function archiveDensityRange(now: Date): { start: string; end: string } {
  const end = new Date(now)
  end.setHours(0, 0, 0, 0)
  const start = new Date(end)
  start.setFullYear(start.getFullYear() - ARCHIVE_DENSITY_YEARS)
  return { start: isoDateOnly(start), end: isoDateOnly(end) }
}

interface UseExplorerArchiveDensityOptions {
  archiveReady: boolean
  profileId: string | null
  /** Bumped whenever the route wants to force a re-fetch (e.g. after import). */
  refreshKey?: number
}

/**
 * Hook that exposes archive-wide density + bounds for the paper Browse view.
 *
 * Returns `EMPTY_PER_DAY` / `EMPTY_PER_YEAR` and `bounds=null` until the
 * first fetch resolves so callers can fall back to per-entry density
 * without a flash of empty state.
 */
export function useExplorerArchiveDensity({
  archiveReady,
  profileId,
  refreshKey = 0,
}: UseExplorerArchiveDensityOptions): ExplorerArchiveDensity {
  const [density, setDensity] = useState<ExplorerArchiveDensity>({
    perDay: EMPTY_PER_DAY,
    perYear: EMPTY_PER_YEAR,
    bounds: null,
  })
  // The mounted flag stops a late discovery-trend response from clobbering
  // a more recent fetch (or writing to state after the route unmounts).
  const requestSeqRef = useRef(0)

  useEffect(() => {
    if (!archiveReady) {
      // Walk a microtask before resetting so the effect doesn't write
      // synchronously during commit (`react-hooks/set-state-in-effect`).
      // The empty fallback is idempotent so the deferral is invisible.
      let cancelledReset = false
      queueMicrotask(() => {
        if (cancelledReset) return
        setDensity({
          perDay: EMPTY_PER_DAY,
          perYear: EMPTY_PER_YEAR,
          bounds: null,
        })
      })
      return () => {
        cancelledReset = true
      }
    }
    const sequence = ++requestSeqRef.current
    const range = archiveDensityRange(new Date())
    let cancelled = false
    void getDiscoveryTrend(range, profileId, 'day')
      .then((envelope) => {
        if (cancelled || requestSeqRef.current !== sequence) return
        // `getDiscoveryTrend` wraps the payload in a section envelope; the
        // density rollups live on `.data`. Missing-data states surface
        // `.data` as a default-constructed shape with empty points + years.
        const payload = envelope.data
        const perDay = new Map<string, number>()
        const perYear = new Map<number, number>()
        for (const point of payload.points) {
          if (!point.dateKey) continue
          perDay.set(point.dateKey, point.totalVisits)
          const year = Number.parseInt(point.dateKey.slice(0, 4), 10)
          if (Number.isFinite(year)) {
            perYear.set(year, (perYear.get(year) ?? 0) + point.totalVisits)
          }
        }
        // Prefer the backend-provided availableYears for bounds — it covers
        // years that have rollups even outside the requested range, so the
        // year rail still shows "first" correctly on very old archives.
        const yearsFromBackend = payload.availableYears.filter((year: number) =>
          Number.isFinite(year),
        )
        const allYears = new Set<number>([
          ...yearsFromBackend,
          ...perYear.keys(),
        ])
        const sortedYears = [...allYears].sort((a, b) => a - b)
        const bounds = sortedYears.length
          ? buildBounds(sortedYears, perDay)
          : null
        setDensity({ perDay, perYear, bounds })
      })
      .catch(() => {
        // Density is best-effort — if the backend rejects (locked archive,
        // empty rollup table, transient error) we leave the cache in its
        // pre-fetch state so the route falls back to per-entry density.
        if (cancelled || requestSeqRef.current !== sequence) return
        setDensity({
          perDay: EMPTY_PER_DAY,
          perYear: EMPTY_PER_YEAR,
          bounds: null,
        })
      })
    return () => {
      cancelled = true
    }
  }, [archiveReady, profileId, refreshKey])

  return useMemo(() => density, [density])
}

function buildBounds(
  sortedYears: number[],
  perDay: ReadonlyMap<string, number>,
): ExplorerArchiveDensity['bounds'] {
  const firstYear = sortedYears[0]
  const lastYear = sortedYears[sortedYears.length - 1]
  // Walk the real data once and pick the actual earliest / latest dates
  // that have a rollup row. The previous tighten-toward-edges loop never
  // assigned anything because its `dateKey < firstIso` / `dateKey > lastIso`
  // gate was never satisfied against the Jan-1 / Dec-31 seed values, which
  // is why clicking the topmost year on a partial-year archive used to
  // jump to Dec 31 of a future year and trigger the empty state.
  let earliest: string | null = null
  let latest: string | null = null
  for (const dateKey of perDay.keys()) {
    if (earliest === null || dateKey < earliest) earliest = dateKey
    if (latest === null || dateKey > latest) latest = dateKey
  }
  const firstIso = earliest ?? `${firstYear.toString().padStart(4, '0')}-01-01`
  const lastIso = latest ?? `${lastYear.toString().padStart(4, '0')}-12-31`
  const totalDays =
    Math.max(
      1,
      Math.round(
        (new Date(lastIso).getTime() - new Date(firstIso).getTime()) /
          86_400_000,
      ),
    ) + 1
  return { firstIso, lastIso, firstYear, lastYear, totalDays }
}
