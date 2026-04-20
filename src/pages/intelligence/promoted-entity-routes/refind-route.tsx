import { useParams } from 'react-router-dom'
import { ExplainabilityPanel } from '../../../components/intelligence/explainability-panel'
import { InsightEntityActions } from '../../../components/intelligence/entity-actions'
import { InsightEntityHero } from '../../../components/intelligence/entity-hero'
import { IntelligenceMetricGrid } from '../../../components/intelligence/metric-grid'
import { IntelligenceSectionMeta } from '../../../components/intelligence/section-meta'
import { TimeRangeSelector } from '../../../components/intelligence/time-range-selector'
import { useAsyncData } from '../../../lib/core-intelligence'
import * as api from '../../../lib/core-intelligence/api'
import { useI18n } from '../../../lib/i18n/hooks'
import {
  dayInsightsHref,
  domainInsightsHref,
  evidenceHref,
} from '../../../lib/intelligence'
import { useIntelligenceRouteState } from '../route-state'
import { IntelligenceSectionBody } from '../sections/section-body'
import { normalizeRefindFactors, useScopeCallout } from './helpers'
import { RefindFactorSection, ScopeCallout, TrailLinkCard } from './shared'

export function RefindPageInsightsRoutePage() {
  const { canonicalUrl } = useParams<{ canonicalUrl: string }>()
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
  const decodedUrl = canonicalUrl ? decodeURIComponent(canonicalUrl) : null

  const { data, error, loading } = useAsyncData<Awaited<
    ReturnType<typeof api.getRefindPageDetail>
  > | null>(
    () =>
      decodedUrl
        ? api.getRefindPageDetail(decodedUrl, dateRange, effectiveProfileId)
        : Promise.resolve(null),
    [dateRange, decodedUrl, effectiveProfileId],
  )

  const detail = data?.data ?? null
  const refindFactors = normalizeRefindFactors(detail?.explanation.factors)

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
      {!decodedUrl ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('refindEmpty')}</p>
        </div>
      ) : loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--card" />
      ) : error || !detail ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">
            {error || t('refindEmpty')}
          </p>
        </div>
      ) : (
        <>
          <InsightEntityHero
            actions={
              <InsightEntityActions
                items={[
                  {
                    href: evidenceHref({
                      profileId: effectiveProfileId,
                      domain: detail.page.registrableDomain,
                      url: detail.page.canonicalUrl,
                      dateRange,
                    }),
                    label: t('entityOpenExplorer'),
                  },
                  {
                    href: domainInsightsHref({
                      domain: detail.page.registrableDomain,
                      dateRange,
                      preset,
                      profileId: effectiveProfileId,
                    }),
                    label: t('openDomainInsights'),
                  },
                ]}
              />
            }
            backHref={`/intelligence${withCurrentRouteSearch({ focus: null })}`}
            backLabel={t('entityBackToOverview')}
            eyebrow={t('refindRouteTitle')}
            subtitle={t('refindRouteSubtitle')}
            title={detail.page.title ?? detail.page.url}
          />
          {data ? (
            <IntelligenceSectionMeta meta={data.meta} scopeLabel={scopeLabel} />
          ) : null}
          <IntelligenceMetricGrid
            className="day-insights__stats"
            items={[
              {
                icon: '🔄',
                label: t('refindFactorDays'),
                value: detail.page.crossDayCount,
              },
              {
                icon: '🧭',
                label: t('refindFactorTrails'),
                value: detail.page.trailCount,
              },
              {
                icon: '⭐',
                label: t('refindScore'),
                value: detail.page.refindScore.toFixed(1),
              },
            ]}
          />
          <div className="intelligence-row intelligence-row--two-col">
            <section className="intelligence-section">
              <h2 className="intelligence-section__title">
                {t('refindFactorsTitle')}
              </h2>
              <IntelligenceSectionBody>
                <RefindFactorSection factors={refindFactors} />
              </IntelligenceSectionBody>
            </section>
            <section className="intelligence-section">
              <h2 className="intelligence-section__title">
                {t('refindRecentDaysTitle')}
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
                      href: dayInsightsHref(dateKey, effectiveProfileId),
                      key: dateKey,
                      label: dateKey,
                      style: 'chip',
                    }))}
                  />
                </IntelligenceSectionBody>
              )}
            </section>
          </div>
          <section className="intelligence-section">
            <h2 className="intelligence-section__title">
              {t('queryFamilyTrailsTitle')}
            </h2>
            {detail.relatedTrails.length === 0 ? (
              <div className="intelligence-empty">
                <p className="intelligence-empty__text">
                  {t('trailGroupEmpty')}
                </p>
              </div>
            ) : (
              <IntelligenceSectionBody className="trail-group-panel__list">
                {detail.relatedTrails.map((trail) => (
                  <TrailLinkCard
                    dateRange={dateRange}
                    key={trail.trailId}
                    preset={preset}
                    profileId={effectiveProfileId}
                    t={t}
                    trail={trail}
                  />
                ))}
              </IntelligenceSectionBody>
            )}
          </section>
          <ExplainabilityPanel
            entityType="refind_page"
            entityId={detail.page.canonicalUrl}
            explanation={{
              entityType: 'refind_page',
              entityId: detail.page.canonicalUrl,
              triggerRule: `Refind score >= ${detail.explanation.refindScore.toFixed(1)}`,
              factors: detail.explanation.factors.map((factor) => ({
                label: factor.signal,
                rawValue: factor.rawValue,
                weight: factor.weight,
                contribution: factor.contribution,
              })),
              participatingVisitIds: detail.explanation.visitIds,
            }}
            t={t}
          />
        </>
      )}
    </div>
  )
}
