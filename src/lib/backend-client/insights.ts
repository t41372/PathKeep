/**
 * Read commands for the deterministic insights (computed locally from the
 * archive by the core-intelligence jobs, no AI involved).
 *
 * Each section comes back as `{ data, meta }`. `meta.state` is `stale` while
 * the background job that refreshes it is still running after a backup, so
 * callers can show the old numbers and poll until they settle.
 */
import type {
  ActivityMixTrend,
  CoreIntelligenceSectionMeta,
  DateRange,
  DigestSummary,
  DiscoveryTrend,
  OnThisDayEntry,
  QueryFamilyResult,
  RefindPage,
  ReopenedInvestigation,
  RhythmHeatmap,
  SearchConcept,
  TopSite,
} from '../core-intelligence/types'
import { call } from './shared'

export interface Section<T> {
  data: T
  meta: CoreIntelligenceSectionMeta | null
}

async function section<T>(command: string, args: Record<string, unknown>) {
  const result = await call<Section<T> | T>(command, args)
  if (
    result &&
    typeof result === 'object' &&
    'data' in result &&
    'meta' in result
  ) {
    return result
  }
  return { data: result as T, meta: null }
}

interface Scope {
  dateRange: DateRange
  profileId?: string | null
}

export const insightsClient = {
  digest: (scope: Scope) =>
    section<DigestSummary>('get_digest_summary', { request: scope }),
  discoveryTrend: (scope: Scope, granularity: 'day' | 'week' = 'day') =>
    section<DiscoveryTrend>('get_discovery_trend', {
      request: { ...scope, granularity },
    }),
  activityMixTrend: (scope: Scope, granularity: 'day' | 'week' = 'day') =>
    call<ActivityMixTrend>('get_activity_mix_trend', {
      request: { ...scope, granularity },
    }),
  onThisDay: (profileId?: string | null) =>
    section<OnThisDayEntry[]>('get_on_this_day', { profileId }),
  topSites: (scope: Scope, limit = 10) =>
    section<TopSite[]>('get_top_sites', {
      request: { ...scope, sortBy: 'visits', limit },
    }),
  rhythm: (scope: Scope) =>
    section<RhythmHeatmap>('get_browsing_rhythm', { request: scope }),
  searchConcepts: (scope: Scope, limit = 12) =>
    section<SearchConcept[]>('get_top_search_concepts', {
      request: { ...scope, limit },
    }),
  /** Pages are zero-based, like every paged intelligence command. */
  queryFamilies: (scope: Scope, pageSize = 200) =>
    section<QueryFamilyResult>('get_query_families', {
      request: { ...scope, page: 0, pageSize },
    }),
  refindPages: (scope: Scope, limit = 8) =>
    section<RefindPage[]>('get_refind_pages', { request: { ...scope, limit } }),
  reopenedInvestigations: (scope: Scope) =>
    section<ReopenedInvestigation[]>('get_reopened_investigations', {
      request: scope,
    }),
}

/** True while the background job that refreshes this section is running. */
export function isStale(
  result: { meta: { state: string } | null } | undefined,
) {
  return result?.meta?.state === 'stale'
}

/** Keeps polling a section while its background refresh is still running. */
export function pollWhileStale(query: {
  state: { data?: { meta: { state: string } | null } }
}): number | false {
  return isStale(query.state.data) ? 5_000 : false
}

/** `YYYY-MM-DD` in local time, the format every insights command expects. */
export function localDateKey(date: Date) {
  const pad = (value: number) => String(value).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`
}

/** The last `days` days, ending today. */
export function lastDays(days: number): DateRange {
  const end = new Date()
  const start = new Date(end)
  start.setDate(start.getDate() - (days - 1))
  return { start: localDateKey(start), end: localDateKey(end) }
}

export function calendarYear(year: number): DateRange {
  return { start: `${year}-01-01`, end: `${year}-12-31` }
}
