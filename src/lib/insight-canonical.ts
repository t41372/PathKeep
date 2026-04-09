import { calendarDayKey } from './format'
import type { InsightEvidenceItem, InsightSnapshot } from './types'

type InsightTranslator = (
  key: string,
  values?: Record<string, number | string>,
) => string

export interface InsightTopDomain {
  domain: string
  count: number
  pct: number
}

function uniqueNonEmpty(values: string[]) {
  return [...new Set(values.map((value) => value.trim()).filter(Boolean))]
}

function resolveLimit(limit: number | undefined, fallback: number) {
  return Math.max(0, limit ?? fallback)
}

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

export function resolveInsightPeriodicSummary(
  snapshot: InsightSnapshot,
  t: InsightTranslator,
  limit = 2,
) {
  const paragraphs = uniqueNonEmpty(snapshot.cards.map((card) => card.summary))
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
