/**
 * Shared invoke, normalization, and cache helpers for Core Intelligence API modules.
 *
 * Why this file exists:
 * - M10 keeps the public API surface stable while splitting the old mega-file
 *   by ownership, so cross-cutting transport/cache logic needs one home.
 */

import { call } from '../../backend-client/shared'
import type {
  CoreIntelligencePrimaryOverview,
  CoreIntelligenceSecondaryOverview,
  CoreIntelligenceSectionMeta,
  CoreIntelligenceSectionResult,
  CoreIntelligenceSectionTiming,
  CoreIntelligenceSectionWindow,
  DateRange,
} from '../types'

export function invokeRequest<
  TResponse,
  TRequest extends Record<string, unknown>,
>(command: string, request: TRequest) {
  return call<TResponse>(command, { request })
}

function directSectionFallback(
  sectionId: string,
  window: CoreIntelligenceSectionWindow,
): CoreIntelligenceSectionMeta {
  return {
    sectionId,
    generatedAt: null,
    window,
    moduleIds: [],
    sourceTables: [],
    includesEnrichment: false,
    state: 'degraded',
    stateReason: null,
    notes: [],
  }
}

function normalizeSectionWindow(
  window: unknown,
  fallback: CoreIntelligenceSectionWindow,
): CoreIntelligenceSectionWindow {
  if (!window || typeof window !== 'object') {
    return fallback
  }

  const raw = window as Record<string, unknown>
  if (raw.kind === 'date-range') {
    const rawDateRange =
      raw.dateRange && typeof raw.dateRange === 'object'
        ? raw.dateRange
        : raw.date_range && typeof raw.date_range === 'object'
          ? raw.date_range
          : null
    const start =
      rawDateRange &&
      typeof (rawDateRange as Record<string, unknown>).start === 'string'
        ? ((rawDateRange as Record<string, unknown>).start as string)
        : fallback.kind === 'date-range'
          ? fallback.dateRange.start
          : ''
    const end =
      rawDateRange &&
      typeof (rawDateRange as Record<string, unknown>).end === 'string'
        ? ((rawDateRange as Record<string, unknown>).end as string)
        : fallback.kind === 'date-range'
          ? fallback.dateRange.end
          : ''

    return {
      kind: 'date-range',
      dateRange: { start, end },
    }
  }

  if (raw.kind === 'calendar-day-history') {
    const referenceDate =
      typeof raw.referenceDate === 'string'
        ? raw.referenceDate
        : typeof raw.reference_date === 'string'
          ? raw.reference_date
          : fallback.kind === 'calendar-day-history'
            ? fallback.referenceDate
            : formatLocalDateKey(new Date())

    return {
      kind: 'calendar-day-history',
      referenceDate,
    }
  }

  return fallback
}

function normalizeSectionMeta(
  sectionId: string,
  fallbackWindow: CoreIntelligenceSectionWindow,
  meta: unknown,
): CoreIntelligenceSectionMeta {
  if (!meta || typeof meta !== 'object') {
    return directSectionFallback(sectionId, fallbackWindow)
  }

  const raw = meta as Record<string, unknown>
  const state =
    raw.state === 'ready' ||
    raw.state === 'stale' ||
    raw.state === 'disabled' ||
    raw.state === 'degraded'
      ? raw.state
      : 'degraded'

  return {
    sectionId: typeof raw.sectionId === 'string' ? raw.sectionId : sectionId,
    generatedAt:
      typeof raw.generatedAt === 'string' || raw.generatedAt === null
        ? (raw.generatedAt ?? null)
        : null,
    window: normalizeSectionWindow(raw.window, fallbackWindow),
    moduleIds: Array.isArray(raw.moduleIds)
      ? raw.moduleIds.filter(
          (value): value is string => typeof value === 'string',
        )
      : [],
    sourceTables: Array.isArray(raw.sourceTables)
      ? raw.sourceTables.filter(
          (value): value is string => typeof value === 'string',
        )
      : [],
    includesEnrichment: Boolean(raw.includesEnrichment),
    state,
    stateReason:
      typeof raw.stateReason === 'string' || raw.stateReason === null
        ? (raw.stateReason ?? null)
        : null,
    notes: Array.isArray(raw.notes)
      ? raw.notes.filter((value): value is string => typeof value === 'string')
      : [],
  }
}

