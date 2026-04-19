/**
 * Route-first promoted intelligence entity pages introduced in M7.
 *
 * Why this file exists:
 * - Query-family, refind-page, session, and trail are now shared destinations
 *   instead of explainability-only or evidence-only fragments.
 * - Grouping these route pages keeps the route-promotion work together while
 *   reusing the same hero/action/scope grammar.
 */

import { Link, useParams } from 'react-router-dom'
import { ExplainabilityPanel } from '../../components/intelligence/explainability-panel'
import { InsightEntityActions } from '../../components/intelligence/entity-actions'
import { InsightEntityHero } from '../../components/intelligence/entity-hero'
import { IntelligenceSectionMeta } from '../../components/intelligence/section-meta'
import { TimeRangeSelector } from '../../components/intelligence/time-range-selector'
import { StatusCallout } from '../../components/primitives/status-callout'
import {
  localDateKeyFromIso,
  type RefindScoreFactor,
  singleDayDateRange,
  useAsyncData,
  type DateRange,
  type TrailSummary,
} from '../../lib/core-intelligence'
import * as api from '../../lib/core-intelligence/api'
import { formatDateTime } from '../../lib/format'
import { useI18n } from '../../lib/i18n/hooks'
import {
  dayInsightsHref,
  domainInsightsHref,
  evidenceHref,
  sessionInsightsHref,
  trailInsightsHref,
} from '../../lib/intelligence'
import { intelligenceText } from './copy'
import { useIntelligenceRouteState } from './route-state'
import { IntelligenceSectionBody } from './sections/section-body'
import { formatNumber } from './sections/shared'

function ScopeCallout({ body, title }: { body: string; title: string }) {
  return <StatusCallout tone="info" title={title} body={body} />
}

function useScopeCallout() {
  const { language, t } = useI18n('intelligence')
  const { effectiveProfileId, profileScopeLabel } = useIntelligenceRouteState()
  const archiveWideBadge = intelligenceText(language, t, 'archiveWideBadge')
  const archiveWideBody = intelligenceText(language, t, 'archiveWideBody')

  return {
    effectiveProfileId,
    profileScopeLabel,
    renderScopeCallout: () => (
      <ScopeCallout
        body={
          effectiveProfileId
            ? t('scopedViewBody', {
                profile: profileScopeLabel ?? effectiveProfileId,
              })
            : archiveWideBody
        }
        title={effectiveProfileId ? t('scopedViewTitle') : archiveWideBadge}
      />
    ),
    scopeLabel: effectiveProfileId
      ? (profileScopeLabel ?? effectiveProfileId)
      : archiveWideBadge,
  }
}

