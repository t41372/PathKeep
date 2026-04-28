/**
 * Core Intelligence section coordinator used by the main route shell.
 *
 * Why this file exists:
 * - The `/intelligence` route should stay focused on scope and deep-link state,
 *   while this module decides the page-level IA and keeps the remaining
 *   overview sections together.
 * - Larger interactive sections live in focused sibling modules so this file
 *   can stay readable when the page evolves.
 *
 * Main declarations:
 * - `IntelligenceSections`
 *
 * Source-of-truth notes:
 * - Keep section ordering aligned with `docs/features/core-intelligence-ultimate-design.md`.
 * - Keep scope honesty and deep-link behavior aligned with `docs/design/screens-and-nav.md`.
 */

import { Fragment, useState, type ReactNode } from 'react'
import { Link } from 'react-router-dom'
import { ExplainabilityPanel } from '../../components/intelligence/explainability-panel'
import { Glyph } from '../../components/ui'
import { IntelligenceMetricGrid } from '../../components/intelligence/metric-grid'
import { IntelligenceSectionMeta } from '../../components/intelligence/section-meta'
import {
  RefindSummaryCard,
  type RefindWorkbenchFactor,
} from '../../components/intelligence/workbench'
import {
  useAsyncData,
  type DateRange,
  type RefindPage,
  type TimeRangePreset,
} from '../../lib/core-intelligence'
import * as api from '../../lib/core-intelligence/api'
import type { ResolvedLanguage } from '../../lib/i18n'
import { evidenceHref } from '../../lib/intelligence-links'
import { storageGrowthEvidence } from '../../lib/storage-analytics'
import type { DashboardSnapshot } from '../../lib/types'
import { IntelligenceSectionBody } from './sections/section-body'
import { GrowthSignalSection, StorageAnalyticsSection } from './sections/health'
import {
  ActivityMixSection,
  SearchActivitySection,
} from './sections/search-and-activity-section'
import { BrowsingRhythmSection } from './sections/browsing-rhythm-section'
import {
  BreadthIndexSection,
  CompareSetsSection,
  DiscoveryTrendSection,
  FrictionDetectionSection,
  HabitsSection,
  MultiBrowserDiffSection,
  ObservedInteractionsSection,
  PathFlowsSection,
  ReopenedInvestigationsSection,
  SearchEffectivenessSection,
  StableSourcesSection,
} from './sections/secondary-sections'
import { formatNumber, type T } from './sections/shared'

interface IntelligenceSectionsProps {
  compareSetHref: (compareSetId: string) => string
  dashboard: DashboardSnapshot | null
  dateRange: DateRange
  dayHref: (date: string) => string
  domainHref: (domain: string) => string
  focusedDomainHref: (
    domain: string,
    focus: { focusType: 'compare-set' | 'path-flow'; focusId: string },
  ) => string
  language: ResolvedLanguage
  preset: TimeRangePreset
  profileId: string | null
  queryFamilyHref: (familyId: string, profileId?: string | null) => string
  refindHref: (canonicalUrl: string) => string
  secondaryReady: boolean
  scopeLabel: string
  trailHref: (trailId: string, profileId?: string | null) => string
  t: T
}

interface SecondarySectionSlot {
  isReady: () => boolean
  key: string
  node: ReactNode
}

/**
 * Renders the complete set of Core Intelligence overview sections.
 */