export function formatLocalDateKey(date: Date) {
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}

export function normalizeSectionResult<T>(
  sectionId: string,
  window: CoreIntelligenceSectionWindow,
  result: CoreIntelligenceSectionResult<T> | T,
): CoreIntelligenceSectionResult<T> {
  if (
    result &&
    typeof result === 'object' &&
    'data' in result &&
    'meta' in result
  ) {
    return {
      data: result.data,
      meta: normalizeSectionMeta(sectionId, window, result.meta),
    }
  }

  return {
    data: result,
    meta: directSectionFallback(sectionId, window),
  }
}

interface OverviewCacheEntry {
  primary?: CoreIntelligencePrimaryOverview
  secondary?: CoreIntelligenceSecondaryOverview
}

const overviewCache = new Map<string, OverviewCacheEntry>()

function overviewScopeKey(dateRange: DateRange, profileId?: string | null) {
  return JSON.stringify({
    dateRange,
    profileId: profileId ?? null,
  })
}

function readOverviewCache(dateRange: DateRange, profileId?: string | null) {
  return overviewCache.get(overviewScopeKey(dateRange, profileId)) ?? null
}

export function writeOverviewCache(
  dateRange: DateRange,
  profileId: string | null | undefined,
  updater: (current: OverviewCacheEntry) => OverviewCacheEntry,
) {
  const key = overviewScopeKey(dateRange, profileId)
  const current = overviewCache.get(key) ?? {}
  overviewCache.set(key, updater(current))
}

export function cachedPrimaryOverview(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return readOverviewCache(dateRange, profileId)?.primary ?? null
}

export function cachedSecondaryOverview(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return readOverviewCache(dateRange, profileId)?.secondary ?? null
}

export function cachedPrimarySectionForProfile<T>(
  profileId: string | null | undefined,
  read: (
    overview: CoreIntelligencePrimaryOverview,
  ) => CoreIntelligenceSectionResult<T> | undefined,
) {
  const targetProfile = profileId ?? null
  for (const [key, entry] of overviewCache.entries()) {
    const parsed = JSON.parse(key) as {
      profileId?: string | null
    }
    if ((parsed.profileId ?? null) !== targetProfile) {
      continue
    }
    const section = entry.primary ? read(entry.primary) : undefined
    if (section) {
      return section
    }
  }

  return null
}

export function cachedSecondarySectionForDateRange<T>(
  dateRange: DateRange,
  read: (
    overview: CoreIntelligenceSecondaryOverview,
  ) => CoreIntelligenceSectionResult<T> | undefined,
) {
  for (const [key, entry] of overviewCache.entries()) {
    const parsed = JSON.parse(key) as {
      dateRange?: DateRange
    }
    if (
      parsed.dateRange?.start !== dateRange.start ||
      parsed.dateRange?.end !== dateRange.end
    ) {
      continue
    }
    const section = entry.secondary ? read(entry.secondary) : undefined
    if (section) {
      return section
    }
  }

  return null
}

function normalizeTiming(
  timing: unknown,
): CoreIntelligenceSectionTiming | null {
  if (!timing || typeof timing !== 'object') {
    return null
  }

  const raw = timing as Record<string, unknown>
  if (typeof raw.sectionId !== 'string' || typeof raw.durationMs !== 'number') {
    return null
  }

  return {
    sectionId: raw.sectionId,
    durationMs: raw.durationMs,
  }
}

function normalizeTimings(value: unknown) {
  if (!Array.isArray(value)) return []
  return value
    .map(normalizeTiming)
    .filter(
      (timing): timing is CoreIntelligenceSectionTiming => timing !== null,
    )
}

