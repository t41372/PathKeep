/**
 * Shared route and local-date helpers for Core Intelligence entity surfaces.
 *
 * Why this file exists:
 * - Domain and day insights are first-class entities, so their route grammar
 *   should not be reassembled ad hoc inside individual pages.
 * - M11 also promotes generic insight href/label helpers into this file so
 *   route grammar stops living inside the mixed `src/lib/intelligence.ts`
 *   barrel.
 */

import type {
  InsightEntityReference,
  DateRange,
  TimeRangePreset,
} from './types'

export type InsightRouteFocusType = 'compare-set' | 'path-flow'

export interface InsightRouteFocus {
  focusType: InsightRouteFocusType
  focusId: string
}

export interface BuildIntelligenceSearchParamsOptions {
  dateRange: DateRange
  preset: TimeRangePreset
  profileId?: string | null
  focus?: InsightRouteFocus | null
}

export type InsightEntityKind =
  | 'day'
  | 'domain'
  | 'queryFamily'
  | 'refindPage'
  | 'session'
  | 'compareSet'
  | 'trail'

type RoutedInsightEntityTarget = {
  dateRange: DateRange
  preset?: TimeRangePreset
  profileId?: string | null
  focus?: InsightRouteFocus | null
}

export type InsightEntityTarget =
  | {
      kind: 'day'
      date: string
      profileId?: string | null
      focus?: InsightRouteFocus | null
    }
  | ({
      kind: 'domain'
      domain: string
    } & RoutedInsightEntityTarget)
  | ({
      kind: 'queryFamily'
      familyId: string
    } & RoutedInsightEntityTarget)
  | ({
      kind: 'refindPage'
      canonicalUrl: string
    } & RoutedInsightEntityTarget)
  | ({
      kind: 'session'
      sessionId: string
    } & RoutedInsightEntityTarget)
  | ({
      kind: 'compareSet'
      compareSetId: string
    } & RoutedInsightEntityTarget)
  | ({
      kind: 'trail'
      trailId: string
    } & RoutedInsightEntityTarget)

type Translate = (key: string, vars?: Record<string, string | number>) => string

export function buildIntelligenceSearchParams({
  dateRange,
  preset,
  profileId,
  focus,
}: BuildIntelligenceSearchParamsOptions) {
  const params = new URLSearchParams()
  params.set('range', preset)
  if (preset === 'custom') {
    params.set('start', dateRange.start)
    params.set('end', dateRange.end)
  }
  if (profileId) {
    params.set('profileId', profileId)
  }
  if (focus?.focusId) {
    params.set('focusType', focus.focusType)
    params.set('focusId', focus.focusId)
  }
  return params
}

export function buildDayInsightsSearchParams(
  profileId?: string | null,
  focus?: InsightRouteFocus | null,
) {
  const params = new URLSearchParams()
  if (profileId) {
    params.set('profileId', profileId)
  }
  if (focus?.focusId) {
    params.set('focusType', focus.focusType)
    params.set('focusId', focus.focusId)
  }
  return params
}

export function parseInsightRouteFocus(
  searchParams: URLSearchParams,
): InsightRouteFocus | null {
  const focusType = searchParams.get('focusType')
  const focusId = searchParams.get('focusId')
  if (!focusId || (focusType !== 'compare-set' && focusType !== 'path-flow')) {
    return null
  }
  return { focusType, focusId }
}

export function isLocalDateKey(value: string) {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) {
    return false
  }

  const [year, month, day] = value.split('-').map(Number)
  const date = new Date(year, month - 1, day)
  return (
    date.getFullYear() === year &&
    date.getMonth() === month - 1 &&
    date.getDate() === day
  )
}

export function formatLocalDateKey(date: Date) {
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}

export function localDateKeyFromIso(value: string) {
  return formatLocalDateKey(new Date(value))
}

export function singleDayDateRange(dateKey: string): DateRange {
  return {
    start: dateKey,
    end: dateKey,
  }
}

function buildInsightEntityRoutePath(
  target: Exclude<InsightEntityTarget, { kind: 'day' }>,
) {
  switch (target.kind) {
    case 'domain':
      return `/intelligence/domain/${encodeURIComponent(target.domain)}`
    case 'queryFamily':
      return `/intelligence/query-family/${encodeURIComponent(target.familyId)}`
    case 'refindPage':
      return `/intelligence/refind/${encodeURIComponent(target.canonicalUrl)}`
    case 'session':
      return `/intelligence/session/${encodeURIComponent(target.sessionId)}`
    case 'compareSet':
      return `/intelligence/compare-set/${encodeURIComponent(target.compareSetId)}`
    case 'trail':
      return `/intelligence/trail/${encodeURIComponent(target.trailId)}`
  }
}

export function insightEntityHref(target: InsightEntityTarget) {
  if (target.kind === 'day') {
    const params = buildDayInsightsSearchParams(target.profileId, target.focus)
    const query = params.toString()
    return `/intelligence/day/${encodeURIComponent(target.date)}${query ? `?${query}` : ''}`
  }

  const params = buildIntelligenceSearchParams({
    dateRange: target.dateRange,
    preset: target.preset ?? 'custom',
    profileId: target.profileId,
    focus: target.focus,
  })
  const query = params.toString()
  return `${buildInsightEntityRoutePath(target)}${query ? `?${query}` : ''}`
}

export function dayInsightsHref(
  date: string,
  profileId?: string | null,
  focus?: InsightRouteFocus | null,
) {
  return insightEntityHref({
    kind: 'day',
    date,
    profileId,
    focus,
  })
}

export function visitDayInsightsHref(
  visitedAt: string,
  profileId?: string | null,
  focus?: InsightRouteFocus | null,
) {
  return dayInsightsHref(localDateKeyFromIso(visitedAt), profileId, focus)
}

