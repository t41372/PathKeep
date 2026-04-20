/**
 * Day insights route and presentational surface.
 *
 * Why this file exists:
 * - Local-calendar-day insights are now a first-class route under
 *   `/intelligence/day/:date`.
 * - The route owns the full day entity review surface so overview cards can
 *   stay navigation-first instead of rebuilding inline detail everywhere.
 */

import { useMemo } from 'react'
import { Link, useParams, useSearchParams } from 'react-router-dom'
import { InsightEntityActions } from '../../components/intelligence/entity-actions'
import { InsightEntityHero } from '../../components/intelligence/entity-hero'
import { IntelligenceMetricGrid } from '../../components/intelligence/metric-grid'
import { QueryFamilyCard } from '../../components/intelligence/query-family-card'
import { IntelligenceSectionMeta } from '../../components/intelligence/section-meta'
import { StatusCallout } from '../../components/primitives/status-callout'
import {
  singleDayDateRange,
  useAsyncData,
  type DayInsightsHourlyBucket,
} from '../../lib/core-intelligence'
import * as api from '../../lib/core-intelligence/api'
import { useI18n } from '../../lib/i18n/hooks'
import {
  domainDayInsightsHref,
  evidenceHref,
  queryFamilyInsightsHref,
  refindInsightsHref,
} from '../../lib/intelligence'
import {
  profileIdLabel,
  useProfileScope,
} from '../../lib/profile-scope-context'
import {
  parseInsightRouteFocus,
  buildIntelligenceSearchParams,
  isLocalDateKey,
} from '../../lib/core-intelligence/routes'
import { intelligenceCategoryLabel, intelligenceText } from './copy'
import { IntelligenceSectionBody } from './sections/section-body'
import { formatHourRange, formatNumber } from './sections/shared'

interface DayInsightsPageProps {
  backHref: string
  date: string
  focus: ReturnType<typeof parseInsightRouteFocus>
  profileId: string | null
  scopeLabel: string
}

export function DayInsightsRoutePage() {
  const { date } = useParams<{ date: string }>()
  const [searchParams] = useSearchParams()
  const { activeProfileId } = useProfileScope()
  const { language, t } = useI18n('intelligence')
  const effectiveProfileId = searchParams.get('profileId') ?? activeProfileId
  const focus = parseInsightRouteFocus(searchParams)

  if (!date || !isLocalDateKey(date)) {
    return (
      <div
        className="intelligence-page day-insights"
        data-testid="day-insights-page"
      >
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">
            {t('dayInsightsInvalidDate')}
          </p>
        </div>
      </div>
    )
  }

  const scopeLabel = effectiveProfileId
    ? profileIdLabel(effectiveProfileId)
    : intelligenceText(language, t, 'archiveWideBadge')
  const backParams = buildIntelligenceSearchParams({
    dateRange: singleDayDateRange(date),
    preset: 'custom',
    profileId: effectiveProfileId,
  })
  const backQuery = backParams.toString()
  const backHref = `/intelligence${backQuery ? `?${backQuery}` : ''}`

  return (
    <div
      className="intelligence-page day-insights"
      data-testid="day-insights-page"
    >
      <StatusCallout
        tone="info"
        title={effectiveProfileId ? t('scopedViewTitle') : scopeLabel}
        body={
          effectiveProfileId
            ? t('scopedViewBody', {
                profile: scopeLabel,
              })
            : intelligenceText(language, t, 'archiveWideBody')
        }
      />
      <DayInsightsPage
        backHref={backHref}
        date={date}
        focus={focus}
        profileId={effectiveProfileId}
        scopeLabel={scopeLabel}
      />
    </div>
  )
}

