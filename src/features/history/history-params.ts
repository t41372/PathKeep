/**
 * The History screen's state as URL search params, so deep links work
 * (Home day links, the command palette) and a reload keeps the view.
 *
 * Responsible for: parsing and updating the params, the date filter value and
 * how a typed query resolves to a search mode and order.
 * Not responsible for: fetching anything.
 */
import { useCallback, useMemo } from 'react'
import { useSearchParamsUpdate } from '@/lib/search-params'
import { localDateKey } from '@/lib/backend-client/insights'
import { checkRustRegex, type RegexProblem } from './regex-dialect'

export type HistoryView = 'timeline' | 'sites' | 'starred'
export type SearchMode = 'full' | 'regex' | 'semantic'
/** The orders the search engine can return results in (`SearchRequest.sort`). */
export type SearchSort = 'relevance' | 'newest' | 'oldest' | 'visits'

/**
 * The orders each search mode offers, its default first. A regex either
 * matches or not, so it has no best match. Semantic search ranks by meaning
 * only: its date orders would list keyword matches (`AiSearchSort`), which is
 * not what a semantic search asked for.
 */
const sortsByMode: Record<SearchMode, readonly SearchSort[]> = {
  full: ['relevance', 'newest', 'oldest', 'visits'],
  regex: ['newest', 'oldest', 'visits'],
  semantic: ['relevance'],
}

export const searchSorts: readonly SearchSort[] = [
  'relevance',
  'newest',
  'oldest',
  'visits',
]

export function sortsFor(mode: SearchMode) {
  return sortsByMode[mode]
}

export interface HistoryParams {
  q: string
  mode: SearchMode
  /** The order picked in the sort menu; null means the mode's default. */
  sort: SearchSort | null
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
  const sort = search.get('sort') as SearchSort
  const visit = Number(search.get('visit'))
  return {
    q: search.get('q') ?? '',
    mode: modes.includes(mode) ? mode : 'full',
    sort: searchSorts.includes(sort) ? sort : null,
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
  sort: null,
  view: 'timeline',
  date: null,
  browser: null,
  domain: null,
  visit: null,
}

export function useHistoryParams() {
  const [search, setSearch] = useSearchParamsUpdate()
  const params = useMemo(() => parse(search), [search])
  const update = useCallback(
    (patch: Partial<HistoryParams>) => {
      setSearch((next) => {
        for (const [name, value] of Object.entries(patch)) {
          if (value === defaults[name as keyof HistoryParams] || value === '') {
            next.delete(name)
          } else next.set(name, String(value))
        }
        return next
      })
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
  /** The order results come in: the one picked, when this mode offers it. */
  sort: SearchSort
  /** The text sent to the backend: the regex body when in regex mode. */
  pattern: string
  /** Why the backend's regex dialect (Rust) would refuse the pattern; the search is not sent. */
  regexError: RegexProblem | null
}

/**
 * A query wrapped in slashes is a regex whatever the mode toggle says. The
 * picked order holds in every mode that offers it, so switching modes and
 * back keeps it; a mode without it uses its own default.
 */
export function resolveSearch(
  q: string,
  mode: SearchMode,
  semanticAvailable: boolean,
  sort: SearchSort | null = null,
): SearchSpec {
  const text = q.trim()
  const wrapped = /^\/(.+)\/$/s.exec(text)
  const effective: SearchMode = wrapped
    ? 'regex'
    : mode === 'semantic' && !semanticAvailable
      ? 'full'
      : mode
  const pattern = wrapped ? wrapped[1] : text
  const offered = sortsFor(effective)
  return {
    text,
    mode: effective,
    sort: sort !== null && offered.includes(sort) ? sort : offered[0],
    pattern,
    regexError: text && effective === 'regex' ? checkRustRegex(pattern) : null,
  }
}
