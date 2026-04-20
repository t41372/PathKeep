import { Link, useParams } from 'react-router-dom'
import { ExplainabilityPanel } from '../../../components/intelligence/explainability-panel'
import { InsightEntityActions } from '../../../components/intelligence/entity-actions'
import { InsightEntityHero } from '../../../components/intelligence/entity-hero'
import { IntelligenceMetricGrid } from '../../../components/intelligence/metric-grid'
import { WorkbenchEntityRow } from '../../../components/intelligence/workbench'
import { StatusCallout } from '../../../components/primitives/status-callout'
import {
  singleDayDateRange,
  useAsyncData,
} from '../../../lib/core-intelligence'
import * as api from '../../../lib/core-intelligence/api'
import { useI18n } from '../../../lib/i18n/hooks'
import {
  compareSetInsightsHref,
  dayInsightsHref,
  domainInsightsHref,
  evidenceHref,
  sessionInsightsHref,
} from '../../../lib/intelligence'
import { useIntelligenceRouteState } from '../route-state'
import { IntelligenceSectionBody } from '../sections/section-body'
import {
  useFocusedCompareSet,
  localDateKeyFromIso,
  useScopeCallout,
} from './helpers'
import { ScopeCallout } from './shared'

export function TrailInsightsRoutePage() {
  const { trailId } = useParams<{ trailId: string }>()
  const { t } = useI18n('intelligence')
  const {
    effectiveProfileId,
    focus,
    withCurrentRouteSearch,
    dateRange,
    preset,
  } = useIntelligenceRouteState()
  const { renderScopeCallout } = useScopeCallout()
  const scopeCallout = renderScopeCallout()
  const decodedTrailId = trailId ? decodeURIComponent(trailId) : null
  const { data, error, loading } = useAsyncData<Awaited<
    ReturnType<typeof api.getTrailDetail>
  > | null>(
    () =>
      decodedTrailId
        ? api.getTrailDetail(decodedTrailId)
        : Promise.resolve(null),
    [decodedTrailId],
  )
  const detail = data ?? null
  const trailRange = detail?.trail
    ? {
        start: localDateKeyFromIso(
          new Date(detail.trail.firstVisitMs).toISOString(),
        ),
        end: localDateKeyFromIso(
          new Date(detail.trail.lastVisitMs).toISOString(),
        ),
      }
    : dateRange
  const focusedCompareSetId =
    focus?.focusType === 'compare-set' ? focus.focusId : null
  const focusedCompareSetResult = useFocusedCompareSet(
    focusedCompareSetId,
    dateRange,
    effectiveProfileId,
  )
  const focusedCompareSetCandidate = focusedCompareSetResult.data?.data ?? null
  const focusedCompareSet =
    focusedCompareSetCandidate?.compareSet.trailId === detail?.trail?.trailId
      ? focusedCompareSetCandidate
      : null
  const focusedCompareSetPages = new Set(
    focusedCompareSet?.compareSet.pages.map((page) => page.canonicalUrl) ?? [],
  )

  return (
    <div className="intelligence-page">
      <ScopeCallout body={scopeCallout.body} title={scopeCallout.title} />
      {!decodedTrailId ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('trailGroupEmpty')}</p>
        </div>
      ) : loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--card" />
      ) : error || !detail?.trail ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">
            {error || t('trailGroupEmpty')}
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
                      title: detail.trail.initialQuery,
                      dateRange: trailRange,
                    }),
                    label: t('entityOpenExplorer'),
                  },
                  ...(detail.trail.sessionId
                    ? [
                        {
                          href: sessionInsightsHref({
                            sessionId: detail.trail.sessionId,
                            dateRange: trailRange,
                            preset,
                            profileId: effectiveProfileId,
                          }),
                          label: t('trailRouteOpenSession'),
                        },
                      ]
                    : []),
                ]}
              />
            }
            backHref={`/intelligence${withCurrentRouteSearch({ focus: null })}`}
            backLabel={t('entityBackToOverview')}
            eyebrow={t('trailRouteTitle')}
            subtitle={t('trailRouteSubtitle')}
            title={`"${detail.trail.initialQuery}"`}
          />
          {focusedCompareSet ? (
            <StatusCallout
              tone="info"
              title={t('compareSetFocusTitle')}
              body={t('compareSetFocusBody', {
                query: focusedCompareSet.compareSet.searchQuery,
                count: focusedCompareSet.compareSet.pages.length,
              })}
              actions={
                <Link
                  className="btn-secondary"
                  to={compareSetInsightsHref({
                    compareSetId: focusedCompareSet.compareSet.compareSetId,
                    dateRange,
                    preset,
                    profileId: effectiveProfileId,
                  })}
                >
                  {t('compareSetRouteTitle')}
                </Link>
              }
            />
          ) : null}
          <IntelligenceMetricGrid
            className="day-insights__stats"
            items={[
              {
                icon: '🔍',
                label: t('sessionVisitLabel'),
                value: detail.trail.visitCount,
              },
              {
                icon: '🪜',
                label: t('trailReformulation'),
                value: detail.trail.reformulationCount,
              },
              {
                icon: '🌐',
                label: t('trailRouteDepthLabel'),
                value: detail.trail.maxDepth,
              },
            ]}
          />
          <div className="intelligence-row intelligence-row--two-col">
            <section className="intelligence-section">
              <h2 className="intelligence-section__title">
                {t('trailEvolution')}
              </h2>
              <IntelligenceSectionBody className="trail-card__evolution-chain">
                {detail.trail.queries.map((query, index) => (
                  <span
                    key={`${detail.trail.trailId}:${query}:${index}`}
                    className="trail-card__evolution-step"
                  >
                    {index > 0 ? (
                      <span className="trail-card__evolution-arrow">→</span>
                    ) : null}
                    <span className="trail-card__evolution-query">
                      "{query}"
                    </span>
                  </span>
                ))}
              </IntelligenceSectionBody>
            </section>
            <section className="intelligence-section">
              <h2 className="intelligence-section__title">
                {t('trailRouteMembersTitle')}
              </h2>
              <IntelligenceSectionBody className="trail-card__members">
                {detail.members.map((member) => {
                  const dateKey = localDateKeyFromIso(
                    new Date(member.visitTimeMs).toISOString(),
                  )
                  const focusedMember = Boolean(
                    member.canonicalUrl &&
                    focusedCompareSetPages.has(member.canonicalUrl),
                  )
                  return (
                    <WorkbenchEntityRow
                      key={member.visitId}
                      actions={
                        member.registrableDomain ? (
                          <InsightEntityActions
                            items={[
                              {
                                href: dayInsightsHref(
                                  dateKey,
                                  effectiveProfileId,
                                  focusedMember ? focus : null,
                                ),
                                label: dateKey,
                                style: 'text',
                              },
                              {
                                href: domainInsightsHref({
                                  domain: member.registrableDomain,
                                  dateRange: singleDayDateRange(dateKey),
                                  preset: 'custom',
                                  profileId: effectiveProfileId,
                                  focus: focusedMember ? focus : null,
                                }),
                                label: member.registrableDomain,
                                style: 'text',
                              },
                            ]}
                          />
                        ) : null
                      }
                      className={`trail-member-row${
                        focusedMember ? ' trail-member-row--focused' : ''
                      }`}
                      content={
                        member.role === 'search_event' && member.searchQuery
                          ? `"${member.searchQuery}"`
                          : (member.title ?? member.url)
                      }
                      contentClassName="trail-member-row__content"
                    />
                  )
                })}
              </IntelligenceSectionBody>
            </section>
          </div>
          <ExplainabilityPanel
            entityType="search_trail"
            entityId={detail.trail.trailId}
            t={t}
          />
        </>
      )}
    </div>
  )
}
