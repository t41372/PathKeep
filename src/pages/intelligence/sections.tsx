/**
 * Core Intelligence section coordinator — newspaper-style hub layout.
 *
 * Why this file exists:
 * - The `/intelligence` route should stay focused on scope and deep-link state,
 *   while this module decides the page-level IA and keeps the remaining
 *   overview sections together.
 * - Organizes sections into a 3-layer newspaper hierarchy:
 *   Layer 1 (above the fold): KPI digest + spotlight
 *   Layer 2 (three-axis navigation): Time / Sources / Research preview cards
 *   Layer 3 (always-visible secondary grid): remaining secondary sections,
 *     each lazy-mounted on scroll so everything is reachable by scrolling
 *     while the up-front render cost stays bounded
 *
 * Main declarations:
 * - `IntelligenceSections`
 *
 * Source-of-truth notes:
 * - Keep section ordering aligned with `docs/features/core-intelligence-ultimate-design.md`.
 * - Keep scope honesty and deep-link behavior aligned with `docs/design/screens-and-nav.md`.
 */

import { useState, type ReactNode } from 'react'
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
  type QueryFamily,
  type RefindPage,
  type RhythmHeatmapCell,
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
import { AxisCard, SpotlightCard } from './sections/hub-layout'
import {
  hasDiscoveryTrendContent,
  hasFrictionContent,
  hasPathFlowsContent,
  hasReopenedInvestigationsContent,
  hasSearchEffectivenessContent,
} from './sections/secondary-sections/secondary-content'
import { LazySection } from './sections/lazy-section'
import { WeekdayWeekendStrip, PeakHoursStrip } from './sections/time-patterns'
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
  secondaryError: string | null
  secondaryReady: boolean
  onRetrySecondary: () => void
  scopeLabel: string
  trailHref: (trailId: string, profileId?: string | null) => string
  t: T
}

interface SecondarySectionSlot {
  isReady: () => boolean
  key: string
  node: ReactNode
  /**
   * Optional emptiness probe for hide-when-empty sections. Returns `false` only
   * when the section is provably going to render nothing (its cache is `ready`
   * and the filtered data is empty), so the layout can drop its slot instead of
   * leaving a blank masonry gap. Omitted for always-rendering cards, which keep
   * their slot and show an honest empty state.
   */
  hasContent?: () => boolean
}

