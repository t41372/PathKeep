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
import { InsightEntityActions } from '../../components/intelligence/entity-actions'
import { InsightEntityHero } from '../../components/intelligence/entity-hero'
import { IntelligenceSectionMeta } from '../../components/intelligence/section-meta'
import { TimeRangeSelector } from '../../components/intelligence/time-range-selector'
import { StatusCallout } from '../../components/primitives/status-callout'
import { useI18n } from '../../lib/i18n/hooks'
import {
  useAsyncData,
  type DateRange,
  type DomainTrendPoint,
  type InsightRouteFocus,
} from '../../lib/core-intelligence'
import * as api from '../../lib/core-intelligence/api'
import {
  compareSetInsightsHref,
  dayInsightsHref,
  domainInsightsHref,
  evidenceHref,
} from '../../lib/intelligence'
import {
  formatDomainPagePath,
  intelligenceCategoryLabel,
  intelligenceText,
} from './copy'
import { useIntelligenceRouteState } from './route-state'

interface DomainDeepDivePageProps {
  backHref: string
  dateRange: DateRange
  dayHref: (date: string) => string
  domainHref: (domain: string) => string
  domain: string
  focus: InsightRouteFocus | null
  profileId: string | null
  scopeLabel: string
}

/**
 * Renders the deep-linkable domain route.
 */
