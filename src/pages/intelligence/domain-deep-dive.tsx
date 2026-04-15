/**
 * Domain Deep Dive page — full breakdown for a single domain.
 *
 * Why this file exists:
 * - Part of Core Intelligence P2-5b Domain Deep Dive.
 * - Accessed by clicking a domain from Top Sites or any domain link.
 * - Shows Top Pages, Referrers, Exits, Arrival Breakdown, and Visit Trend.
 *
 * Source-of-truth:
 * - `docs/features/core-intelligence-ultimate-design.md` §4.1
 */

import { useI18n } from '../../lib/i18n/hooks'
import {
  useTimeRange,
  useAsyncData,
  type DateRange,
  type DomainTrendPoint,
} from '../../lib/core-intelligence'
import * as api from '../../lib/core-intelligence/api'

// ---------------------------------------------------------------------------
// Props — the domain to drill into is passed via route params / state
// ---------------------------------------------------------------------------

interface DomainDeepDivePageProps {
  domain: string
  dateRange?: DateRange
  onBack?: () => void
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function DomainDeepDivePage({
  domain,
  dateRange: providedDateRange,
  onBack,
}: DomainDeepDivePageProps) {
  const { t } = useI18n('intelligence')
  const { dateRange: fallbackDateRange } = useTimeRange('month')
  const dateRange = providedDateRange ?? fallbackDateRange

  const { data, loading, error } = useAsyncData(
    () => api.getDomainDeepDive(domain, dateRange, null),
    [domain, dateRange],
  )

  if (loading) {
    return (
      <div className="intelligence-page domain-deep-dive">
        <div className="intelligence-skeleton intelligence-skeleton--card" />
      </div>
    )
  }

  if (error || !data) {
    return (
      <div className="intelligence-page domain-deep-dive">
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">
            {error || t('domainDeepDiveEmpty')}
          </p>
        </div>
      </div>
    )
  }

  const arrivalTotal =
    data.arrivalBreakdown.search +
    data.arrivalBreakdown.link +
    data.arrivalBreakdown.typed +
    data.arrivalBreakdown.other

  return (
    <div className="intelligence-page domain-deep-dive">
      {onBack && (
        <button
          className="btn-secondary"
          type="button"
          onClick={onBack}
          style={{ alignSelf: 'flex-start', marginBottom: 'var(--space-3)' }}
        >
          ← {t('domainDeepDiveBack')}
        </button>
      )}

      {/* Header */}
      <div className="domain-deep-dive__header">
        <span className="domain-deep-dive__domain-name">
          {data.displayName ?? data.registrableDomain}
        </span>
        <span className="domain-deep-dive__category-badge">
          {t(`category_${data.domainCategory}`) || data.domainCategory}
        </span>
      </div>

      {/* KPI row */}
      <div className="domain-deep-dive__kpi-row">
        <div className="domain-deep-dive__kpi">
          <span className="domain-deep-dive__kpi-value">
            {formatNumber(data.totalVisits)}
          </span>
          <span className="domain-deep-dive__kpi-label">
            {t('domainDeepDiveVisits')}
          </span>
        </div>
        <div className="domain-deep-dive__kpi">
          <span className="domain-deep-dive__kpi-value">{data.activeDays}</span>
          <span className="domain-deep-dive__kpi-label">
            {t('domainDeepDiveActiveDays')}
          </span>
        </div>
        <div className="domain-deep-dive__kpi">
          <span className="domain-deep-dive__kpi-value">{data.trailCount}</span>
          <span className="domain-deep-dive__kpi-label">
            {t('domainDeepDiveTrails')}
          </span>
        </div>
      </div>

      {/* Arrival breakdown */}
      {arrivalTotal > 0 && (
        <div>
          <h3 className="domain-deep-dive__section-title">
            {t('domainDeepDiveArrival')}
          </h3>
          <div
            className="domain-deep-dive__arrival-breakdown"
            style={{ width: '100%' }}
          >
            {[
              { key: 'search', value: data.arrivalBreakdown.search },
              { key: 'link', value: data.arrivalBreakdown.link },
              { key: 'typed', value: data.arrivalBreakdown.typed },
              { key: 'other', value: data.arrivalBreakdown.other },
            ]
              .filter((a) => a.value > 0)
              .map((a) => (
                <span
                  key={a.key}
                  className={`domain-deep-dive__arrival-bar domain-deep-dive__arrival-bar--${a.key}`}
                  style={{
                    width: `${Math.round((a.value / arrivalTotal) * 100)}%`,
                  }}
                  title={`${t(`domainDeepDiveArrival_${a.key}`)}: ${Math.round((a.value / arrivalTotal) * 100)}%`}
                />
              ))}
          </div>
          <div className="domain-deep-dive__arrival-legend">
            <span>
              🔍 {t('domainDeepDiveArrival_search')}{' '}
              {Math.round((data.arrivalBreakdown.search / arrivalTotal) * 100)}%
            </span>
            <span>
              🔗 {t('domainDeepDiveArrival_link')}{' '}
              {Math.round((data.arrivalBreakdown.link / arrivalTotal) * 100)}%
            </span>
            <span>
              ⌨️ {t('domainDeepDiveArrival_typed')}{' '}
              {Math.round((data.arrivalBreakdown.typed / arrivalTotal) * 100)}%
            </span>
          </div>
        </div>
      )}

      {/* Sections: Top Pages, Referrers, Exits */}
      <div className="domain-deep-dive__sections">
        {data.topPages.length > 0 && (
          <div>
            <h3 className="domain-deep-dive__section-title">
              {t('domainDeepDiveTopPages')}
            </h3>
            {data.topPages.slice(0, 10).map((page) => (
              <div key={page.path} className="domain-deep-dive__page-row">
                <span className="domain-deep-dive__page-path">{page.path}</span>
                <span className="domain-deep-dive__page-count">
                  {formatNumber(page.visitCount)}
                </span>
              </div>
            ))}
          </div>
        )}

        {data.topReferrers.length > 0 && (
          <div>
            <h3 className="domain-deep-dive__section-title">
              {t('domainDeepDiveReferrers')}
            </h3>
            {data.topReferrers.slice(0, 5).map((ref) => (
              <div key={ref.domain} className="domain-deep-dive__flow-row">
                <span className="domain-deep-dive__flow-domain">
                  {ref.displayName ?? ref.domain}
                </span>
                <span className="domain-deep-dive__flow-count">
                  {formatNumber(ref.count)}
                </span>
              </div>
            ))}
          </div>
        )}

        {data.topExits.length > 0 && (
          <div>
            <h3 className="domain-deep-dive__section-title">
              {t('domainDeepDiveExits')}
            </h3>
            {data.topExits.slice(0, 5).map((exit) => (
              <div key={exit.domain} className="domain-deep-dive__flow-row">
                <span className="domain-deep-dive__flow-domain">
                  {exit.displayName ?? exit.domain}
                </span>
                <span className="domain-deep-dive__flow-count">
                  {formatNumber(exit.count)}
                </span>
              </div>
            ))}
          </div>
        )}

        {/* Visit trend mini chart */}
        {data.visitTrend.length > 0 && (
          <div>
            <h3 className="domain-deep-dive__section-title">
              {t('domainDeepDiveTrend')}
            </h3>
            <div className="discovery-trend__chart" style={{ height: 80 }}>
              {data.visitTrend.map((point: DomainTrendPoint) => {
                const max = Math.max(
                  ...data.visitTrend.map((p: DomainTrendPoint) => p.visitCount),
                  1,
                )
                return (
                  <div
                    key={point.dateKey}
                    className="discovery-trend__bar-group"
                    title={`${point.dateKey}: ${point.visitCount}`}
                  >
                    <div className="discovery-trend__domain-bar-container">
                      <span
                        className="discovery-trend__rate-bar"
                        style={{
                          height: `${Math.round((point.visitCount / max) * 100)}%`,
                        }}
                      />
                    </div>
                  </div>
                )
              })}
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatNumber(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return String(n)
}
