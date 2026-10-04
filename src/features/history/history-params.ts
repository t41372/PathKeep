/**
 * The History screen's state as URL search params, so deep links work
 * (Home day links, the command palette) and a reload keeps the view.
 *
 * Responsible for: parsing and updating the params, the date filter value and
 * how a typed query resolves to a search mode.
 * Not responsible for: fetching anything.
 */
import { useCallback, useMemo } from 'react'
import { useSearchParams } from 'react-router-dom'
import { localDateKey } from '@/lib/backend-client/insights'

export type HistoryView = 'timeline' | 'sites' | 'starred'
export type SearchMode = 'full' | 'regex' | 'semantic'

export interface HistoryParams {
  q: string
  mode: SearchMode
  view: HistoryView
  /** `today`, `yesterday`, `7d`, `30d`, `YYYY-MM-DD` or `YYYY-MM-DD..YYYY-MM-DD`. */
  date: string | null
  /** Browser kind, the part of a profile id before the colon (`chrome`, `firefox`). */
  browser: string | null
  domain: string | null
  visit: number | null
}

const views: readonly HistoryView[] = ['timeline', 'sites', 'starred']
const modes: readonly SearchMode[] = ['full', 'regex', 'semantic']

function parse(search: URLSearchParams): HistoryParams {
  const view = search.get('view') as HistoryView
  const mode = search.get('mode') as SearchMode
  const visit = Number(search.get('visit'))
  return {
    q: search.get('q') ?? '',
    mode: modes.includes(mode) ? mode : 'full',
    view: views.includes(view) ? view : 'timeline',
    date: search.get('date') || null,
    browser: search.get('browser') || null,
    domain: search.get('domain') || null,
    visit: Number.isInteger(visit) && visit > 0 ? visit : null,
  }
}

const defaults: HistoryParams = {
  q: '',
  mode: 'full',
  view: 'timeline',
  date: null,
  browser: null,
  domain: null,
  visit: null,
}

export function useHistoryParams() {
  const [search, setSearch] = useSearchParams()
  const params = useMemo(() => parse(search), [search])
  const update = useCallback(
    (patch: Partial<HistoryParams>) => {
      setSearch(
        (previous) => {
          const next = new URLSearchParams(previous)
          for (const [name, value] of Object.entries(patch)) {
            if (
              value === defaults[name as keyof HistoryParams] ||
              value === ''
            ) {
              next.delete(name)
            } else next.set(name, String(value))
          }
          return next
        },
        { replace: true },
      )
    },
    [setSearch],
  )
  return { params, update }
}

export type DateFilter =
  | { kind: 'all' }
  | { kind: 'preset'; preset: 'today' | 'yesterday' | '7d' | '30d' }
  | { kind: 'range'; start: string; end: string }

const dayKey = /^\d{4}-\d{2}-\d{2}$/

export function parseDateFilter(value: string | null): DateFilter {
  if (!value) return { kind: 'all' }
  if (
    value === 'today' ||
    value === 'yesterday' ||
    value === '7d' ||
    value === '30d'
  ) {
    return { kind: 'preset', preset: value }
  }
  const [start, end = start] = value.split('..')
  if (dayKey.test(start) && dayKey.test(end)) {
    return start <= end
      ? { kind: 'range', start, end }
      : { kind: 'range', start: end, end: start }
  }
  return { kind: 'all' }
}

export function rangeParam(start: Date, end: Date) {
  const a = localDateKey(start)
  const b = localDateKey(end)
  return a === b ? a : `${a}..${b}`
}

export interface DayRange {
  start: string
  end: string
}

/** Local `YYYY-MM-DD` bounds of the filter, or null for all dates. */
export function dayRange(
  filter: DateFilter,
  now = new Date(),
): DayRange | null {
  const shift = (days: number) => {
    const day = new Date(now)
    day.setDate(day.getDate() - days)
    return localDateKey(day)
  }
  if (filter.kind === 'all') return null
  if (filter.kind === 'range') return { start: filter.start, end: filter.end }
  switch (filter.preset) {
    case 'today':
      return { start: shift(0), end: shift(0) }
    case 'yesterday':
      return { start: shift(1), end: shift(1) }
    case '7d':
      return { start: shift(6), end: shift(0) }
    case '30d':
      return { start: shift(29), end: shift(0) }
  }
}

export function parseDayKey(key: string) {
  const [year, month, day] = key.split('-').map(Number)
  return new Date(year, month - 1, day)
}

export interface TimeBounds {
  startTimeMs: number | null
  endTimeMs: number | null
}

/** Inclusive millisecond bounds covering whole local days. */
export function timeBounds(range: DayRange | null): TimeBounds {
  if (!range) return { startTimeMs: null, endTimeMs: null }
  const end = parseDayKey(range.end)
  end.setDate(end.getDate() + 1)
  return {
    startTimeMs: parseDayKey(range.start).getTime(),
    endTimeMs: end.getTime() - 1,
  }
}

export interface SearchSpec {
  /** What the user typed, trimmed. */
  text: string
  mode: SearchMode
  /** The text sent to the backend: the regex body when in regex mode. */
  pattern: string
  regexError: boolean
}

function validRegex(pattern: string) {
  try {
    new RegExp(pattern, 'i')
    return true
  } catch {
    return false
  }
}

/** A query wrapped in slashes is a regex whatever the mode toggle says. */
export function resolveSearch(
  q: string,
  mode: SearchMode,
  semanticAvailable: boolean,
): SearchSpec {
  const text = q.trim()
  const wrapped = /^\/(.+)\/$/s.exec(text)
  const effective: SearchMode = wrapped
    ? 'regex'
    : mode === 'semantic' && !semanticAvailable
      ? 'full'
      : mode
  const pattern = wrapped ? wrapped[1] : text
  return {
    text,
    mode: effective,
    pattern,
    regexError: Boolean(text) && effective === 'regex' && !validRegex(pattern),
  }
}