export function DomainDeepDiveRoutePage() {
  const { domain } = useParams<{ domain: string }>()
  const { language, t } = useI18n('intelligence')
  const {
    dateRange,
    effectiveProfileId,
    focus,
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

  const archiveWideBadge = intelligenceText(language, t, 'archiveWideBadge')
  const archiveWideBody = intelligenceText(language, t, 'archiveWideBody')
  const dayHref = (date: string) =>
    dayInsightsHref(date, effectiveProfileId, focus)
  const domainHref = (nextDomain: string) =>
    domainInsightsHref({
      domain: nextDomain,
      dateRange,
      preset,
      profileId: effectiveProfileId,
      focus,
    })

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
        title={effectiveProfileId ? t('scopedViewTitle') : archiveWideBadge}
        body={
          effectiveProfileId
            ? t('scopedViewBody', {
                profile: profileScopeLabel ?? effectiveProfileId,
              })
            : archiveWideBody
        }
      />

      <DomainDeepDivePage
        backHref={`/intelligence${withCurrentRouteSearch({ focus: null })}`}
        dateRange={dateRange}
        dayHref={dayHref}
        domainHref={domainHref}
        domain={decodeURIComponent(domain)}
        focus={focus}
        profileId={effectiveProfileId}
        scopeLabel={
          effectiveProfileId
            ? (profileScopeLabel ?? effectiveProfileId)
            : archiveWideBadge
        }
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
  dayHref,
  domainHref,
  domain,
  focus,
  profileId,
  scopeLabel,
}: DomainDeepDivePageProps) {
  const { language, t } = useI18n('intelligence')
  const { data, loading, error } = useAsyncData(
    () => api.getDomainDeepDive(domain, dateRange, profileId),
    [dateRange, domain, profileId],
  )
  const detail = data?.data ?? null
  const focusedCompareSetId =
    focus?.focusType === 'compare-set' ? focus.focusId : null
  const focusedCompareSetResult = useAsyncData<Awaited<
    ReturnType<typeof api.getCompareSetDetail>
  > | null>(
    () =>
      focusedCompareSetId
        ? api.getCompareSetDetail(focusedCompareSetId, dateRange, profileId)
        : Promise.resolve(null),
    [dateRange, focusedCompareSetId, profileId],
  )
  const focusedCompareSetCandidate = focusedCompareSetResult.data?.data ?? null
  const focusedCompareSet = focusedCompareSetCandidate?.compareSet.pages.some(
    (page) => page.registrableDomain === detail?.registrableDomain,
  )
    ? focusedCompareSetCandidate
    : null
  const focusedComparePaths = new Set(
    focusedCompareSet?.compareSet.pages
      .filter((page) => page.registrableDomain === detail?.registrableDomain)
      .map((page) => canonicalPath(page.canonicalUrl))
      .filter((path): path is string => Boolean(path)) ?? [],
  )
  const focusedPathFlowId =
    focus?.focusType === 'path-flow' ? focus.focusId : null
  const focusedPathFlowStepCount = parsePathFlowStepCount(focusedPathFlowId)
  const focusedPathFlowResult = useAsyncData<Awaited<
    ReturnType<typeof api.getPathFlows>
  > | null>(
    () =>
      focusedPathFlowId
        ? api.getPathFlows(
            dateRange,
            profileId,
            focusedPathFlowStepCount ?? 3,
            50,
          )
        : Promise.resolve(null),
    [dateRange, focusedPathFlowId, focusedPathFlowStepCount, profileId],
  )
  const focusedPathFlow =
    focusedPathFlowResult.data?.data.find(
      (flow) => flow.flowId === focusedPathFlowId,
    ) ?? null
  const pathFlowMatchesCurrentDomain = Boolean(
    focusedPathFlow?.steps.some(
      (step) => step.registrableDomain === detail?.registrableDomain,
    ),
  )

  if (loading) {
    return (
      <div className="intelligence-page domain-deep-dive">
        <div className="intelligence-skeleton intelligence-skeleton--card" />
      </div>
    )
  }

  if (error || !detail) {
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
    detail.arrivalBreakdown.search +
    detail.arrivalBreakdown.link +
    detail.arrivalBreakdown.typed +
    detail.arrivalBreakdown.other

  return (
    <div className="intelligence-page domain-deep-dive">
      <InsightEntityHero
        actions={
          <InsightEntityActions
            items={[
              {
                href: evidenceHref({
                  profileId,
                  domain: detail.registrableDomain,
                  dateRange,
                }),
                label: t('domainInsightsOpenExplorer'),
              },
            ]}
          />
        }
        backHref={backHref}
        backLabel={t('domainDeepDiveBack')}
        eyebrow={t('domainInsightsTitle')}
        subtitle={t('domainInsightsSubtitle')}
        title={detail.displayName ?? detail.registrableDomain}
      />
      <IntelligenceSectionMeta meta={data!.meta} scopeLabel={scopeLabel} />
      {focusedCompareSet ? (
        <StatusCallout
          tone="info"
          title={t('compareSetFocusTitle')}
          body={t('compareSetDomainFocusBody', {
            query: focusedCompareSet.compareSet.searchQuery,
            count: focusedCompareSet.compareSet.pages.length,
          })}
          actions={
            <Link
              className="btn-secondary"
              to={compareSetInsightsHref({
                compareSetId: focusedCompareSet.compareSet.compareSetId,
                dateRange,
                preset: 'custom',
                profileId,
              })}
            >
              {t('compareSetRouteTitle')}
            </Link>
          }
        />
      ) : null}
      {!focusedCompareSet && pathFlowMatchesCurrentDomain && focusedPathFlow ? (
        <StatusCallout
          tone="info"
          title={t('pathFlowFocusTitle')}
          body={t('pathFlowFocusBody', {
            flow: focusedPathFlow.flowPattern,
          })}
        />
      ) : null}

      <div className="domain-deep-dive__header">
        <span className="domain-deep-dive__category-badge">
          {intelligenceCategoryLabel(language, t, detail.domainCategory)}
        </span>
      </div>

      <div className="domain-deep-dive__kpi-row">
        <div className="domain-deep-dive__kpi">
          <span className="domain-deep-dive__kpi-value">
            {formatNumber(detail.totalVisits)}
          </span>
          <span className="domain-deep-dive__kpi-label">
            {t('domainDeepDiveVisits')}
          </span>
        </div>
        <div className="domain-deep-dive__kpi">
          <span className="domain-deep-dive__kpi-value">
            {detail.activeDays}
          </span>
          <span className="domain-deep-dive__kpi-label">
            {t('domainDeepDiveActiveDays')}
          </span>
        </div>
        <div className="domain-deep-dive__kpi">
          <span className="domain-deep-dive__kpi-value">
            {detail.trailCount}
          </span>
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
              { key: 'search', value: detail.arrivalBreakdown.search },
              { key: 'link', value: detail.arrivalBreakdown.link },
              { key: 'typed', value: detail.arrivalBreakdown.typed },
              { key: 'other', value: detail.arrivalBreakdown.other },
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
              {Math.round(
                (detail.arrivalBreakdown.search / arrivalTotal) * 100,
              )}
              %
            </span>
            <span>
              🔗 {t('domainDeepDiveArrival_link')}{' '}
              {Math.round((detail.arrivalBreakdown.link / arrivalTotal) * 100)}%
            </span>
            <span>
              ⌨️ {t('domainDeepDiveArrival_typed')}{' '}
              {Math.round((detail.arrivalBreakdown.typed / arrivalTotal) * 100)}
              %
            </span>
          </div>
        </div>
      ) : null}

      <div className="domain-deep-dive__sections">
        {detail.topPages.length > 0 ? (
          <div>
            <h3 className="domain-deep-dive__section-title">
              {t('domainDeepDiveTopPages')}
            </h3>
            {detail.topPages.slice(0, 10).map((page) => (
              <div
                key={page.path}
                className={`domain-deep-dive__page-row${
                  focusedComparePaths.has(page.path)
                    ? ' domain-deep-dive__page-row--focused'
                    : ''
                }`}
              >
                <span className="domain-deep-dive__page-path">
                  {formatDomainPagePath(page.path)}
                </span>
                {focusedComparePaths.has(page.path) ? (
                  <span className="compare-set__landing-badge">
                    {t('compareSetFocusBadge')}
                  </span>
                ) : null}
                <span className="domain-deep-dive__page-count">
                  {formatNumber(page.visitCount)}
                </span>
              </div>
            ))}
          </div>
        ) : null}

        {detail.topReferrers.length > 0 ? (
          <div>
            <h3 className="domain-deep-dive__section-title">
              {t('domainDeepDiveReferrers')}
            </h3>
            {detail.topReferrers.slice(0, 5).map((referrer) => (
              <div key={referrer.domain} className="domain-deep-dive__flow-row">
                <Link
                  className="domain-deep-dive__flow-domain intelligence-link"
                  to={domainHref(referrer.domain)}
                >
                  {referrer.displayName ?? referrer.domain}
                </Link>
                <span className="domain-deep-dive__flow-count">
                  {formatNumber(referrer.count)}
                </span>
              </div>
            ))}
          </div>
        ) : null}

        {detail.topExits.length > 0 ? (
          <div>
            <h3 className="domain-deep-dive__section-title">
              {t('domainDeepDiveExits')}
            </h3>
            {detail.topExits.slice(0, 5).map((exit) => (
              <div key={exit.domain} className="domain-deep-dive__flow-row">
                <Link
                  className="domain-deep-dive__flow-domain intelligence-link"
                  to={domainHref(exit.domain)}
                >
                  {exit.displayName ?? exit.domain}
                </Link>
                <span className="domain-deep-dive__flow-count">
                  {formatNumber(exit.count)}
                </span>
              </div>
            ))}
          </div>
        ) : null}

        {detail.visitTrend.length > 0 ? (
          <div>
            <h3 className="domain-deep-dive__section-title">
              {t('domainDeepDiveTrend')}
            </h3>
            <div className="discovery-trend__chart" style={{ height: 80 }}>
              {detail.visitTrend.map((point: DomainTrendPoint) => {
                const max = Math.max(
                  ...detail.visitTrend.map(
                    (trendPoint: DomainTrendPoint) => trendPoint.visitCount,
                  ),
                  1,
                )
                return (
                  <Link
                    key={point.dateKey}
                    className="discovery-trend__bar-group"
                    to={dayHref(point.dateKey)}
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
                  </Link>
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

function canonicalPath(value: string | null | undefined) {
  if (!value) return null
  try {
    return new URL(value).pathname || '/'
  } catch {
    return null
  }
}

function parsePathFlowStepCount(flowId: string | null) {
  if (!flowId) return null
  const parts = flowId.split(':')
  const candidate = Number(parts.at(-2))
  return Number.isFinite(candidate) ? candidate : null
}