export function normalizePrimaryOverview(
  dateRange: DateRange,
  result: CoreIntelligencePrimaryOverview,
): CoreIntelligencePrimaryOverview {
  return {
    digestSummary: normalizeSectionResult(
      'digest-summary',
      { kind: 'date-range', dateRange },
      result.digestSummary,
    ),
    onThisDay: normalizeSectionResult(
      'on-this-day',
      {
        kind: 'calendar-day-history',
        referenceDate: formatLocalDateKey(new Date()),
      },
      result.onThisDay,
    ),
    topSites: normalizeSectionResult(
      'top-sites',
      { kind: 'date-range', dateRange },
      result.topSites,
    ),
    refindPages: normalizeSectionResult(
      'refind-pages',
      { kind: 'date-range', dateRange },
      result.refindPages,
    ),
    searchEngineRanking: normalizeSectionResult(
      'search-activity',
      { kind: 'date-range', dateRange },
      result.searchEngineRanking,
    ),
    topSearchConcepts: normalizeSectionResult(
      'search-activity',
      { kind: 'date-range', dateRange },
      result.topSearchConcepts,
    ),
    queryFamilies: normalizeSectionResult(
      'search-activity',
      { kind: 'date-range', dateRange },
      result.queryFamilies,
    ),
    activityMix: normalizeSectionResult(
      'activity-mix',
      { kind: 'date-range', dateRange },
      result.activityMix,
    ),
    discoveryTrendDay: normalizeSectionResult(
      'browsing-rhythm',
      { kind: 'date-range', dateRange },
      result.discoveryTrendDay,
    ),
    habitPatterns: normalizeSectionResult(
      'habits',
      { kind: 'date-range', dateRange },
      result.habitPatterns,
    ),
    interruptedHabits: normalizeSectionResult(
      'habits',
      { kind: 'date-range', dateRange },
      result.interruptedHabits,
    ),
    timings: normalizeTimings(result.timings),
    totalDurationMs:
      typeof result.totalDurationMs === 'number' ? result.totalDurationMs : 0,
  }
}

export function normalizeSecondaryOverview(
  dateRange: DateRange,
  result: CoreIntelligenceSecondaryOverview,
): CoreIntelligenceSecondaryOverview {
  return {
    stableSources: normalizeSectionResult(
      'stable-sources',
      { kind: 'date-range', dateRange },
      result.stableSources,
    ),
    searchEffectiveness: normalizeSectionResult(
      'search-effectiveness',
      { kind: 'date-range', dateRange },
      result.searchEffectiveness,
    ),
    frictionSignals: normalizeSectionResult(
      'friction-signals',
      { kind: 'date-range', dateRange },
      result.frictionSignals,
    ),
    reopenedInvestigations: normalizeSectionResult(
      'reopened-investigations',
      { kind: 'date-range', dateRange },
      result.reopenedInvestigations,
    ),
    discoveryTrendWeek: normalizeSectionResult(
      'discovery-trend',
      { kind: 'date-range', dateRange },
      result.discoveryTrendWeek,
    ),
    breadthIndex: normalizeSectionResult(
      'breadth-index',
      { kind: 'date-range', dateRange },
      result.breadthIndex,
    ),
    pathFlows: normalizeSectionResult(
      'path-flows',
      { kind: 'date-range', dateRange },
      result.pathFlows,
    ),
    compareSets: normalizeSectionResult(
      'compare-sets',
      { kind: 'date-range', dateRange },
      result.compareSets,
    ),
    multiBrowserDiff: normalizeSectionResult(
      'multi-browser-diff',
      { kind: 'date-range', dateRange },
      result.multiBrowserDiff,
    ),
    observedInteractions: normalizeSectionResult(
      'observed-interactions',
      { kind: 'date-range', dateRange },
      result.observedInteractions,
    ),
    timings: normalizeTimings(result.timings),
    totalDurationMs:
      typeof result.totalDurationMs === 'number' ? result.totalDurationMs : 0,
  }
}

export function invokeSectionRequest<
  TResponse,
  TRequest extends Record<string, unknown>,
>(
  command: string,
  request: TRequest,
  sectionId: string,
  window: CoreIntelligenceSectionWindow,
) {
  return call<CoreIntelligenceSectionResult<TResponse> | TResponse>(command, {
    request,
  }).then((result) => normalizeSectionResult(sectionId, window, result))
}

export function invokeSectionArgs<TResponse>(
  command: string,
  args: Record<string, unknown>,
  sectionId: string,
  window: CoreIntelligenceSectionWindow,
) {
  return call<CoreIntelligenceSectionResult<TResponse> | TResponse>(
    command,
    args,
  ).then((result) => normalizeSectionResult(sectionId, window, result))
}

export function clearOverviewCache() {
  overviewCache.clear()
}