export function domainInsightsHref(options: {
  domain: string
  dateRange: DateRange
  preset?: TimeRangePreset
  profileId?: string | null
  focus?: InsightRouteFocus | null
}) {
  return insightEntityHref({
    kind: 'domain',
    domain: options.domain,
    dateRange: options.dateRange,
    preset: options.preset,
    profileId: options.profileId,
    focus: options.focus,
  })
}

export function domainDayInsightsHref(
  domain: string,
  date: string,
  profileId?: string | null,
  focus?: InsightRouteFocus | null,
) {
  return domainInsightsHref({
    domain,
    dateRange: singleDayDateRange(date),
    preset: 'custom',
    profileId,
    focus,
  })
}

export function queryFamilyInsightsHref(options: {
  familyId: string
  dateRange: DateRange
  preset?: TimeRangePreset
  profileId?: string | null
  focus?: InsightRouteFocus | null
}) {
  return insightEntityHref({
    kind: 'queryFamily',
    familyId: options.familyId,
    dateRange: options.dateRange,
    preset: options.preset,
    profileId: options.profileId,
    focus: options.focus,
  })
}

export function refindInsightsHref(options: {
  canonicalUrl: string
  dateRange: DateRange
  preset?: TimeRangePreset
  profileId?: string | null
  focus?: InsightRouteFocus | null
}) {
  return insightEntityHref({
    kind: 'refindPage',
    canonicalUrl: options.canonicalUrl,
    dateRange: options.dateRange,
    preset: options.preset,
    profileId: options.profileId,
    focus: options.focus,
  })
}

export function sessionInsightsHref(options: {
  sessionId: string
  dateRange: DateRange
  preset?: TimeRangePreset
  profileId?: string | null
  focus?: InsightRouteFocus | null
}) {
  return insightEntityHref({
    kind: 'session',
    sessionId: options.sessionId,
    dateRange: options.dateRange,
    preset: options.preset,
    profileId: options.profileId,
    focus: options.focus,
  })
}

export function trailInsightsHref(options: {
  trailId: string
  dateRange: DateRange
  preset?: TimeRangePreset
  profileId?: string | null
  focus?: InsightRouteFocus | null
}) {
  return insightEntityHref({
    kind: 'trail',
    trailId: options.trailId,
    dateRange: options.dateRange,
    preset: options.preset,
    profileId: options.profileId,
    focus: options.focus,
  })
}

export function compareSetInsightsHref(options: {
  compareSetId: string
  dateRange: DateRange
  preset?: TimeRangePreset
  profileId?: string | null
  focus?: InsightRouteFocus | null
}) {
  return insightEntityHref({
    kind: 'compareSet',
    compareSetId: options.compareSetId,
    dateRange: options.dateRange,
    preset: options.preset,
    profileId: options.profileId,
    focus: options.focus,
  })
}

export function reopenedInvestigationHref(options: {
  anchorId: string
  anchorType: 'query_family' | 'reference_page'
  dateRange: DateRange
  preset?: TimeRangePreset
  profileId?: string | null
  focus?: InsightRouteFocus | null
}) {
  return options.anchorType === 'query_family'
    ? queryFamilyInsightsHref({
        familyId: options.anchorId,
        dateRange: options.dateRange,
        preset: options.preset,
        profileId: options.profileId,
        focus: options.focus,
      })
    : refindInsightsHref({
        canonicalUrl: options.anchorId,
        dateRange: options.dateRange,
        preset: options.preset,
        profileId: options.profileId,
        focus: options.focus,
      })
}

export interface InsightEntityReferenceHrefContext {
  dateRange: DateRange
  preset?: TimeRangePreset
  profileId?: string | null
  focus?: InsightRouteFocus | null
}

export function insightEntityReferenceLabel(
  target: InsightEntityReference,
  t: Translate,
) {
  switch (target.kind) {
    case 'day':
      return target.date
    case 'domain':
      return target.domain
    case 'queryFamily':
      return t('queryFamilyRouteTitle')
    case 'refindPage':
      return t('refindRouteTitle')
    case 'session':
      return t('sessionRouteTitle')
    case 'trail':
      return t('trailRouteTitle')
    case 'compareSet':
      return t('compareSetRouteTitle')
  }
}

export function insightEntityReferenceHref(
  target: InsightEntityReference,
  context: InsightEntityReferenceHrefContext,
) {
  switch (target.kind) {
    case 'day':
      return dayInsightsHref(target.date, context.profileId, context.focus)
    case 'domain':
      return domainInsightsHref({
        domain: target.domain,
        dateRange: context.dateRange,
        preset: context.preset,
        profileId: context.profileId,
        focus: context.focus,
      })
    case 'queryFamily':
      return queryFamilyInsightsHref({
        familyId: target.familyId,
        dateRange: context.dateRange,
        preset: context.preset,
        profileId: context.profileId,
        focus: context.focus,
      })
    case 'refindPage':
      return refindInsightsHref({
        canonicalUrl: target.canonicalUrl,
        dateRange: context.dateRange,
        preset: context.preset,
        profileId: context.profileId,
        focus: context.focus,
      })
    case 'session':
      return sessionInsightsHref({
        sessionId: target.sessionId,
        dateRange: context.dateRange,
        preset: context.preset,
        profileId: context.profileId,
        focus: context.focus,
      })
    case 'trail':
      return trailInsightsHref({
        trailId: target.trailId,
        dateRange: context.dateRange,
        preset: context.preset,
        profileId: context.profileId,
        focus: context.focus,
      })
    case 'compareSet':
      return compareSetInsightsHref({
        compareSetId: target.compareSetId,
        dateRange: context.dateRange,
        preset: context.preset,
        profileId: context.profileId,
        focus: context.focus,
      })
  }
}