export function IntelligenceSections({
  compareSetHref,
  dashboard,
  dateRange,
  dayHref,
  domainHref,
  focusedDomainHref,
  language,
  preset,
  profileId,
  queryFamilyHref,
  refindHref,
  secondaryReady,
  scopeLabel,
  trailHref,
  t,
}: IntelligenceSectionsProps) {
  const growth = storageGrowthEvidence(dashboard)
  const healthSections = [
    {
      element: (
        <StorageAnalyticsSection
          key="storage-analytics"
          dashboard={dashboard}
        />
      ),
      empty: dashboard === null,
      key: 'storage-analytics',
    },
    {
      element: (
        <GrowthSignalSection key="growth-signal" dashboard={dashboard} />
      ),
      empty: growth.latestRunId === null,
      key: 'growth-signal',
    },
  ]
  const primaryHealthSections = healthSections
    .filter((section) => !section.empty)
    .map((section) => section.element)
  const deferredHealthSections = healthSections
    .filter((section) => section.empty)
    .map((section) => section.element)
  const secondarySlots: SecondarySectionSlot[] = [
    {
      isReady: () => Boolean(api.peekStableSources(dateRange, profileId)),
      key: 'stable-sources',
      node: (
        <StableSourcesSection
          dateRange={dateRange}
          domainHref={domainHref}
          profileId={profileId}
          scopeLabel={scopeLabel}
          t={t}
        />
      ),
    },
    {
      isReady: () => Boolean(api.peekSearchEffectiveness(dateRange, profileId)),
      key: 'search-effectiveness',
      node: (
        <SearchEffectivenessSection
          dateRange={dateRange}
          domainHref={domainHref}
          profileId={profileId}
          queryFamilyHref={queryFamilyHref}
          scopeLabel={scopeLabel}
          t={t}
        />
      ),
    },
    {
      isReady: () => Boolean(api.peekFrictionSignals(dateRange, profileId)),
      key: 'friction-signals',
      node: (
        <FrictionDetectionSection
          dateRange={dateRange}
          domainHref={domainHref}
          profileId={profileId}
          scopeLabel={scopeLabel}
          t={t}
        />
      ),
    },
    {
      isReady: () =>
        Boolean(api.peekHabitPatterns(dateRange, profileId)) ||
        Boolean(api.peekInterruptedHabits(profileId)),
      key: 'habits',
      node: (
        <HabitsSection
          dateRange={dateRange}
          domainHref={domainHref}
          profileId={profileId}
          scopeLabel={scopeLabel}
          t={t}
        />
      ),
    },
    {
      isReady: () =>
        Boolean(api.peekReopenedInvestigations(dateRange, profileId)),
      key: 'reopened-investigations',
      node: (
        <ReopenedInvestigationsSection
          dateRange={dateRange}
          profileId={profileId}
          scopeLabel={scopeLabel}
          t={t}
        />
      ),
    },
    {
      isReady: () =>
        Boolean(api.peekDiscoveryTrend(dateRange, profileId, 'week')),
      key: 'discovery-trend',
      node: (
        <DiscoveryTrendSection
          dateRange={dateRange}
          profileId={profileId}
          scopeLabel={scopeLabel}
          t={t}
        />
      ),
    },
    ...primaryHealthSections.map((node, index) => ({
      isReady: () => true,
      key: `primary-health-${index}`,
      node,
    })),
    {
      isReady: () => Boolean(api.peekBreadthIndex(dateRange, profileId)),
      key: 'breadth-index',
      node: (
        <BreadthIndexSection
          dateRange={dateRange}
          profileId={profileId}
          scopeLabel={scopeLabel}
          t={t}
        />
      ),
    },
    {
      isReady: () => Boolean(api.peekPathFlows(dateRange, profileId, 3, 15)),
      key: 'path-flows',
      node: (
        <PathFlowsSection
          dateRange={dateRange}
          focusedDomainHref={focusedDomainHref}
          profileId={profileId}
          scopeLabel={scopeLabel}
          t={t}
        />
      ),
    },
    {
      isReady: () => Boolean(api.peekCompareSets(dateRange, profileId)),
      key: 'compare-sets',
      node: (
        <CompareSetsSection
          compareSetHref={compareSetHref}
          dateRange={dateRange}
          focusedDomainHref={focusedDomainHref}
          profileId={profileId}
          scopeLabel={scopeLabel}
          trailHref={trailHref}
          t={t}
        />
      ),
    },
    {
      isReady: () => Boolean(api.peekMultiBrowserDiff(dateRange)),
      key: 'multi-browser-diff',
      node: (
        <MultiBrowserDiffSection
          dateRange={dateRange}
          domainHref={domainHref}
          language={language}
          scopeLabel={scopeLabel}
          t={t}
        />
      ),
    },
    {
      isReady: () =>
        Boolean(api.peekObservedInteractions(dateRange, profileId)),
      key: 'observed-interactions',
      node: (
        <ObservedInteractionsSection
          dateRange={dateRange}
          profileId={profileId}
          scopeLabel={scopeLabel}
          t={t}
        />
      ),
    },
    ...deferredHealthSections.map((node, index) => ({
      isReady: () => false,
      key: `deferred-health-${index}`,
      node,
    })),
  ]

  return (
    <div className="intelligence-grid">
      <DigestSection
        dateRange={dateRange}
        profileId={profileId}
        scopeLabel={scopeLabel}
        t={t}
      />
      <div className="intelligence-row intelligence-row--two-col">
        <TopSitesSection
          dateRange={dateRange}
          domainHref={domainHref}
          profileId={profileId}
          scopeLabel={scopeLabel}
          t={t}
        />
        <RefindPagesSection
          dateRange={dateRange}
          profileId={profileId}
          refindHref={refindHref}
          scopeLabel={scopeLabel}
          t={t}
        />
      </div>
      <div className="intelligence-row intelligence-row--two-col">
        <SearchActivitySection
          dateRange={dateRange}
          language={language}
          profileId={profileId}
          queryFamilyHref={queryFamilyHref}
          scopeLabel={scopeLabel}
          trailHref={trailHref}
          t={t}
        />
        <ActivityMixSection
          dateRange={dateRange}
          domainHref={domainHref}
          language={language}
          profileId={profileId}
          scopeLabel={scopeLabel}
          t={t}
        />
      </div>
      <BrowsingRhythmSection
        dateRange={dateRange}
        dayHref={dayHref}
        language={language}
        preset={preset}
        profileId={profileId}
        scopeLabel={scopeLabel}
        t={t}
      />
      <div className="intelligence-secondary-grid">
        {secondarySlots.map((slot) =>
          secondaryReady || slot.isReady() ? (
            <Fragment key={slot.key}>{slot.node}</Fragment>
          ) : (
            <SecondarySectionSkeleton key={slot.key} sectionKey={slot.key} />
          ),
        )}
      </div>
    </div>
  )
}

