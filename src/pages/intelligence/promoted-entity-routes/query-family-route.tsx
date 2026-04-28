import { useParams } from 'react-router-dom'
import { ExplainabilityPanel } from '../../../components/intelligence/explainability-panel'
import { InsightEntityActions } from '../../../components/intelligence/entity-actions'
import { InsightEntityHero } from '../../../components/intelligence/entity-hero'
import { IntelligenceMetricGrid } from '../../../components/intelligence/metric-grid'
import { QueryFamilyCard } from '../../../components/intelligence/query-family-card'
import { IntelligenceSectionMeta } from '../../../components/intelligence/section-meta'
import { TimeRangeSelector } from '../../../components/intelligence/time-range-selector'
import { useAsyncData } from '../../../lib/core-intelligence'
import * as api from '../../../lib/core-intelligence/api'
import { formatDateTime } from '../../../lib/format'
import { useI18n } from '../../../lib/i18n/hooks'
import { evidenceHref } from '../../../lib/intelligence-links'
import { useIntelligenceRouteState } from '../route-state'
import { IntelligenceSectionBody } from '../sections/section-body'
import { formatNumber } from '../sections/shared'
import { useScopeCallout } from './helpers'
import { ScopeCallout, TrailLinkCard } from './shared'

export function QueryFamilyInsightsRoutePage() {
  const { familyId } = useParams<{ familyId: string }>()
  const { language, t } = useI18n('intelligence')
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
  const decodedFamilyId = familyId ? decodeURIComponent(familyId) : null

  const { data, error, loading } = useAsyncData<Awaited<
    ReturnType<typeof api.getQueryFamilyDetail>
  > | null>(
    () =>
      decodedFamilyId
        ? api.getQueryFamilyDetail(
            decodedFamilyId,
            dateRange,
            effectiveProfileId,
          )
        : Promise.resolve(null),
    [dateRange, decodedFamilyId, effectiveProfileId],
  )
  const detail = data?.data ?? null
  const explorerHref = detail
    ? evidenceHref({
        profileId: effectiveProfileId,
        title: detail.family.anchorQuery,
        dateRange,
      })
    : evidenceHref({
        profileId: effectiveProfileId,
        title: decodedFamilyId ?? '',
        dateRange,
      })
  const firstSeenLabel = detail
    ? (formatDateTime(detail.family.firstSeenAt, language) ??
      detail.family.firstSeenAt)
    : null
  const lastSeenLabel = detail
    ? (formatDateTime(detail.family.lastSeenAt, language) ??
      detail.family.lastSeenAt)
    : null

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
      {!decodedFamilyId ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">
            {t('queryFamiliesPlaceholder')}
          </p>
        </div>
      ) : loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--card" />
      ) : error || !detail ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">
            {error || t('queryFamiliesPlaceholder')}
          </p>
        </div>
      ) : (
        <>
          <InsightEntityHero
            actions={
              <InsightEntityActions
                items={[
                  {
                    href: explorerHref,
                    label: t('entityOpenExplorer'),
                  },
                ]}
              />
            }
            backHref={`/intelligence${withCurrentRouteSearch({ focus: null })}`}
            backLabel={t('entityBackToOverview')}
            eyebrow={t('queryFamilyRouteTitle')}
            subtitle={t('queryFamilyRouteSubtitle')}
            title={`"${detail.family.anchorQuery}"`}
          />
          <IntelligenceSectionMeta meta={data!.meta} scopeLabel={scopeLabel} />
          <IntelligenceMetricGrid
            className="day-insights__stats"
            items={[
              {
                icon: '🔍',
                label: t('queryFamilyMemberCount'),
                value: detail.family.memberCount,
              },
              {
                icon: '🧭',
                label: t('searchQueriesEngineFilter'),
                value: detail.family.searchEngine,
              },
              {
                icon: '🪜',
                label: t('trailEvolution'),
                value: formatNumber(detail.family.queries.length),
              },
            ]}
          />
          <div className="intelligence-row intelligence-row--two-col">
            <section className="intelligence-section">
              <h2 className="intelligence-section__title">
                {t('queryFamilyQueriesTitle')}
              </h2>
              <IntelligenceSectionBody className="query-families">
                <QueryFamilyCard
                  family={detail.family}
                  footer={
                    <span className="query-family-card__dates">
                      {firstSeenLabel} - {lastSeenLabel}
                    </span>
                  }
                  href={explorerHref}
                  linkMode="anchor"
                  memberCountLabel={t('queryFamilyMemberCount')}
                  showDates={false}
                />
              </IntelligenceSectionBody>
            </section>
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
          </div>
          <ExplainabilityPanel
            entityType="query_family"
            entityId={detail.family.familyId}
            t={t}
          />
        </>
      )}
    </div>
  )
}