function trailCard({
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

function normalizeRefindFactors(value: unknown): RefindScoreFactor[] {
  if (!Array.isArray(value)) {
    return []
  }

  return value.flatMap((entry) => {
    if (!entry || typeof entry !== 'object') {
      return []
    }

    const factor = entry as Record<string, unknown>
    return [
      {
        signal: typeof factor.signal === 'string' ? factor.signal : '',
        rawValue:
          typeof factor.rawValue === 'number' &&
          Number.isFinite(factor.rawValue)
            ? factor.rawValue
            : 0,
        weight:
          typeof factor.weight === 'number' && Number.isFinite(factor.weight)
            ? factor.weight
            : 0,
        contribution:
          typeof factor.contribution === 'number' &&
          Number.isFinite(factor.contribution)
            ? factor.contribution
            : 0,
      },
    ]
  })
}

export function QueryFamilyInsightsRoutePage() {
  const { familyId } = useParams<{ familyId: string }>()
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
      {renderScopeCallout()}
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
            backHref={`/intelligence${withCurrentRouteSearch()}`}
            backLabel={t('entityBackToOverview')}
            eyebrow={t('queryFamilyRouteTitle')}
            subtitle={t('queryFamilyRouteSubtitle')}
            title={`"${detail.family.anchorQuery}"`}
          />
          {data ? (
            <IntelligenceSectionMeta meta={data.meta} scopeLabel={scopeLabel} />
          ) : null}
          <div className="day-insights__stats">
            <div className="digest-card">
              <span className="digest-card__icon">🔍</span>
              <span className="digest-card__value">
                {formatNumber(detail.family.memberCount)}
              </span>
              <span className="digest-card__label">
                {t('queryFamilyMemberCount')}
              </span>
            </div>
            <div className="digest-card">
              <span className="digest-card__icon">🧭</span>
              <span className="digest-card__value">
                {formatNumber(detail.relatedTrails.length)}
              </span>
              <span className="digest-card__label">
                {t('queryFamilyRelatedTrails')}
              </span>
            </div>
          </div>
          <div className="intelligence-row intelligence-row--two-col">
            <section className="intelligence-section">
              <h2 className="intelligence-section__title">
                {t('queryFamilyQueriesTitle')}
              </h2>
              <IntelligenceSectionBody className="query-families">
                <div className="query-family-card">
                  <div className="query-family-card__header">
                    <span className="query-family-card__engine">
                      {detail.family.searchEngine}
                    </span>
                    <span className="query-family-card__dates">
                      {detail.family.firstSeenAt} - {detail.family.lastSeenAt}
                    </span>
                  </div>
                  <div className="query-family-card__members">
                    {detail.family.queries.map((query, index) => (
                      <span
                        key={`${detail.family.familyId}:${query}:${index}`}
                        className="query-family-card__member"
                      >
                        "{query}"
                      </span>
                    ))}
                  </div>
                </div>
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
                  {detail.relatedTrails.map((trail) =>
                    trailCard({
                      dateRange,
                      preset,
                      profileId: effectiveProfileId,
                      t,
                      trail,
                    }),
                  )}
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
      {renderScopeCallout()}
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
            backHref={`/intelligence${withCurrentRouteSearch()}`}
            backLabel={t('entityBackToOverview')}
            eyebrow={t('refindRouteTitle')}
            subtitle={t('refindRouteSubtitle')}
            title={detail.page.title ?? detail.page.url}
          />
          {data ? (
            <IntelligenceSectionMeta meta={data.meta} scopeLabel={scopeLabel} />
          ) : null}
          <div className="day-insights__stats">
            <div className="digest-card">
              <span className="digest-card__icon">🔄</span>
              <span className="digest-card__value">
                {detail.page.crossDayCount}
              </span>
              <span className="digest-card__label">
                {t('refindFactorDays')}
              </span>
            </div>
            <div className="digest-card">
              <span className="digest-card__icon">🧭</span>
              <span className="digest-card__value">
                {detail.page.trailCount}
              </span>
              <span className="digest-card__label">
                {t('refindFactorTrails')}
              </span>
            </div>
            <div className="digest-card">
              <span className="digest-card__icon">⭐</span>
              <span className="digest-card__value">
                {detail.page.refindScore.toFixed(1)}
              </span>
              <span className="digest-card__label">{t('refindScore')}</span>
            </div>
          </div>
          <div className="intelligence-row intelligence-row--two-col">
            <section className="intelligence-section">
              <h2 className="intelligence-section__title">
                {t('refindFactorsTitle')}
              </h2>
              <IntelligenceSectionBody className="refind-card__factors">
                {refindFactors.map((factor, index) => (
                  <div
                    key={`${factor.signal}:${index}`}
                    className="refind-card__factor"
                  >
                    <span className="refind-card__factor-label">
                      {factor.signal}
                    </span>
                    <span className="refind-card__factor-value">
                      {factor.rawValue} ×{factor.weight}
                    </span>
                  </div>
                ))}
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
                {detail.relatedTrails.map((trail) =>
                  trailCard({
                    dateRange,
                    preset,
                    profileId: effectiveProfileId,
                    t,
                    trail,
                  }),
                )}
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

export function SessionInsightsRoutePage() {
  const { sessionId } = useParams<{ sessionId: string }>()
  const { language, t } = useI18n('intelligence')
  const { effectiveProfileId, withCurrentRouteSearch, dateRange, preset } =
    useIntelligenceRouteState()
  const { renderScopeCallout } = useScopeCallout()
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
      {renderScopeCallout()}
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
            backHref={`/intelligence${withCurrentRouteSearch()}`}
            backLabel={t('entityBackToOverview')}
            eyebrow={t('sessionRouteTitle')}
            subtitle={t('sessionRouteSubtitle')}
            title={detail.session.autoTitle ?? t('sessionUntitled')}
          />
          <div className="day-insights__stats">
            <div className="digest-card">
              <span className="digest-card__icon">📄</span>
              <span className="digest-card__value">
                {detail.session.visitCount}
              </span>
              <span className="digest-card__label">
                {t('sessionVisitLabel')}
              </span>
            </div>
            <div className="digest-card">
              <span className="digest-card__icon">🔍</span>
              <span className="digest-card__value">
                {detail.session.searchCount}
              </span>
              <span className="digest-card__label">
                {t('sessionSearchLabel')}
              </span>
            </div>
            <div className="digest-card">
              <span className="digest-card__icon">🌐</span>
              <span className="digest-card__value">
                {detail.session.domainCount}
              </span>
              <span className="digest-card__label">{t('digestNewSites')}</span>
            </div>
          </div>
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
                    <div key={visit.visitId} className="session-visit-row">
                      <span className="session-visit-row__content">
                        {visit.title ?? visit.url}
                        <div className="session-card__meta">
                          {formatDateTime(
                            new Date(visit.visitTimeMs).toISOString(),
                            language,
                          ) ?? new Date(visit.visitTimeMs).toISOString()}
                        </div>
                      </span>
                      <InsightEntityActions
                        items={[
                          {
                            href: dayInsightsHref(dateKey, effectiveProfileId),
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
                    </div>
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
                  {detail.trails.map((trail) =>
                    trailCard({
                      dateRange: sessionRange,
                      preset,
                      profileId: effectiveProfileId,
                      t,
                      trail,
                    }),
                  )}
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

export function TrailInsightsRoutePage() {
  const { trailId } = useParams<{ trailId: string }>()
  const { t } = useI18n('intelligence')
  const { effectiveProfileId, withCurrentRouteSearch, dateRange, preset } =
    useIntelligenceRouteState()
  const { renderScopeCallout } = useScopeCallout()
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

  return (
    <div className="intelligence-page">
      {renderScopeCallout()}
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
            backHref={`/intelligence${withCurrentRouteSearch()}`}
            backLabel={t('entityBackToOverview')}
            eyebrow={t('trailRouteTitle')}
            subtitle={t('trailRouteSubtitle')}
            title={`"${detail.trail.initialQuery}"`}
          />
          <div className="day-insights__stats">
            <div className="digest-card">
              <span className="digest-card__icon">🔍</span>
              <span className="digest-card__value">
                {detail.trail.visitCount}
              </span>
              <span className="digest-card__label">
                {t('sessionVisitLabel')}
              </span>
            </div>
            <div className="digest-card">
              <span className="digest-card__icon">🪜</span>
              <span className="digest-card__value">
                {detail.trail.reformulationCount}
              </span>
              <span className="digest-card__label">
                {t('trailReformulation')}
              </span>
            </div>
            <div className="digest-card">
              <span className="digest-card__icon">🌐</span>
              <span className="digest-card__value">
                {detail.trail.maxDepth}
              </span>
              <span className="digest-card__label">
                {t('trailRouteDepthLabel')}
              </span>
            </div>
          </div>
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
                  return (
                    <div key={member.visitId} className="trail-member-row">
                      <span className="trail-member-row__content">
                        {member.role === 'search_event' && member.searchQuery
                          ? `"${member.searchQuery}"`
                          : (member.title ?? member.url)}
                      </span>
                      {member.registrableDomain ? (
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
                                domain: member.registrableDomain,
                                dateRange: singleDayDateRange(dateKey),
                                preset: 'custom',
                                profileId: effectiveProfileId,
                              }),
                              label: member.registrableDomain,
                              style: 'text',
                            },
                          ]}
                        />
                      ) : null}
                    </div>
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
