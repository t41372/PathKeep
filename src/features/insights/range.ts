/**
 * What every Insights screen is scoped to: a time range, all time unless the
 * user picks another, and optionally one browser profile. It lives in the URL
 * (`?range=`, `?start=`/`?end=` for a custom range, `?profile=`) so a drill-in
 * opens on what the user was looking at, a reload keeps it, and a link can be
 * shared.
 *
 * Responsible for: the ranges, reading and writing the params, the dates a
 * scope covers. Not responsible for: fetching (queries take a scope), the
 * controls (`ScopeControls` in `scope-controls.tsx`).
 */
import { useCallback, useMemo } from 'react'
import { useSearchParams } from 'react-router-dom'
import { localDateKey } from '@/lib/backend-client/insights'
import type { DateRange } from '@/lib/core-intelligence/types'

/** The ranges in the toggle, longest first; all time is the default. */
export const presets = ['all', 'y1', 'd90', 'd30', 'd7'] as const
export type Preset = (typeof presets)[number]
export type RangeChoice = Preset | 'custom'
export const defaultRange: Preset = 'all'

const presetDays: Record<Exclude<Preset, 'all'>, number> = {
  y1: 365,
  d90: 90,
  d30: 30,
  d7: 7,
}

/**
 * Where an all-time range starts. The backend recognizes it and serves the
 * overviews from the snapshot it keeps after each Insights build
 * (`ALL_TIME_SCOPE_START` in `intelligence_overview_snapshot.rs`).
 */
export const ALL_TIME_START = '1900-01-01'

export interface Scope {
  range: RangeChoice
  /** The local days covered; all time runs from `ALL_TIME_START` to today. */
  dates: DateRange
  /** One browser profile, or every profile. */
  profileId: string | null
}

const dateKey = /^\d{4}-\d{2}-\d{2}$/

function isDateKey(value: string | null): value is string {
  return (
    value !== null &&
    dateKey.test(value) &&
    localDateKey(new Date(`${value}T00:00:00`)) === value
  )
}

export function isPreset(value: string | null): value is Preset {
  return (presets as readonly (string | null)[]).includes(value)
}

/** The days a preset covers, ending today. */
export function presetDates(range: Preset, today = new Date()): DateRange {
  const end = localDateKey(today)
  if (range === 'all') return { start: ALL_TIME_START, end }
  const start = new Date(today)
  start.setDate(start.getDate() - (presetDays[range] - 1))
  return { start: localDateKey(start), end }
}

/** A preset over every profile, for links from outside Insights. */
export function presetScope(range: Preset): Scope {
  return { range, dates: presetDates(range), profileId: null }
}

/**
 * The scope a URL asks for. A custom range needs a valid start no later than
 * its end and no later than today; anything else falls back to all time.
 */
export function scopeFromParams(
  params: URLSearchParams,
  today = new Date(),
): Scope {
  const profileId = params.get('profile') || null
  const requested = params.get('range')
  if (requested === 'custom') {
    const start = params.get('start')
    const end = params.get('end')
    if (
      isDateKey(start) &&
      isDateKey(end) &&
      start <= end &&
      end <= localDateKey(today)
    ) {
      return { range: 'custom', dates: { start, end }, profileId }
    }
  }
  const range = isPreset(requested) ? requested : defaultRange
  return { range, dates: presetDates(range, today), profileId }
}

/** The URL params for a scope; the default range is left out. */
export function scopeParams(scope: Scope, into = new URLSearchParams()) {
  for (const name of ['range', 'start', 'end', 'profile']) into.delete(name)
  if (scope.range !== defaultRange) into.set('range', scope.range)
  if (scope.range === 'custom') {
    into.set('start', scope.dates.start)
    into.set('end', scope.dates.end)
  }
  if (scope.profileId) into.set('profile', scope.profileId)
  return into
}

/** What tells two scopes' results apart, for query keys. */
export function scopeKey(scope: Scope) {
  return [scope.dates.start, scope.dates.end, scope.profileId] as const
}

/** How many days the scope covers; all time has no fixed length. */
export function scopeDays(scope: Scope): number | null {
  if (scope.range === 'all') return null
  const start = new Date(`${scope.dates.start}T00:00:00`)
  const end = new Date(`${scope.dates.end}T00:00:00`)
  return Math.round((end.getTime() - start.getTime()) / 86_400_000) + 1
}

export interface ScopeChange {
  range?: RangeChoice
  /** For a custom range. */
  dates?: DateRange
  profileId?: string | null
}

export function useScope() {
  const [params, setParams] = useSearchParams()
  const scope = useMemo(() => scopeFromParams(params), [params])
  const setScope = useCallback(
    (change: ScopeChange) =>
      setParams(
        (previous) => {
          const current = scopeFromParams(previous)
          const range = change.range ?? current.range
          const next: Scope = {
            range,
            dates:
              range === 'custom'
                ? (change.dates ?? current.dates)
                : presetDates(range),
            profileId:
              change.profileId === undefined
                ? current.profileId
                : change.profileId,
          }
          return scopeParams(next, new URLSearchParams(previous))
        },
        { replace: true },
      ),
    [setParams],
  )
  return [scope, setScope] as const
}
