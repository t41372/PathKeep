/**
 * Day insights route and presentational surface.
 *
 * Why this file exists:
 * - Local-calendar-day insights are now a first-class route under
 *   `/intelligence/day/:date`.
 * - The route owns the full day entity review surface so overview cards can
 *   stay navigation-first instead of rebuilding inline detail everywhere.
 */

import { useEffect, useMemo } from 'react'
import { Link, useParams, useSearchParams } from 'react-router-dom'
import {
  RhythmActivityProportionBar,
  RhythmHourStrip,
} from '../../components/intelligence/browsing-rhythm-detail'
import { InsightEntityActions } from '../../components/intelligence/entity-actions'
import { StarToggle } from '../../components/shell/star-toggle'
import { useDesktopStars } from '../explorer/use-desktop-stars'
import { Glyph } from '../../components/ui'
import { InsightEntityHero } from '../../components/intelligence/entity-hero'
import { IntelligenceMetricGrid } from '../../components/intelligence/metric-grid'
import { QueryFamilyCard } from '../../components/intelligence/query-family-card'
import { IntelligenceSectionMeta } from '../../components/intelligence/section-meta'
import { RefindSummaryCard } from '../../components/intelligence/workbench'
import { StatusCallout } from '../../components/primitives/status-callout'
import { singleDayDateRange, useAsyncData } from '../../lib/core-intelligence'
import * as api from '../../lib/core-intelligence/api'
import { useI18n } from '../../lib/i18n/hooks'
import {
  domainDayInsightsHref,
  queryFamilyInsightsHref,
  refindInsightsHref,
} from '../../lib/core-intelligence/routes'
import { evidenceHref } from '../../lib/intelligence-links'
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
import { formatNumber } from './sections/shared'

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
  const backHref = `/intelligence?${backParams.toString()}`

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
  const topSiteMaxVisits = detail
    ? Math.max(
        detail.topSites.reduce(
          (maxVisits, site) => Math.max(maxVisits, site.visitCount),
          0,
        ),
        1,
      )
    : 1
  const explorerHref = detail
    ? evidenceHref({
        profileId,
        dateRange: detail.drilldown.explorerDateRange,
      })
    : evidenceHref({
        profileId,
        dateRange: singleDayDateRange(date),
      })

  // Stars are an "everywhere" affordance — intelligence entities are starrable
  // too. Hydrate exactly the domains + refind pages this view renders (bounded
  // by the contact-sheet's top-N lists, never the archive), then expose a star
  // on each top-site domain row and each refind page row.
  const stars = useDesktopStars()
  const visibleDomains = useMemo(
    () => detail?.topSites.map((site) => site.registrableDomain) ?? [],
    [detail],
  )
  const visibleRefindUrls = useMemo(
    () => detail?.refindPages.map((page) => page.canonicalUrl) ?? [],
    [detail],
  )
  const domainsKey = visibleDomains.join('\n')
  const refindUrlsKey = visibleRefindUrls.join('\n')
  useEffect(() => {
    if (visibleDomains.length > 0) stars.hydrate('domain', visibleDomains)
    // domainsKey collapses the array identity so the effect re-runs only when
    // the actual domain set changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [domainsKey, stars])
  useEffect(() => {
    if (visibleRefindUrls.length > 0) stars.hydrate('url', visibleRefindUrls)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [refindUrlsKey, stars])
  const starStatusLabel = {
    starred: t('entityStarStatusStarred'),
    unstarred: t('entityStarStatusUnstarred'),
  }

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
            icon: <Glyph icon="bar_chart" />,
            label: t('digestVisits'),
            value: formatNumber(detail.digestSummary.totalVisits.value),
          },
          {
            icon: <Glyph icon="search" />,
            label: t('digestSearches'),
            value: formatNumber(detail.digestSummary.totalSearches.value),
          },
          {
            icon: <Glyph icon="public" />,
            label: t('digestNewSites'),
            value: formatNumber(detail.digestSummary.newDomains.value),
          },
          {
            icon: <Glyph icon="auto_stories" />,
            label: t('digestDeepRead'),
            value: formatNumber(detail.digestSummary.deepReadPages.value),
          },
        ]}
      />

      <section className="intelligence-section day-insights__wide-section">
        <h2 className="intelligence-section__title">
          {t('dayInsightsHourlyTitle')}
        </h2>
        {detail.hourlyActivity.some((bucket) => bucket.visitCount > 0) ? (
          <RhythmHourStrip
            date={detail.date}
            hourly={detail.hourlyActivity}
            t={t}
          />
        ) : (
          <div className="intelligence-empty">
            <p className="intelligence-empty__text">
              {t('rhythmDayNoHourlyData')}
            </p>
          </div>
        )}
      </section>

      <section className="intelligence-section day-insights__wide-section">
        <h2 className="intelligence-section__title">
          {t('dayInsightsActivityMixTitle')}
        </h2>
        {detail.activityMix.categories.length === 0 ? (
          <div className="intelligence-empty">
            <p className="intelligence-empty__text">{t('activityMixEmpty')}</p>
          </div>
        ) : (
          <RhythmActivityProportionBar
            categories={detail.activityMix.categories}
            categoryLabel={(domainCategory) =>
              intelligenceCategoryLabel(language, t, domainCategory)
            }
            language={language}
            t={t}
          />
        )}
      </section>

      <div className="intelligence-secondary-grid day-insights__secondary-grid">
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
                <div
                  key={site.registrableDomain}
                  className="top-site-row top-site-row--interactive group flex items-center gap-2"
                >
                  <Link
                    className="flex min-w-0 flex-1 items-center gap-2"
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
                            (site.visitCount / topSiteMaxVisits) * 100,
                          )}%`,
                        }}
                      />
                    </span>
                    <span className="top-site-row__count">
                      {formatNumber(site.visitCount)} {t('visits')}
                    </span>
                  </Link>
                  <StarToggle
                    starred={stars.isStarred('domain', site.registrableDomain)}
                    onToggle={() =>
                      stars.toggle('domain', site.registrableDomain)
                    }
                    starLabel={t('entityStarSourceAria')}
                    unstarLabel={t('entityUnstarSourceAria')}
                    statusLabel={starStatusLabel}
                    testId={`day-insights-star-domain-${site.registrableDomain}`}
                  />
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
              {detail.refindPages.map((page) => (
                <RefindSummaryCard
                  key={page.canonicalUrl}
                  actionsExtra={
                    <StarToggle
                      starred={stars.isStarred('url', page.canonicalUrl)}
                      onToggle={() => stars.toggle('url', page.canonicalUrl)}
                      starLabel={t('entityStarPageAria')}
                      unstarLabel={t('entityUnstarPageAria')}
                      statusLabel={starStatusLabel}
                      alwaysVisible
                      testId={`day-insights-star-page-${page.canonicalUrl}`}
                    />
                  }
                  actionItems={[
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
                  description={t('refindDescription', {
                    days: page.crossDayCount,
                    searches: page.searchArrivalCount,
                  })}
                  title={page.title ?? page.url}
                  titleHref={refindInsightsHref({
                    canonicalUrl: page.canonicalUrl,
                    dateRange: detail.drilldown.explorerDateRange,
                    preset: 'custom',
                    profileId,
                  })}
                />
              ))}
            </IntelligenceSectionBody>
          )}
        </section>
      </div>
    </div>
  )
}
