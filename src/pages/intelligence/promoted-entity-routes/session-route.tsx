import { useParams } from 'react-router-dom'
import { ExplainabilityPanel } from '../../../components/intelligence/explainability-panel'
import { InsightEntityActions } from '../../../components/intelligence/entity-actions'
import { InsightEntityHero } from '../../../components/intelligence/entity-hero'
import { IntelligenceMetricGrid } from '../../../components/intelligence/metric-grid'
import { WorkbenchEntityRow } from '../../../components/intelligence/workbench'
import {
  useAsyncData,
  singleDayDateRange,
} from '../../../lib/core-intelligence'
import * as api from '../../../lib/core-intelligence/api'
import { formatDateTime } from '../../../lib/format'
import { useI18n } from '../../../lib/i18n/hooks'
import {
  dayInsightsHref,
  domainInsightsHref,
  evidenceHref,
} from '../../../lib/intelligence'
import { useIntelligenceRouteState } from '../route-state'
import { IntelligenceSectionBody } from '../sections/section-body'
import { localDateKeyFromIso, useScopeCallout } from './helpers'
import { ScopeCallout, TrailLinkCard } from './shared'

export function SessionInsightsRoutePage() {
  const { sessionId } = useParams<{ sessionId: string }>()
  const { language, t } = useI18n('intelligence')
  const { effectiveProfileId, withCurrentRouteSearch, dateRange, preset } =
    useIntelligenceRouteState()
  const { renderScopeCallout } = useScopeCallout()
  const scopeCallout = renderScopeCallout()
  const decodedSessionId = sessionId ? decodeURIComponent(sessionId) : null
  const { data, error, loading } = useAsyncData<Awaited<
    ReturnType<typeof api.getSessionDetail>
  > | null>(
    () =>
      decodedSessionId
        ? api.getSessionDetail(decodedSessionId)
        : Promise.resolve(null),
    [decodedSessionId],
  )
  const detail = data ?? null
  const sessionRange =
    detail && detail.session
      ? {
          start: localDateKeyFromIso(
            new Date(detail.session.firstVisitMs).toISOString(),
          ),
          end: localDateKeyFromIso(
            new Date(detail.session.lastVisitMs).toISOString(),
          ),
        }
      : dateRange

  return (
    <div className="intelligence-page">
      <ScopeCallout body={scopeCallout.body} title={scopeCallout.title} />
      {!decodedSessionId ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('sessionGroupEmpty')}</p>
        </div>
      ) : loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--card" />
      ) : error || !detail?.session ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">
            {error || t('sessionGroupEmpty')}
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
                      dateRange: sessionRange,
                    }),
                    label: t('entityOpenExplorer'),
                  },
                ]}
              />
            }
            backHref={`/intelligence${withCurrentRouteSearch({ focus: null })}`}
            backLabel={t('entityBackToOverview')}
            eyebrow={t('sessionRouteTitle')}
            subtitle={t('sessionRouteSubtitle')}
            title={detail.session.autoTitle ?? t('sessionUntitled')}
          />
          <IntelligenceMetricGrid
            className="day-insights__stats"
            items={[
              {
                icon: '📄',
                label: t('sessionVisitLabel'),
                value: detail.session.visitCount,
              },
              {
                icon: '🔍',
                label: t('sessionSearchLabel'),
                value: detail.session.searchCount,
              },
              {
                icon: '🌐',
                label: t('digestNewSites'),
                value: detail.session.domainCount,
              },
            ]}
          />
          <div className="intelligence-row intelligence-row--two-col">
            <section className="intelligence-section">
              <h2 className="intelligence-section__title">
                {t('sessionRouteVisitsTitle')}
              </h2>
              <IntelligenceSectionBody className="session-card__visits">
                {detail.visits.map((visit) => {
                  const dateKey = localDateKeyFromIso(
                    new Date(visit.visitTimeMs).toISOString(),
                  )
                  return (
                    <WorkbenchEntityRow
                      key={visit.visitId}
                      actions={
                        <InsightEntityActions
                          items={[
                            {
                              href: dayInsightsHref(
                                dateKey,
                                effectiveProfileId,
                              ),
                              label: dateKey,
                              style: 'text',
                            },
                            {
                              href: domainInsightsHref({
                                domain: visit.registrableDomain,
                                dateRange: singleDayDateRange(dateKey),
                                preset: 'custom',
                                profileId: effectiveProfileId,
                              }),
                              label: visit.registrableDomain,
                              style: 'text',
                            },
                          ]}
                        />
                      }
                      className="session-visit-row"
                      content={visit.title ?? visit.url}
                      contentClassName="session-visit-row__content"
                      meta={
                        formatDateTime(
                          new Date(visit.visitTimeMs).toISOString(),
                          language,
                        ) ?? new Date(visit.visitTimeMs).toISOString()
                      }
                      metaClassName="session-card__meta"
                    />
                  )
                })}
              </IntelligenceSectionBody>
            </section>
            <section className="intelligence-section">
              <h2 className="intelligence-section__title">
                {t('sessionRouteTrailsTitle')}
              </h2>
              {detail.trails.length === 0 ? (
                <div className="intelligence-empty">
                  <p className="intelligence-empty__text">
                    {t('trailGroupEmpty')}
                  </p>
                </div>
              ) : (
                <IntelligenceSectionBody className="trail-group-panel__list">
                  {detail.trails.map((trail) => (
                    <TrailLinkCard
                      dateRange={sessionRange}
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
            entityType="session"
            entityId={detail.session.sessionId}
            t={t}
          />
        </>
      )}
    </div>
  )
}
