/**
 * Shared helpers for promoted intelligence entity routes.
 *
 * Why this file exists:
 * - M10 splits route-first entity pages by ownership, but they still share the
 *   same scope callout, trail-link card, and compare-set focus utilities.
 */

import { Link } from 'react-router-dom'
import {
  RefindFactorList,
  type RefindWorkbenchFactor,
} from '../../../components/intelligence/workbench'
import { StatusCallout } from '../../../components/primitives/status-callout'
import {
  type DateRange,
  type RefindScoreFactor,
  type TrailSummary,
} from '../../../lib/core-intelligence'
import { trailInsightsHref } from '../../../lib/intelligence'

export function ScopeCallout({ body, title }: { body: string; title: string }) {
  return <StatusCallout tone="info" title={title} body={body} />
}

export function TrailLinkCard({
  dateRange,
  preset,
  profileId,
  t,
  trail,
}: {
  dateRange: DateRange
  preset: 'day' | 'week' | 'month' | 'quarter' | 'year' | 'custom'
  profileId: string | null
  t: (key: string, vars?: Record<string, string | number>) => string
  trail: TrailSummary
}) {
  return (
    <Link
      key={trail.trailId}
      className="trail-card"
      to={trailInsightsHref({
        trailId: trail.trailId,
        dateRange,
        preset,
        profileId,
      })}
    >
      <div className="trail-card__header">
        <span className="trail-card__query">"{trail.initialQuery}"</span>
        <span className="trail-card__meta">
          {t('trailRouteVisitCount', { count: trail.visitCount })}
        </span>
      </div>
      <div className="trail-card__body">
        <div className="trail-card__evolution">
          <span className="trail-card__evolution-label">
            {trail.searchEngine}
          </span>
          <div className="trail-card__evolution-chain">
            {trail.queries.slice(0, 4).map((query, index) => (
              <span
                key={`${trail.trailId}:${query}:${index}`}
                className="trail-card__evolution-step"
              >
                {index > 0 ? (
                  <span className="trail-card__evolution-arrow">→</span>
                ) : null}
                <span className="trail-card__evolution-query">"{query}"</span>
              </span>
            ))}
          </div>
        </div>
      </div>
    </Link>
  )
}

export function RefindFactorSection({
  factors,
}: {
  factors: RefindScoreFactor[]
}) {
  const workbenchFactors = factors.map<RefindWorkbenchFactor>((factor) => ({
    label: factor.signal,
    emphasis: factor.contribution,
    valueLabel: `${factor.rawValue} ×${factor.weight}`,
  }))

  return <RefindFactorList factors={workbenchFactors} />
}