/**
 * Renders the complete set of Core Intelligence overview sections in a
 * newspaper-style 3-layer hub layout.
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
  secondaryError,
  secondaryReady,
  onRetrySecondary,
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

  // Build secondary slots for Layer 3
  const secondarySlots: SecondarySectionSlot[] = [
    {
      isReady: () => Boolean(api.peekSearchEffectiveness(dateRange, profileId)),
      hasContent: () =>
        hasSearchEffectivenessContent(
          api.peekSearchEffectiveness(dateRange, profileId) ?? null,
        ),
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
      hasContent: () =>
        hasFrictionContent(
          api.peekFrictionSignals(dateRange, profileId) ?? null,
        ),
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
      hasContent: () =>
        hasReopenedInvestigationsContent(
          api.peekReopenedInvestigations(dateRange, profileId) ?? null,
        ),
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
      hasContent: () =>
        hasDiscoveryTrendContent(
          api.peekDiscoveryTrend(dateRange, profileId, 'week') ?? null,
        ),
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
      hasContent: () =>
        hasPathFlowsContent(
          api.peekPathFlows(dateRange, profileId, 3, 15) ?? null,
        ),
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

  // Rhythm heatmap data for time-pattern strips
  const { data: rhythmData } = useAsyncData(
    () => api.getBrowsingRhythm(dateRange, profileId),
    [dateRange, profileId],
  )
  const rhythmCells: RhythmHeatmapCell[] = rhythmData?.data?.cells ?? []

  // Peek data for Layer 2 previews
  const stableSourcesData = api.peekStableSources(dateRange, profileId)
  const queryFamiliesData = api.peekQueryFamilies(dateRange, profileId)

  // Derive a spotlight sentence from available overview data
  const spotlightSentence = deriveSpotlightSentence(
    api.peekRefindPages(dateRange, profileId, 5)?.data ?? null,
    queryFamiliesData?.data?.families ?? null,
    t,
  )

  return (
    <div className="intelligence-grid">
      {/* ── Layer 1: Above the Fold ── */}
      <DigestSection
        dateRange={dateRange}
        profileId={profileId}
        scopeLabel={scopeLabel}
        t={t}
      />
      <SpotlightCard sentence={spotlightSentence} />

      {/* ── Layer 2: Three-Axis Navigation ── */}

      {/* Time axis — full width */}
      <AxisCard
        title={t('hubTimeAxisTitle')}
        seeAllLabel={t('hubSeeAll')}
        testId="hub-axis-time"
      >
        <div className="hub-time-strips">
          <WeekdayWeekendStrip cells={rhythmCells} t={t} />
          <PeakHoursStrip cells={rhythmCells} t={t} />
        </div>
      </AxisCard>

      <BrowsingRhythmSection
        dateRange={dateRange}
        dayHref={dayHref}
        language={language}
        preset={preset}
        profileId={profileId}
        scopeLabel={scopeLabel}
        t={t}
      />

      {/* Sources + Research axis — two columns */}
      <div className="intelligence-row intelligence-row--two-col">
        {/* Sources axis */}
        <div className="hub-axis-stack">
          <AxisCard
            title={t('hubSourcesAxisTitle')}
            seeAllLabel={t('hubSeeAll')}
            testId="hub-axis-sources"
          >
            <TopSitesPreview
              dateRange={dateRange}
              domainHref={domainHref}
              profileId={profileId}
              t={t}
            />
            {stableSourcesData ? (
              <div className="hub-preview-subsection">
                <span className="hub-preview-subsection__label">
                  {t('hubStableSourcesPreview')}
                </span>
                <ul className="hub-preview-list">
                  {stableSourcesData.data.slice(0, 3).map((source) => (
                    <li key={source.registrableDomain}>
                      <Link
                        className="hub-preview-link"
                        to={domainHref(source.registrableDomain)}
                      >
                        {source.displayName ?? source.registrableDomain}
                      </Link>
                    </li>
                  ))}
                </ul>
              </div>
            ) : null}
          </AxisCard>
          <StableSourcesSection
            dateRange={dateRange}
            domainHref={domainHref}
            profileId={profileId}
            scopeLabel={scopeLabel}
            t={t}
          />
        </div>

        {/* Research axis */}
        <div className="hub-axis-stack">
          <AxisCard
            title={t('hubResearchAxisTitle')}
            seeAllLabel={t('hubSeeAll')}
            testId="hub-axis-research"
          >
            {queryFamiliesData ? (
              <div className="hub-preview-subsection">
                <span className="hub-preview-subsection__label">
                  {t('hubQueryFamiliesPreview')}
                </span>
                <ul className="hub-preview-list">
                  {queryFamiliesData.data.families.slice(0, 3).map((family) => (
                    <li key={family.familyId}>
                      <Link
                        className="hub-preview-link"
                        to={queryFamilyHref(family.familyId)}
                      >
                        {family.anchorQuery}
                      </Link>
                    </li>
                  ))}
                </ul>
              </div>
            ) : null}
            <RefindPreview
              dateRange={dateRange}
              profileId={profileId}
              refindHref={refindHref}
              t={t}
            />
          </AxisCard>
        </div>
      </div>

      {/* Full Top Sites + Refind + Search + Activity sections */}
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

      {/* ── Layer 3: Always-visible secondary grid ──
          Every secondary section renders inline — the user just scrolls and
          sees everything, no expand/collapse. Each section is lazy-mounted on
          scroll (IntersectionObserver, near-viewport) so the up-front render
          cost stays bounded; the staged hook batch-prefetches all secondary
          data into cache when `secondaryReady` flips, so a section that mounts
          near the viewport reads warm cache and resolves fast. While a section
          is not-yet-near OR not-yet-ready, the same skeleton holds its space. */}
      <div className="intelligence-secondary-grid">
        {secondarySlots
          .filter((slot) => slot.hasContent?.() ?? true)
          .map((slot) => {
            const skeleton = <SecondarySectionSkeleton sectionKey={slot.key} />
            const dataReady = secondaryReady || slot.isReady()
            // When the batch load failed and this slot has no warm cache, the
            // skeleton would otherwise hang forever. Surface an honest error with a
            // retry instead so the user is never stuck staring at an inert box.
            const placeholder =
              !dataReady && secondaryError ? (
                <SecondarySectionError
                  sectionKey={slot.key}
                  onRetry={onRetrySecondary}
                  t={t}
                />
              ) : (
                skeleton
              )
            return (
              <LazySection key={slot.key} skeleton={placeholder}>
                {dataReady ? slot.node : placeholder}
              </LazySection>
            )
          })}
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
      <section className="intelligence-section">
        <div className="intelligence-skeleton intelligence-skeleton--card" />
      </section>
      <section className="intelligence-section rhythm-section">
        <div className="intelligence-skeleton intelligence-skeleton--heatmap" />
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

