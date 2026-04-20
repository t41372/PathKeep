/**
 * This module holds UI-facing intelligence helpers such as provider state, evidence links, and assistant response metadata.
 *
 * Why this file exists:
 * - Files in `src/lib/` are where UI policy becomes testable without inflating every route component.
 * - If you are trying to understand a front-end contract quickly, these helpers usually explain the reusable part of the story.
 *
 * Main declarations:
 * - `IntelligenceTone`
 * - `selectedAiProvider`
 * - `aiStatusMeta`
 * - `scoreBand`
 * - `evidenceHref`
 * - `assistantHref`
 * - `dedupeEvidence`
 * - `assistantResponseMeta`
 *
 * Source-of-truth notes:
 * - Keep helper behavior aligned with the shipping design, feature, and architecture docs rather than local route assumptions.
 * - Avoid burying user-visible copy or route-only workflow rules here unless the helper truly owns that cross-cutting contract.
 *
 * TODO: M11 - Split this remaining mixed helper surface into narrower modules
 * once the next app-wide reuse audit decides which pieces belong to route
 * grammar, evidence helpers, and assistant/runtime presentation.
 */

import type {
  AiAssistantResponse,
  AiAssistantCitation,
  AiIndexStatus,
  AiProviderConfig,
  AppConfig,
} from './types'
import {
  buildDayInsightsSearchParams,
  buildIntelligenceSearchParams,
  localDateKeyFromIso,
  singleDayDateRange,
  type DateRange,
  type InsightEntityReference,
  type InsightRouteFocus,
  type TimeRangePreset,
} from './core-intelligence'

/**
 * Defines the type-level contract for intelligence tone.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export type IntelligenceTone = 'success' | 'warning' | 'blocked' | 'info'
export type InsightEntityKind =
  | 'day'
  | 'domain'
  | 'queryFamily'
  | 'refindPage'
  | 'session'
  | 'compareSet'
  | 'trail'
/**
 * Defines the type-level contract for translate.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
type Translate = (key: string, vars?: Record<string, string | number>) => string

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

/**
 * Provides selected ai to descendant components.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function selectedAiProvider(
  config: AppConfig['ai'],
  purpose: 'embedding' | 'llm',
): AiProviderConfig | null {
  const providerId =
    purpose === 'embedding' ? config.embeddingProviderId : config.llmProviderId
  const providers =
    purpose === 'embedding' ? config.embeddingProviders : config.llmProviders
  return providers.find((provider) => provider.id === providerId) ?? null
}

/**
 * Explains how ai status meta works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function aiStatusMeta(
  status: AiIndexStatus,
  t: Translate,
): {
  label: string
  tone: IntelligenceTone
  description: string
} {
  switch (status.state) {
    case 'ready':
      return {
        label: t('statusReadyLabel'),
        tone: 'success',
        description: t('statusReadyDescription', {
          count: status.indexedItems,
        }),
      }
    case 'rebuilding':
      return {
        label: t('statusRebuildingLabel'),
        tone: 'warning',
        description: t('statusRebuildingDescription'),
      }
    case 'queued':
      return {
        label: t('statusQueuedLabel'),
        tone: 'warning',
        description: t('statusQueuedDescription'),
      }
    case 'paused':
      return {
        label: t('statusPausedLabel'),
        tone: 'warning',
        description: t('statusPausedDescription'),
      }
    case 'failed':
      return {
        label: t('statusFailedLabel'),
        tone: 'blocked',
        description: status.warning ?? t('statusFailedDescription'),
      }
    case 'stale':
      return {
        label: t('statusStaleLabel'),
        tone: 'warning',
        description: status.warning ?? t('statusStaleDescription'),
      }
    case 'degraded':
      return {
        label: t('statusDegradedLabel'),
        tone: 'blocked',
        description: status.warning ?? t('statusDegradedDescription'),
      }
    case 'blocked':
      return {
        label: t('statusBlockedLabel'),
        tone: 'blocked',
        description: status.warning ?? t('statusBlockedDescription'),
      }
    case 'disabled':
      return {
        label: t('statusDisabledLabel'),
        tone: 'info',
        description: t('statusDisabledDescription'),
      }
    default:
      return {
        label: t('statusEmptyLabel'),
        tone: 'info',
        description: status.warning ?? t('statusEmptyDescription'),
      }
  }
}

/**
 * Explains how score band works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function scoreBand(
  score: number | null | undefined,
  t: Translate,
): {
  label: string
  tone: IntelligenceTone
} {
  if (score == null) return { label: t('noScore'), tone: 'info' }
  if (score >= 0.85) return { label: t('highConfidence'), tone: 'success' }
  if (score >= 0.65) return { label: t('relevant'), tone: 'warning' }
  return { label: t('weakMatch'), tone: 'info' }
}

/**
 * Explains how evidence href works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function evidenceHref(evidence: {
  profileId?: string | null
  url?: string | null
  domain?: string | null
  title?: string | null
  dateRange?: DateRange | null
}) {
  const params = new URLSearchParams()
  if (evidence.profileId) params.set('profileId', evidence.profileId)
  if (evidence.domain) params.set('domain', evidence.domain)
  if (evidence.dateRange) {
    params.set('start', evidence.dateRange.start)
    params.set('end', evidence.dateRange.end)
  }
  if (evidence.url) params.set('q', evidence.url)
  else if (evidence.title) params.set('q', evidence.title)
  const query = params.toString()
  return query ? `/explorer?${query}` : '/explorer'
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

/**
 * Explains how assistant href works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function assistantHref(question: string, profileId?: string | null) {
  const params = new URLSearchParams()
  params.set('question', question)
  if (profileId) {
    params.set('profileId', profileId)
  }
  return `/assistant?${params.toString()}`
}

/**
 * Explains how dedupe evidence works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function dedupeEvidence<
  T extends Pick<AiAssistantCitation, 'historyId' | 'url'>,
>(items: T[]) {
  const seen = new Set<string>()
  return items.filter((item) => {
    const key = `${item.historyId}:${item.url}`
    if (seen.has(key)) return false
    seen.add(key)
    return true
  })
}

/**
 * Explains how assistant response meta works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function assistantResponseMeta(
  response: AiAssistantResponse,
  t: Translate,
): {
  label: string
  tone: IntelligenceTone
} {
  switch (response.state) {
    case 'completed':
      return { label: t('answerReady'), tone: 'success' }
    case 'queued':
      return { label: t('queued'), tone: 'warning' }
    case 'insufficient-evidence':
      return { label: t('evidenceMissing'), tone: 'blocked' }
    case 'failed':
      return { label: t('assistantFailed'), tone: 'blocked' }
    case 'cancelled':
      return { label: t('cancelled'), tone: 'info' }
    default:
      return { label: t('inProgress'), tone: 'info' }
  }
}