export function IntelligenceSectionsSkeleton() {
  return (
    <div className="intelligence-grid">
      <section className="intelligence-section digest-section">
        <div className="intelligence-skeleton intelligence-skeleton--card" />
      </section>
      <div className="intelligence-row intelligence-row--two-col">
        <section className="intelligence-section">
          <div className="intelligence-skeleton intelligence-skeleton--list" />
        </section>
        <section className="intelligence-section">
          <div className="intelligence-skeleton intelligence-skeleton--list" />
        </section>
      </div>
      <div className="intelligence-row intelligence-row--two-col">
        <section className="intelligence-section">
          <div className="intelligence-skeleton intelligence-skeleton--card" />
        </section>
        <section className="intelligence-section">
          <div className="intelligence-skeleton intelligence-skeleton--card" />
        </section>
      </div>
      <section className="intelligence-section rhythm-section">
        <div className="intelligence-skeleton intelligence-skeleton--heatmap" />
      </section>
      <div className="intelligence-secondary-grid">
        {Array.from({ length: 6 }).map((_, index) => (
          <section key={index} className="intelligence-section">
            <div className="intelligence-skeleton intelligence-skeleton--card" />
          </section>
        ))}
      </div>
    </div>
  )
}

function SecondarySectionSkeleton({ sectionKey }: { sectionKey: string }) {
  return (
    <section
      className="intelligence-section"
      data-testid={`secondary-section-skeleton-${sectionKey}`}
    >
      <div className="intelligence-skeleton intelligence-skeleton--card" />
    </section>
  )
}

