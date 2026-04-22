import { useParams } from 'react-router-dom'
import { CompareSetPageList } from '../../../components/intelligence/compare-set-page-list'
import { ExplainabilityPanel } from '../../../components/intelligence/explainability-panel'
import { InsightEntityActions } from '../../../components/intelligence/entity-actions'
import { InsightEntityHero } from '../../../components/intelligence/entity-hero'
import { IntelligenceMetricGrid } from '../../../components/intelligence/metric-grid'
import { IntelligenceSectionMeta } from '../../../components/intelligence/section-meta'
import { TimeRangeSelector } from '../../../components/intelligence/time-range-selector'
import { useAsyncData } from '../../../lib/core-intelligence'
import * as api from '../../../lib/core-intelligence/api'
import {
  dayInsightsHref,
  domainInsightsHref,
  sessionInsightsHref,
  trailInsightsHref,
} from '../../../lib/core-intelligence/routes'
import { useI18n } from '../../../lib/i18n/hooks'
import { evidenceHref } from '../../../lib/intelligence-links'
import { useIntelligenceRouteState } from '../route-state'
import { IntelligenceSectionBody } from '../sections/section-body'
import { formatNumber } from '../sections/shared'
import { useScopeCallout } from './helpers'
import { ScopeCallout } from './shared'

export function CompareSetInsightsRoutePage() {
  const { compareSetId } = useParams<{ compareSetId: string }>()
  const { t } = useI18n('intelligence')
  const {
    dateRange,
    effectiveProfileId,
    preset,
    setCustomRange,
    setPreset,
    withCurrentRouteSearch,
  } = useIntelligenceRouteState()
  const { renderScopeCallout, scopeLabel } = useScopeCallout()
  const scopeCallout = renderScopeCallout()
  const decodedCompareSetId = compareSetId
    ? decodeURIComponent(compareSetId)
    : null
  const { data, error, loading } = useAsyncData<Awaited<
    ReturnType<typeof api.getCompareSetDetail>
  > | null>(
    () =>
      decodedCompareSetId
        ? api.getCompareSetDetail(
            decodedCompareSetId,
            dateRange,
            effectiveProfileId,
          )
        : Promise.resolve(null),
    [dateRange, decodedCompareSetId, effectiveProfileId],
  )
  const detail = data?.data ?? null
  const totalVisits =
    detail?.compareSet.pages.reduce((sum, page) => sum + page.visitCount, 0) ??
    0
  const distinctDomains = new Set(
    detail?.compareSet.pages.map((page) => page.registrableDomain) ?? [],
  ).size

  return (
    <div className="intelligence-page">
      <TimeRangeSelector
        key={`${preset}:${dateRange.start}:${dateRange.end}`}
        dateRange={dateRange}
        preset={preset}
        onPresetChange={setPreset}
        onCustomRange={setCustomRange}
        t={t}
      />
      <ScopeCallout body={scopeCallout.body} title={scopeCallout.title} />
      {!decodedCompareSetId ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('compareSetsEmpty')}</p>
        </div>
      ) : loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--card" />
      ) : error || !detail ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">
            {error || t('compareSetsEmpty')}
          </p>
        </div>
      ) : (
        <>
          <InsightEntityHero
            actions={
              <InsightEntityActions
                items={[
                  {
                    href: trailInsightsHref({
                      trailId: detail.compareSet.trailId,
                      dateRange,
                      preset,
                      profileId: effectiveProfileId,
                      focus: {
                        focusType: 'compare-set',
                        focusId: detail.compareSet.compareSetId,
                      },
                    }),
                    label: t('compareSetRouteOpenTrail'),
                  },
                  ...(detail.session
                    ? [
                        {
                          href: sessionInsightsHref({
                            sessionId: detail.session.sessionId,
                            dateRange,
                            preset,
                            profileId: effectiveProfileId,
                          }),
                          label: t('compareSetRouteOpenSession'),
                        },
                      ]
                    : []),
                  {
                    href: evidenceHref({
                      profileId: effectiveProfileId,
                      title: detail.compareSet.searchQuery,
                      dateRange,
                    }),
                    label: t('entityOpenExplorer'),
                  },
                ]}
              />
            }
            backHref={`/intelligence${withCurrentRouteSearch({ focus: null })}`}
            backLabel={t('entityBackToOverview')}
            eyebrow={t('compareSetRouteTitle')}
            subtitle={t('compareSetRouteSubtitle')}
            title={`"${detail.compareSet.searchQuery}"`}
          />
          {data ? (
            <IntelligenceSectionMeta meta={data.meta} scopeLabel={scopeLabel} />
          ) : null}
          <IntelligenceMetricGrid
            className="day-insights__stats"
            items={[
              {
                icon: '↔',
                label: t('compareSetsTitle'),
                value: detail.compareSet.pages.length,
              },
              {
                icon: '📄',
                label: t('sessionVisitLabel'),
                value: formatNumber(totalVisits),
              },
              {
                icon: '🌐',
                label: t('compareSetRouteDomainsLabel'),
                value: distinctDomains,
              },
            ]}
          />
          <div className="intelligence-row intelligence-row--two-col">
            <section className="intelligence-section">
              <h2 className="intelligence-section__title">
                {t('compareSetRoutePagesTitle')}
              </h2>
              <IntelligenceSectionBody>
                <CompareSetPageList
                  getHref={(page) =>
                    domainInsightsHref({
                      domain: page.registrableDomain,
                      dateRange,
                      preset,
                      profileId: effectiveProfileId,
                      focus: {
                        focusType: 'compare-set',
                        focusId: detail.compareSet.compareSetId,
                      },
                    })
                  }
                  keyPrefix={detail.compareSet.compareSetId}
                  landingLabel={t('compareSetsLanding')}
                  pages={detail.compareSet.pages}
                />
              </IntelligenceSectionBody>
            </section>
            <section className="intelligence-section">
              <h2 className="intelligence-section__title">
                {t('compareSetRouteRecentDaysTitle')}
              </h2>
              {detail.recentDays.length === 0 ? (
                <div className="intelligence-empty">
                  <p className="intelligence-empty__text">
                    {t('dayInsightsEmpty')}
                  </p>
                </div>
              ) : (
                <IntelligenceSectionBody className="settings-output-chip-list">
                  <InsightEntityActions
                    className="settings-output-chip-list"
                    items={detail.recentDays.map((dateKey) => ({
                      href: dayInsightsHref(dateKey, effectiveProfileId, {
                        focusType: 'compare-set',
                        focusId: detail.compareSet.compareSetId,
                      }),
                      key: dateKey,
                      label: dateKey,
                      style: 'chip',
                    }))}
                  />
                </IntelligenceSectionBody>
              )}
            </section>
          </div>
          <ExplainabilityPanel
            entityType="compare_set"
            entityId={detail.compareSet.compareSetId}
            t={t}
          />
        </>
      )}
    </div>
  )
}