export function DayInsightsPage({
  backHref,
  date,
  focus,
  profileId,
  scopeLabel,
}: DayInsightsPageProps) {
  const { language, t } = useI18n('intelligence')
  const { data, loading, error } = useAsyncData(
    () => api.getDayInsights(date, profileId),
    [date, profileId],
  )
  const focusedCompareSetId =
    focus?.focusType === 'compare-set' ? focus.focusId : null
  const focusedCompareSetResult = useAsyncData<Awaited<
    ReturnType<typeof api.getCompareSetDetail>
  > | null>(
    () =>
      focusedCompareSetId
        ? api.getCompareSetDetail(
            focusedCompareSetId,
            singleDayDateRange(date),
            profileId,
          )
        : Promise.resolve(null),
    [date, focusedCompareSetId, profileId],
  )
  const detail = data?.data ?? null
  const focusedCompareSetCandidate = focusedCompareSetResult.data?.data ?? null
  const focusedCompareSet = focusedCompareSetCandidate?.recentDays.includes(
    date,
  )
    ? focusedCompareSetCandidate
    : null
  const explorerHref = detail
    ? evidenceHref({
        profileId,
        dateRange: detail.drilldown.explorerDateRange,
      })
    : evidenceHref({
        profileId,
        dateRange: singleDayDateRange(date),
      })

  if (loading) {
    return (
      <div className="intelligence-page day-insights">
        <div className="intelligence-skeleton intelligence-skeleton--card" />
      </div>
    )
  }

  if (error || !data || !detail) {
    return (
      <div className="intelligence-page day-insights">
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">
            {error || t('dayInsightsEmpty')}
          </p>
        </div>
      </div>
    )
  }

  return (
    <div className="intelligence-page day-insights">
      <InsightEntityHero
        actions={
          <InsightEntityActions
            items={[
              {
                href: explorerHref,
                label: t('dayInsightsOpenExplorer'),
              },
            ]}
          />
        }
        backHref={backHref}
        backLabel={t('dayInsightsBack')}
        eyebrow={t('dayInsightsTitle')}
        subtitle={t('dayInsightsSubtitle')}
        title={date}
      />

      <IntelligenceSectionMeta meta={data.meta} scopeLabel={scopeLabel} />
      {focusedCompareSet ? (
        <StatusCallout
          tone="info"
          title={t('compareSetFocusTitle')}
          body={t('compareSetDayFocusBody', {
            query: focusedCompareSet.compareSet.searchQuery,
            count: focusedCompareSet.compareSet.pages.length,
          })}
        />
      ) : null}

      <IntelligenceMetricGrid
        className="day-insights__stats"
        items={[
          {
            icon: '📊',
            label: t('digestVisits'),
            value: formatNumber(detail.digestSummary.totalVisits.value),
          },
          {
            icon: '🔍',
            label: t('digestSearches'),
            value: formatNumber(detail.digestSummary.totalSearches.value),
          },
          {
            icon: '🌐',
            label: t('digestNewSites'),
            value: formatNumber(detail.digestSummary.newDomains.value),
          },
          {
            icon: '📖',
            label: t('digestDeepRead'),
            value: formatNumber(detail.digestSummary.deepReadPages.value),
          },
        ]}
      />

      <div className="intelligence-row intelligence-row--two-col">
        <section className="intelligence-section">
          <h2 className="intelligence-section__title">
            {t('dayInsightsHourlyTitle')}
          </h2>
          <IntelligenceSectionBody>
            <DayInsightsHourStrip
              date={detail.date}
              hourly={detail.hourlyActivity}
              t={t}
            />
          </IntelligenceSectionBody>
        </section>

        <section className="intelligence-section">
          <h2 className="intelligence-section__title">
            {t('dayInsightsTopSitesTitle')}
          </h2>
          {detail.topSites.length === 0 ? (
            <div className="intelligence-empty">
              <p className="intelligence-empty__text">{t('topSitesEmpty')}</p>
            </div>
          ) : (
            <IntelligenceSectionBody className="top-sites-list">
              {detail.topSites.map((site, index) => (
                <Link
                  key={site.registrableDomain}
                  className="top-site-row top-site-row--interactive"
                  to={domainDayInsightsHref(
                    site.registrableDomain,
                    detail.date,
                    profileId,
                  )}
                >
                  <span className="top-site-row__rank">{index + 1}.</span>
                  <span className="top-site-row__domain">
                    {site.displayName ?? site.registrableDomain}
                  </span>
                  <span className="top-site-row__bar">
                    <span
                      className="top-site-row__bar-fill"
                      style={{
                        width: `${Math.round(
                          (site.visitCount /
                            Math.max(detail.topSites[0]?.visitCount ?? 1, 1)) *
                            100,
                        )}%`,
                      }}
                    />
                  </span>
                  <span className="top-site-row__count">
                    {formatNumber(site.visitCount)} {t('visits')}
                  </span>
                </Link>
              ))}
            </IntelligenceSectionBody>
          )}
        </section>
      </div>

      <div className="intelligence-secondary-grid day-insights__secondary-grid">
        <section className="intelligence-section">
          <h2 className="intelligence-section__title">
            {t('dayInsightsActivityMixTitle')}
          </h2>
          {detail.activityMix.categories.length === 0 ? (
            <div className="intelligence-empty">
              <p className="intelligence-empty__text">
                {t('activityMixEmpty')}
              </p>
            </div>
          ) : (
            <IntelligenceSectionBody className="activity-mix">
              {detail.activityMix.categories.map((category) => (
                <div
                  key={category.domainCategory}
                  className="activity-mix__row"
                >
                  <div className="activity-mix__summary">
                    <span className="activity-mix__category">
                      {intelligenceCategoryLabel(
                        language,
                        t,
                        category.domainCategory,
                      )}
                    </span>
                    <span className="activity-mix__share">
                      {Math.round(category.share * 100)}%
                    </span>
                  </div>
                  <span className="activity-mix__bar">
                    <span
                      className="activity-mix__bar-fill"
                      style={{ width: `${Math.round(category.share * 100)}%` }}
                      data-category={category.domainCategory}
                    />
                  </span>
                </div>
              ))}
            </IntelligenceSectionBody>
          )}
        </section>

        <section className="intelligence-section">
          <h2 className="intelligence-section__title">
            {t('dayInsightsQueryFamiliesTitle')}
          </h2>
          {detail.queryFamilies.families.length === 0 ? (
            <div className="intelligence-empty">
              <p className="intelligence-empty__text">
                {t('queryFamiliesPlaceholder')}
              </p>
            </div>
          ) : (
            <IntelligenceSectionBody className="query-families">
              {detail.queryFamilies.families.map((family) => (
                <QueryFamilyCard
                  key={family.familyId}
                  family={family}
                  href={queryFamilyInsightsHref({
                    familyId: family.familyId,
                    dateRange: detail.drilldown.explorerDateRange,
                    preset: 'custom',
                    profileId,
                  })}
                  linkMode="card"
                  memberCountLabel={t('queryFamilyMemberCount')}
                  showDates={false}
                  showMembers={false}
                />
              ))}
            </IntelligenceSectionBody>
          )}
        </section>

        <section className="intelligence-section">
          <h2 className="intelligence-section__title">
            {t('dayInsightsRefindsTitle')}
          </h2>
          {detail.refindPages.length === 0 ? (
            <div className="intelligence-empty">
              <p className="intelligence-empty__text">{t('refindEmpty')}</p>
            </div>
          ) : (
            <IntelligenceSectionBody className="refind-list">
              {/* TODO: M10 - share the refind summary card between overview/day views and the dedicated refind route while keeping entity-first CTAs centralized. */}
              {detail.refindPages.map((page) => (
                <div key={page.canonicalUrl} className="refind-card">
                  <div className="refind-card__header">
                    <span className="refind-card__icon">📄</span>
                    <Link
                      className="refind-card__title"
                      to={refindInsightsHref({
                        canonicalUrl: page.canonicalUrl,
                        dateRange: detail.drilldown.explorerDateRange,
                        preset: 'custom',
                        profileId,
                      })}
                    >
                      {page.title ?? page.url}
                    </Link>
                  </div>
                  <p className="refind-card__description">
                    {t('refindDescription', {
                      days: page.crossDayCount,
                      searches: page.searchArrivalCount,
                    })}
                  </p>
                  <InsightEntityActions
                    className="intelligence-actions"
                    items={[
                      {
                        href: domainDayInsightsHref(
                          page.registrableDomain,
                          detail.date,
                          profileId,
                        ),
                        label: page.registrableDomain,
                        style: 'text',
                      },
                      {
                        href: evidenceHref({
                          profileId,
                          domain: page.registrableDomain,
                          url: page.canonicalUrl,
                          dateRange: detail.drilldown.explorerDateRange,
                        }),
                        label: t('entityOpenExplorer'),
                        style: 'text',
                      },
                    ]}
                  />
                </div>
              ))}
            </IntelligenceSectionBody>
          )}
        </section>
      </div>
    </div>
  )
}

