/**
 * This module derives deterministic insight summaries from canonical evidence without making the routes own that logic directly.
 *
 * Why this file exists:
 * - Files in `src/lib/` are where UI policy becomes testable without inflating every route component.
 * - If you are trying to understand a front-end contract quickly, these helpers usually explain the reusable part of the story.
 *
 * Main declarations:
 * - `InsightTopDomain`
 * - `resolveInsightOnThisDay`
 * - `resolveInsightTopDomains`
 * - `resolveInsightPeriodicSummary`
 *
 * Source-of-truth notes:
 * - Keep helper behavior aligned with the shipping design, feature, and architecture docs rather than local route assumptions.
 * - Avoid burying user-visible copy or route-only workflow rules here unless the helper truly owns that cross-cutting contract.
 */

import { calendarDayKey } from './format'
import type { InsightEvidenceItem, InsightSnapshot } from './types'

/**
 * Defines the type-level contract for insight translator.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
type InsightTranslator = (
  key: string,
  values?: Record<string, number | string>,
) => string

/**
 * Defines the typed shape for insight top domain.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export interface InsightTopDomain {
  domain: string
  count: number
  pct: number
}

/**
 * Explains how unique non empty works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
function uniqueNonEmpty(values: string[]) {
  return [...new Set(values.map((value) => value.trim()).filter(Boolean))]
}

/**
 * Resolves limit from the available inputs.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
function resolveLimit(limit: number | undefined, fallback: number) {
  return Math.max(0, limit ?? fallback)
}

/**
 * Resolves insight on this day from the available inputs.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function resolveInsightOnThisDay(
  snapshot: InsightSnapshot,
  todayKey: string | null,
  limit?: number,
) {
  if (!todayKey) {
    return []
  }

  return [...snapshot.canonical.onThisDay]
    .filter((item) => calendarDayKey(item.visitedAt) === todayKey)
    .sort((left, right) => right.visitedAt.localeCompare(left.visitedAt))
    .slice(0, resolveLimit(limit, snapshot.canonical.onThisDay.length))
}

/**
 * Resolves insight top domains from the available inputs.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function resolveInsightTopDomains(
  snapshot: InsightSnapshot,
  limit = 5,
): InsightTopDomain[] {
  const ranked = [...snapshot.canonical.topDomains]
    .sort(
      (left, right) =>
        right.visitCount - left.visitCount ||
        left.domain.localeCompare(right.domain),
    )
    .slice(0, resolveLimit(limit, 5))

  const maxCount = ranked[0]?.visitCount ?? 0

  return ranked.map((item) => ({
    domain: item.domain,
    count: item.visitCount,
    pct: maxCount === 0 ? 0 : Math.round((item.visitCount / maxCount) * 100),
  }))
}

/**
 * Resolves insight periodic summary from the available inputs.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function resolveInsightPeriodicSummary(
  snapshot: InsightSnapshot,
  t: InsightTranslator,
  limit = 2,
) {
  const paragraphs = uniqueNonEmpty([
    ...snapshot.templateSummaries.map((summary) => summary.body),
    ...snapshot.cards.map((card) => card.summary),
  ])
  const cappedLimit = resolveLimit(limit, 2)
  const rankedDomains = resolveInsightTopDomains(snapshot, 3).map(
    (item) => item.domain,
  )

  if (paragraphs.length < cappedLimit) {
    paragraphs.push(
      t('periodicSummaryFallbackWindow', {
        visits: snapshot.canonical.windowVisitCount,
        domains: snapshot.canonical.windowUniqueDomains,
      }),
    )
  }

  if (paragraphs.length < cappedLimit && rankedDomains.length > 0) {
    paragraphs.push(
      t('periodicSummaryFallbackDomains', {
        domains: rankedDomains.join(', '),
      }),
    )
  }

  return uniqueNonEmpty(paragraphs).slice(0, cappedLimit)
}

export type { InsightEvidenceItem }