/**
 * Replaces a never-resolving skeleton when the deferred batch load fails and a
 * section has no warm cache to fall back to. Announces itself to assistive tech
 * (`role="alert"`) and offers a retry so the insight is recoverable instead of
 * silently stuck.
 */
function SecondarySectionError({
  sectionKey,
  onRetry,
  t,
}: {
  sectionKey: string
  onRetry: () => void
  t: T
}) {
  return (
    <section
      className="intelligence-section"
      data-testid={`secondary-section-error-${sectionKey}`}
    >
      <div className="intelligence-empty" role="alert" aria-live="polite">
        <p className="intelligence-empty__text">
          {t('secondarySectionErrorTitle')}
        </p>
        <p className="intelligence-empty__text">
          {t('secondarySectionErrorBody')}
        </p>
        <button className="btn-secondary" type="button" onClick={onRetry}>
          {t('secondarySectionRetry')}
        </button>
      </div>
    </section>
  )
}

// ---------------------------------------------------------------------------
// Layer 1: Digest section (KPI cards)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Layer 2 preview helpers
// ---------------------------------------------------------------------------

/** Shows top 5 sites inline inside the Sources axis card. */
function TopSitesPreview({
  dateRange,
  domainHref,
  profileId,
  t,
}: {
  dateRange: DateRange
  domainHref: (domain: string) => string
  profileId: string | null
  t: T
}) {
  const cachedData = api.peekTopSites(dateRange, profileId, 'visit_count', 20)
  const sites = cachedData?.data?.slice(0, 5) ?? []

  if (sites.length === 0) return null

  return (
    <div className="hub-preview-subsection">
      <span className="hub-preview-subsection__label">
        {t('hubTopSitesPreview')}
      </span>
      <ul className="hub-preview-list">
        {sites.map((site) => (
          <li key={site.registrableDomain}>
            <Link
              className="hub-preview-link"
              to={domainHref(site.registrableDomain)}
            >
              {site.displayName ?? site.registrableDomain}
            </Link>
            <span className="hub-preview-count">
              {formatNumber(site.visitCount)}
            </span>
          </li>
        ))}
      </ul>
    </div>
  )
}

/** Shows top 3 refind pages inline inside the Research axis card. */
function RefindPreview({
  dateRange,
  profileId,
  refindHref,
  t,
}: {
  dateRange: DateRange
  profileId: string | null
  refindHref: (canonicalUrl: string) => string
  t: T
}) {
  const cachedData = api.peekRefindPages(dateRange, profileId, 5)
  const pages = cachedData?.data?.slice(0, 3) ?? []

  if (pages.length === 0) return null

  return (
    <div className="hub-preview-subsection">
      <span className="hub-preview-subsection__label">
        {t('hubRefindPreview')}
      </span>
      <ul className="hub-preview-list">
        {pages.map((page) => (
          <li key={page.canonicalUrl}>
            <Link
              className="hub-preview-link"
              to={refindHref(page.canonicalUrl)}
            >
              {page.title ?? page.url}
            </Link>
          </li>
        ))}
      </ul>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Full section: Top Sites
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Full section: Refind Pages
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Spotlight derivation
// ---------------------------------------------------------------------------

/**
 * Picks the single most notable insight for the Layer 1 spotlight card.
 * Returns null when no data is compelling enough — progressive disclosure.
 */
function deriveSpotlightSentence(
  refindPages: RefindPage[] | null,
  queryFamilies: QueryFamily[] | null,
  t: T,
): string | null {
  // Try top refind page first — a page revisited across many days is noteworthy
  const topRefind = refindPages?.[0]
  const topFamily = queryFamilies?.[0]

  if (topRefind && topRefind.crossDayCount >= 2) {
    return t('hubSpotlightRefind', {
      title: topRefind.title ?? topRefind.url,
      days: topRefind.crossDayCount,
    })
  }

  // Fall back to the query family with the most reformulations
  if (topFamily && topFamily.memberCount >= 2) {
    return t('hubSpotlightQueryFamily', {
      query: topFamily.anchorQuery,
      count: topFamily.memberCount,
    })
  }

  return null
}
