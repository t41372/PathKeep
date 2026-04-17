/**
 * Domain Deep Dive route and presentational surface.
 *
 * Why this file exists:
 * - Domain drilldowns are now a first-class deep-linkable route under `/intelligence/domain/:domain`.
 * - This module keeps the domain detail surface separate from the main intelligence shell while reusing the same range and scope contract.
 *
 * Main declarations:
 * - `DomainDeepDiveRoutePage`
 * - `DomainDeepDivePage`
 *
 * Source-of-truth notes:
 * - Keep this aligned with `docs/features/core-intelligence-ultimate-design.md` §2.4 and §4.1.
 * - Keep route/query behavior aligned with `docs/design/screens-and-nav.md`.
 */

import { Link, useParams } from 'react-router-dom'
import { TimeRangeSelector } from '../../components/intelligence/time-range-selector'
import { StatusCallout } from '../../components/primitives/status-callout'
import { useI18n } from '../../lib/i18n/hooks'
import {
  useAsyncData,
  type DateRange,
  type DomainTrendPoint,
} from '../../lib/core-intelligence'
import * as api from '../../lib/core-intelligence/api'
import { useIntelligenceRouteState } from './route-state'

interface DomainDeepDivePageProps {
  backHref: string
  dateRange: DateRange
  domain: string
  profileId: string | null
}

/**
 * Renders the deep-linkable domain route.
 */
export function DomainDeepDiveRoutePage() {
  const { domain } = useParams<{ domain: string }>()
  const { t } = useI18n('intelligence')
  const {
    dateRange,
    effectiveProfileId,
    preset,
    profileScopeLabel,
    setCustomRange,
    setPreset,
    withCurrentRouteSearch,
  } = useIntelligenceRouteState()

  if (!domain) {
    return (
      <div className="intelligence-page domain-deep-dive">
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('domainDeepDiveEmpty')}</p>
        </div>
      </div>
    )
  }

  return (
    <div
      className="intelligence-page domain-deep-dive"
      data-testid="domain-deep-dive-page"
    >
      <TimeRangeSelector
        key={`${preset}:${dateRange.start}:${dateRange.end}`}
        dateRange={dateRange}
        preset={preset}
        onPresetChange={setPreset}
        onCustomRange={setCustomRange}
        t={t}
      />

      <StatusCallout
        tone="info"
        title={
          effectiveProfileId ? t('scopedViewTitle') : t('archiveWideBadge')
        }
        body={
          effectiveProfileId
            ? t('scopedViewBody', {
                profile: profileScopeLabel ?? effectiveProfileId,
              })
            : t('archiveWideBody')
        }
      />

      <DomainDeepDivePage
        backHref={`/intelligence${withCurrentRouteSearch()}`}
        dateRange={dateRange}
        domain={decodeURIComponent(domain)}
        profileId={effectiveProfileId}
      />
    </div>
  )
}

/**
 * Renders the domain deep-dive surface.
 */
export function DomainDeepDivePage({
  backHref,
  dateRange,
  domain,
  profileId,
}: DomainDeepDivePageProps) {
  const { t } = useI18n('intelligence')
  const { data, loading, error } = useAsyncData(
    () => api.getDomainDeepDive(domain, dateRange, profileId),
    [dateRange, domain, profileId],
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
      <Link className="btn-secondary" to={backHref}>
        ← {t('domainDeepDiveBack')}
      </Link>

      <div className="domain-deep-dive__header">
        <span className="domain-deep-dive__domain-name">
          {data.displayName ?? data.registrableDomain}
        </span>
        <span className="domain-deep-dive__category-badge">
          {t(`category_${data.domainCategory}`) || data.domainCategory}
        </span>
      </div>

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

      {arrivalTotal > 0 ? (
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
              .filter((entry) => entry.value > 0)
              .map((entry) => (
                <span
                  key={entry.key}
                  className={`domain-deep-dive__arrival-bar domain-deep-dive__arrival-bar--${entry.key}`}
                  style={{
                    width: `${Math.round((entry.value / arrivalTotal) * 100)}%`,
                  }}
                  title={`${t(`domainDeepDiveArrival_${entry.key}`)}: ${Math.round((entry.value / arrivalTotal) * 100)}%`}
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
      ) : null}

      <div className="domain-deep-dive__sections">
        {data.topPages.length > 0 ? (
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
        ) : null}

        {data.topReferrers.length > 0 ? (
          <div>
            <h3 className="domain-deep-dive__section-title">
              {t('domainDeepDiveReferrers')}
            </h3>
            {data.topReferrers.slice(0, 5).map((referrer) => (
              <div key={referrer.domain} className="domain-deep-dive__flow-row">
                <span className="domain-deep-dive__flow-domain">
                  {referrer.displayName ?? referrer.domain}
                </span>
                <span className="domain-deep-dive__flow-count">
                  {formatNumber(referrer.count)}
                </span>
              </div>
            ))}
          </div>
        ) : null}

        {data.topExits.length > 0 ? (
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
        ) : null}

        {data.visitTrend.length > 0 ? (
          <div>
            <h3 className="domain-deep-dive__section-title">
              {t('domainDeepDiveTrend')}
            </h3>
            <div className="discovery-trend__chart" style={{ height: 80 }}>
              {data.visitTrend.map((point: DomainTrendPoint) => {
                const max = Math.max(
                  ...data.visitTrend.map(
                    (trendPoint: DomainTrendPoint) => trendPoint.visitCount,
                  ),
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
                        className="discovery-trend__domain-bar"
                        style={{
                          height: `${Math.round((point.visitCount / max) * 100)}%`,
                        }}
                      />
                    </div>
                    <span className="discovery-trend__date-label">
                      {point.dateKey.slice(5)}
                    </span>
                  </div>
                )
              })}
            </div>
          </div>
        ) : null}
      </div>
    </div>
  )
}

function formatNumber(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`
  return String(value)
}