function DigestSection({
  dateRange,
  profileId,
  scopeLabel,
  t,
}: {
  dateRange: DateRange
  profileId: string | null
  scopeLabel: string
  t: T
}) {
  const { data, loading, error } = useAsyncData(
    () => api.getDigestSummary(dateRange, profileId),
    [dateRange, profileId],
    {
      getCached: () => api.peekDigestSummary(dateRange, profileId),
    },
  )
  const digest = data?.data ?? null

  if (loading) {
    return (
      <section className="intelligence-section digest-section">
        <h2 className="intelligence-section__title">{t('digestTitle')}</h2>
        <div className="digest-cards">
          {Array.from({ length: 5 }).map((_, index) => (
            <div key={index} className="digest-card digest-card--skeleton">
              <div className="digest-card__value-skeleton" />
              <div className="digest-card__label-skeleton" />
            </div>
          ))}
        </div>
      </section>
    )
  }

  if (error || !digest) {
    return (
      <section className="intelligence-section digest-section">
        <h2 className="intelligence-section__title">{t('digestTitle')}</h2>
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">
            {error || t('digestUnavailable')}
          </p>
        </div>
      </section>
    )
  }

  const cards: Array<{
    icon: ReactNode
    label: string
    metric: typeof digest.totalVisits
  }> = [
    {
      icon: <Glyph icon="bar_chart" />,
      label: t('digestVisits'),
      metric: digest.totalVisits,
    },
    {
      icon: <Glyph icon="search" />,
      label: t('digestSearches'),
      metric: digest.totalSearches,
    },
    {
      icon: <Glyph icon="public" />,
      label: t('digestNewSites'),
      metric: digest.newDomains,
    },
    {
      icon: <Glyph icon="auto_stories" />,
      label: t('digestDeepRead'),
      metric: digest.deepReadPages,
    },
    {
      icon: <Glyph icon="sync" />,
      label: t('digestRefind'),
      metric: digest.refindPages,
    },
  ]

  return (
    <section className="intelligence-section digest-section">
      <div className="intelligence-section__title-row">
        <h2 className="intelligence-section__title">{t('digestTitle')}</h2>
        <IntelligenceSectionMeta meta={data!.meta} scopeLabel={scopeLabel} />
      </div>
      <IntelligenceMetricGrid
        items={cards.map(({ icon, label, metric }) => ({
          icon,
          label,
          trend: metric,
          value: formatNumber(metric.value),
        }))}
        t={t}
      />
    </section>
  )
}

function TopSitesSection({
  dateRange,
  domainHref,
  profileId,
  scopeLabel,
  t,
}: {
  dateRange: DateRange
  domainHref: (domain: string) => string
  profileId: string | null
  scopeLabel: string
  t: T
}) {
  const [sortBy, setSortBy] = useState('visit_count')
  const [search, setSearch] = useState('')
  const { data, loading } = useAsyncData(
    () => api.getTopSites(dateRange, profileId, sortBy, 20),
    [dateRange, profileId, sortBy],
    {
      getCached: () => api.peekTopSites(dateRange, profileId, sortBy, 20),
    },
  )
  const sites = data?.data ?? []

  const filteredData = sites.filter((site) => {
    if (!search.trim()) return true
    const needle = search.toLowerCase()
    return (
      site.registrableDomain.toLowerCase().includes(needle) ||
      (site.displayName?.toLowerCase().includes(needle) ?? false)
    )
  })

  return (
    <section className="intelligence-section top-sites-section">
      <div className="intelligence-section__title-row">
        <h2 className="intelligence-section__title">{t('topSitesTitle')}</h2>
        {data ? (
          <IntelligenceSectionMeta meta={data.meta} scopeLabel={scopeLabel} />
        ) : null}
      </div>
      <div className="top-sites-controls">
        <input
          className="top-sites-controls__search"
          type="search"
          placeholder={t('topSitesSearch')}
          value={search}
          onChange={(event) => setSearch(event.target.value)}
          aria-label={t('topSitesSearch')}
        />
        <select
          className="top-sites-controls__sort"
          value={sortBy}
          onChange={(event) => setSortBy(event.target.value)}
          aria-label={t('topSitesSort')}
        >
          <option value="visit_count">{t('topSitesSortVisits')}</option>
          <option value="unique_days">{t('topSitesSortDays')}</option>
          <option value="avg_daily">{t('topSitesSortAvg')}</option>
        </select>
      </div>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--list" />
      ) : filteredData.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('topSitesEmpty')}</p>
        </div>
      ) : (
        <IntelligenceSectionBody className="top-sites-list">
          {filteredData.map((site, index) => {
            const maxValue =
              sortBy === 'unique_days'
                ? filteredData[0].uniqueDays
                : sortBy === 'avg_daily'
                  ? filteredData[0].averageDailyVisits
                  : filteredData[0].visitCount
            const value =
              sortBy === 'unique_days'
                ? site.uniqueDays
                : sortBy === 'avg_daily'
                  ? site.averageDailyVisits
                  : site.visitCount
            const displayValue =
              sortBy === 'avg_daily' ? value.toFixed(1) : formatNumber(value)
            const suffix =
              sortBy === 'unique_days'
                ? t('topSitesDays')
                : sortBy === 'avg_daily'
                  ? t('topSitesAvgSuffix')
                  : t('visits')

            return (
              <Link
                key={site.registrableDomain}
                className="top-site-row top-site-row--interactive"
                to={domainHref(site.registrableDomain)}
              >
                <span className="top-site-row__rank">{index + 1}.</span>
                <span className="top-site-row__domain">
                  {site.displayName ?? site.registrableDomain}
                </span>
                <span className="top-site-row__bar">
                  <span
                    className="top-site-row__bar-fill"
                    style={{
                      width: `${maxValue > 0 ? Math.round((value / maxValue) * 100) : 0}%`,
                    }}
                  />
                </span>
                <span className="top-site-row__count">
                  {displayValue} {suffix}
                </span>
              </Link>
            )
          })}
        </IntelligenceSectionBody>
      )}
    </section>
  )
}