function DayInsightsHourStrip({
  date,
  hourly,
  t,
}: {
  date: string
  hourly: DayInsightsHourlyBucket[]
  t: (key: string, vars?: Record<string, string | number>) => string
}) {
  const maxHourlyCount = Math.max(
    ...hourly.map((bucket) => bucket.visitCount),
    1,
  )
  const normalized = useMemo(() => {
    const byHour = new Map(
      hourly.map((bucket) => [bucket.hour, bucket.visitCount]),
    )
    return Array.from({ length: 24 }, (_, hour) => ({
      hour,
      visitCount: byHour.get(hour) ?? 0,
    }))
  }, [hourly])

  if (!normalized.some((bucket) => bucket.visitCount > 0)) {
    return (
      <p className="rhythm-day-detail__empty">{t('rhythmDayNoHourlyData')}</p>
    )
  }

  return (
    <div
      className="rhythm-hour-strip"
      role="img"
      aria-label={t('rhythmHourStripLabel', { date })}
    >
      <div className="rhythm-hour-strip__grid">
        {normalized.map((bucket) => (
          <span
            key={bucket.hour}
            className="rhythm-hour-strip__cell"
            data-level={heatLevel(bucket.visitCount, maxHourlyCount)}
            title={t('rhythmHourTooltip', {
              hour: formatHourRange(bucket.hour),
              count: bucket.visitCount,
            })}
          />
        ))}
      </div>
      <div className="rhythm-hour-strip__labels" aria-hidden="true">
        {[0, 6, 12, 18, 23].map((hour) => (
          <span key={hour}>{hour}</span>
        ))}
      </div>
    </div>
  )
}

function heatLevel(count: number, maxCount: number) {
  if (count <= 0 || maxCount <= 0) {
    return 0
  }

  const ratio = count / maxCount
  if (ratio >= 0.75) return 4
  if (ratio >= 0.5) return 3
  if (ratio >= 0.25) return 2
  return 1
}