function RefindPagesSection({
  dateRange,
  profileId,
  refindHref,
  scopeLabel,
  t,
}: {
  dateRange: DateRange
  profileId: string | null
  refindHref: (canonicalUrl: string) => string
  scopeLabel: string
  t: T
}) {
  const { data, loading } = useAsyncData(
    () => api.getRefindPages(dateRange, profileId, 5),
    [dateRange, profileId],
    {
      getCached: () => api.peekRefindPages(dateRange, profileId, 5),
    },
  )
  const pages = data?.data ?? []

  return (
    <section className="intelligence-section refind-section">
      <div className="intelligence-section__title-row">
        <h2 className="intelligence-section__title">{t('refindTitle')}</h2>
        {data ? (
          <IntelligenceSectionMeta meta={data.meta} scopeLabel={scopeLabel} />
        ) : null}
      </div>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--list" />
      ) : pages.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('refindEmpty')}</p>
        </div>
      ) : (
        <IntelligenceSectionBody className="refind-list">
          {pages.map((page) => (
            <RefindCard
              key={page.canonicalUrl}
              page={page}
              profileId={profileId}
              refindHref={refindHref}
              t={t}
            />
          ))}
        </IntelligenceSectionBody>
      )}
    </section>
  )
}

function RefindCard({
  page,
  profileId,
  refindHref,
  t,
}: {
  page: RefindPage
  profileId: string | null
  refindHref: (canonicalUrl: string) => string
  t: T
}) {
  const factors = buildRefindSummaryFactors(page, t)

  return (
    <RefindSummaryCard
      actionItems={[
        {
          href: evidenceHref({
            domain: page.registrableDomain,
            profileId,
            url: page.canonicalUrl,
          }),
          label: t('entityOpenExplorer'),
          style: 'text',
        },
      ]}
      description={t('refindDescription', {
        days: page.crossDayCount,
        searches: page.searchArrivalCount,
      })}
      expandLabel={t('refindShowFactors')}
      explainability={
        <ExplainabilityPanel
          entityType="refind_page"
          entityId={page.canonicalUrl}
          t={t}
        />
      }
      factorRows={factors}
      scoreLabel={`${t('refindScore')}: ${page.refindScore.toFixed(1)}`}
      title={page.title ?? page.url}
      titleHref={refindHref(page.canonicalUrl)}
    />
  )
}

function buildRefindSummaryFactors(page: RefindPage, t: T) {
  const rawFactors = [
    { label: t('refindFactorDays'), value: page.crossDayCount, weight: 3 },
    { label: t('refindFactorTrails'), value: page.trailCount, weight: 3 },
    {
      label: t('refindFactorSearch'),
      value: page.searchArrivalCount,
      weight: 2,
    },
    { label: t('refindFactorTyped'), value: page.typedRevisitCount, weight: 1 },
  ]

  return rawFactors.map<RefindWorkbenchFactor>((factor) => ({
    label: factor.label,
    emphasis: factor.value * factor.weight,
    valueLabel: `${factor.value} ×${factor.weight}`,
  }))
}
