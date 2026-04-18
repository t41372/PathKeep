/**
 * Core Intelligence section components used by the main route shell.
 *
 * Why this file exists:
 * - The `/intelligence` shell should stay focused on route, scope, and deep-link state instead of owning every section implementation inline.
 * - Keeping the section components here makes it easier to evolve individual surfaces without turning the route file into another giant blob.
 *
 * Main declarations:
 * - `IntelligenceSections`
 *
 * Source-of-truth notes:
 * - Keep section ordering aligned with `docs/features/core-intelligence-ultimate-design.md` §2.2.
 * - Keep scope honesty and deep-link behavior aligned with `docs/design/screens-and-nav.md`.
 */

import { useState } from 'react'
import { Link } from 'react-router-dom'
import { ExplainabilityPanel } from '../../components/intelligence/explainability-panel'
import {
  useAsyncData,
  type DateRange,
  type EngineRanking,
  type SearchConcept,
  type KpiMetric,
  type RefindPage,
  type QueryFamily,
  type FrictionSignal,
  type DiscoveryTrendPoint,
  type BreadthIndex,
  type HabitPattern,
  type InterruptedHabit,
  type PathFlow,
  type CompareSet,
  type CompareSetPage,
  type BrowserDiff,
  type BrowserProfileSummary,
  type CategoryMixEntry,
  type ObservedInteraction,
} from '../../lib/core-intelligence'
import { evidenceHref } from '../../lib/intelligence'
import * as api from '../../lib/core-intelligence/api'

type T = (key: string, vars?: Record<string, string | number>) => string

interface IntelligenceSectionsProps {
  dateRange: DateRange
  domainHref: (domain: string) => string
  profileId: string | null
  t: T
}

/**
 * Renders the complete set of Core Intelligence overview sections.
 */
export function IntelligenceSections({
  dateRange,
  domainHref,
  profileId,
  t,
}: IntelligenceSectionsProps) {
  return (
    <div className="intelligence-grid">
      <DigestSection dateRange={dateRange} profileId={profileId} t={t} />
      <div className="intelligence-row intelligence-row--two-col">
        <OnThisDaySection profileId={profileId} t={t} />
        <TopSitesSection
          dateRange={dateRange}
          domainHref={domainHref}
          profileId={profileId}
          t={t}
        />
      </div>
      <SearchActivitySection
        dateRange={dateRange}
        profileId={profileId}
        t={t}
      />
      <RefindPagesSection dateRange={dateRange} profileId={profileId} t={t} />
      <div className="intelligence-row intelligence-row--two-col">
        <ActivityMixSection dateRange={dateRange} profileId={profileId} t={t} />
        <BrowsingRhythmSection
          dateRange={dateRange}
          profileId={profileId}
          t={t}
        />
      </div>

      <div className="intelligence-row intelligence-row--two-col">
        <StableSourcesSection
          dateRange={dateRange}
          domainHref={domainHref}
          profileId={profileId}
          t={t}
        />
        <SearchEffectivenessSection
          dateRange={dateRange}
          domainHref={domainHref}
          profileId={profileId}
          t={t}
        />
      </div>
      <div className="intelligence-row intelligence-row--two-col">
        <FrictionDetectionSection
          dateRange={dateRange}
          profileId={profileId}
          t={t}
        />
        <ReopenedInvestigationsSection
          dateRange={dateRange}
          profileId={profileId}
          t={t}
        />
      </div>
      <DiscoveryTrendSection
        dateRange={dateRange}
        profileId={profileId}
        t={t}
      />

      <div className="intelligence-row intelligence-row--two-col">
        <BreadthIndexSection
          dateRange={dateRange}
          profileId={profileId}
          t={t}
        />
        <PathFlowsSection dateRange={dateRange} profileId={profileId} t={t} />
      </div>
      <HabitsSection dateRange={dateRange} profileId={profileId} t={t} />

      <div className="intelligence-row intelligence-row--two-col">
        <CompareSetsSection dateRange={dateRange} profileId={profileId} t={t} />
        <MultiBrowserDiffSection dateRange={dateRange} t={t} />
      </div>
      <ObservedInteractionsSection
        dateRange={dateRange}
        profileId={profileId}
        t={t}
      />
    </div>
  )
}

function DigestSection({
  dateRange,
  profileId,
  t,
}: {
  dateRange: DateRange
  profileId: string | null
  t: T
}) {
  const { data, loading, error } = useAsyncData(
    () => api.getDigestSummary(dateRange, profileId),
    [dateRange, profileId],
  )

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

  if (error || !data) {
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

  const cards: { icon: string; label: string; metric: KpiMetric }[] = [
    { icon: '📊', label: t('digestVisits'), metric: data.totalVisits },
    { icon: '🔍', label: t('digestSearches'), metric: data.totalSearches },
    { icon: '🌐', label: t('digestNewSites'), metric: data.newDomains },
    { icon: '📖', label: t('digestDeepRead'), metric: data.deepReadPages },
    { icon: '🔄', label: t('digestRefind'), metric: data.refindPages },
  ]

  return (
    <section className="intelligence-section digest-section">
      <h2 className="intelligence-section__title">{t('digestTitle')}</h2>
      <div className="digest-cards">
        {cards.map(({ icon, label, metric }) => (
          <div key={label} className="digest-card">
            <span className="digest-card__icon">{icon}</span>
            <span className="digest-card__value">
              {formatNumber(metric.value)}
            </span>
            <span className="digest-card__label">{label}</span>
            <TrendBadge metric={metric} t={t} />
          </div>
        ))}
      </div>
    </section>
  )
}

function TrendBadge({ metric, t }: { metric: KpiMetric; t: T }) {
  if (metric.changePercent == null) return null
  const arrow =
    metric.trend === 'up' ? '↑' : metric.trend === 'down' ? '↓' : '='
  const sign = metric.changePercent > 0 ? '+' : ''
  return (
    <span
      className={`trend-badge trend-badge--${metric.trend}`}
      aria-label={t('trendLabel', {
        direction: metric.trend,
        percent: Math.abs(metric.changePercent),
      })}
    >
      {sign}
      {Math.round(metric.changePercent)}% {arrow}
    </span>
  )
}

function OnThisDaySection({
  profileId,
  t,
}: {
  profileId: string | null
  t: T
}) {
  const { data, loading } = useAsyncData(
    () => api.getOnThisDay(profileId),
    [profileId],
  )
  const [expanded, setExpanded] = useState(false)
  const visibleEntries = expanded ? (data ?? []) : (data ?? []).slice(0, 3)

  return (
    <section className="intelligence-section on-this-day-section">
      <h2 className="intelligence-section__title">{t('onThisDayTitle')}</h2>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--card" />
      ) : !data || data.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__eyebrow">{t('onThisDayEyebrow')}</p>
          <p className="intelligence-empty__text">{t('onThisDayEmpty')}</p>
        </div>
      ) : (
        <div className="on-this-day-list">
          {visibleEntries.map((entry) => (
            <div key={entry.year} className="on-this-day-entry">
              <div className="on-this-day-entry__header">
                <span className="on-this-day-entry__year">{entry.year}</span>
                <span className="on-this-day-entry__visits">
                  {t('onThisDayVisits', { count: entry.totalVisits })}
                </span>
                {entry.deepDiveSessions > 0 ? (
                  <span
                    className="on-this-day-entry__deep-dive-badge"
                    title={t('onThisDayDeepDive', {
                      count: entry.deepDiveSessions,
                    })}
                  >
                    🔬 {entry.deepDiveSessions}
                  </span>
                ) : null}
              </div>
              {entry.summary ? (
                <p className="on-this-day-entry__summary">{entry.summary}</p>
              ) : null}
              {entry.topDomains.length > 0 ? (
                <div className="on-this-day-entry__domains">
                  {entry.topDomains.slice(0, 4).map((domain) => (
                    <span
                      key={domain}
                      className="on-this-day-entry__domain-tag"
                    >
                      {domain}
                    </span>
                  ))}
                </div>
              ) : null}
            </div>
          ))}
          {data.length > 3 ? (
            <button
              className="intelligence-link"
              type="button"
              onClick={() => setExpanded((value) => !value)}
            >
              {expanded ? t('onThisDayCollapse') : t('onThisDayMore')}
            </button>
          ) : null}
        </div>
      )}
    </section>
  )
}

function TopSitesSection({
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
  const [sortBy, setSortBy] = useState('visit_count')
  const [search, setSearch] = useState('')
  const { data, loading } = useAsyncData(
    () => api.getTopSites(dateRange, profileId, sortBy, 20),
    [dateRange, profileId, sortBy],
  )

  const filteredData = data?.filter((site) => {
    if (!search.trim()) return true
    const needle = search.toLowerCase()
    return (
      site.registrableDomain.toLowerCase().includes(needle) ||
      (site.displayName?.toLowerCase().includes(needle) ?? false)
    )
  })

  return (
    <section className="intelligence-section top-sites-section">
      <h2 className="intelligence-section__title">{t('topSitesTitle')}</h2>
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
      ) : !filteredData || filteredData.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('topSitesEmpty')}</p>
        </div>
      ) : (
        <div className="intelligence-section__scroll-region">
          <div className="top-sites-list">
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
          </div>
        </div>
      )}
    </section>
  )
}

function SearchActivitySection({
  dateRange,
  profileId,
  t,
}: {
  dateRange: DateRange
  profileId: string | null
  t: T
}) {
  const [tab, setTab] = useState<'engines' | 'concepts' | 'families'>('engines')
  const engines = useAsyncData(
    () => api.getSearchEngineRanking(dateRange, profileId),
    [dateRange, profileId],
  )
  const concepts = useAsyncData(
    () => api.getTopSearchConcepts(dateRange, profileId, 50),
    [dateRange, profileId],
  )

  return (
    <section className="intelligence-section search-activity-section">
      <h2 className="intelligence-section__title">
        {t('searchActivityTitle')}
      </h2>
      <div className="intelligence-tabs" role="tablist">
        {(['engines', 'concepts', 'families'] as const).map((key) => (
          <button
            key={key}
            role="tab"
            type="button"
            aria-selected={tab === key}
            className={`intelligence-tab${tab === key ? ' intelligence-tab--active' : ''}`}
            onClick={() => setTab(key)}
          >
            {t(`searchTab_${key}`)}
          </button>
        ))}
      </div>
      <div className="intelligence-tab-content">
        {tab === 'engines' ? (
          <EngineRankingPanel
            data={engines.data}
            loading={engines.loading}
            t={t}
          />
        ) : null}
        {tab === 'concepts' ? (
          <ConceptCloudPanel
            data={concepts.data}
            loading={concepts.loading}
            t={t}
          />
        ) : null}
        {tab === 'families' ? (
          <QueryFamiliesPanel
            dateRange={dateRange}
            profileId={profileId}
            t={t}
          />
        ) : null}
      </div>
    </section>
  )
}

function EngineRankingPanel({
  data,
  loading,
  t,
}: {
  data: EngineRanking[] | null
  loading: boolean
  t: T
}) {
  if (loading) {
    return <div className="intelligence-skeleton intelligence-skeleton--bar" />
  }
  if (!data || data.length === 0) {
    return (
      <div className="intelligence-empty">
        <p className="intelligence-empty__text">{t('engineRankingEmpty')}</p>
      </div>
    )
  }

  const max = Math.max(data[0]?.searchCount ?? 0, 1)
  return (
    <div className="intelligence-section__scroll-region">
      <div className="engine-ranking">
        {data.map((engine) => (
          <div key={engine.searchEngine} className="engine-ranking__row">
            <span className="engine-ranking__name">
              {engine.displayName ?? engine.searchEngine}
            </span>
            <span className="engine-ranking__bar">
              <span
                className="engine-ranking__bar-fill"
                style={{
                  width: `${Math.round((engine.searchCount / max) * 100)}%`,
                }}
              />
            </span>
            <span className="engine-ranking__count">
              {formatNumber(engine.searchCount)}
            </span>
          </div>
        ))}
      </div>
    </div>
  )
}

function ConceptCloudPanel({
  data,
  loading,
  t,
}: {
  data: SearchConcept[] | null
  loading: boolean
  t: T
}) {
  if (loading) {
    return (
      <div className="intelligence-skeleton intelligence-skeleton--cloud" />
    )
  }
  if (!data || data.length === 0) {
    return (
      <div className="intelligence-empty">
        <p className="intelligence-empty__text">{t('conceptCloudEmpty')}</p>
      </div>
    )
  }

  const maxFrequency = Math.max(...data.map((concept) => concept.frequency), 1)
  return (
    <div
      className="concept-cloud"
      role="img"
      aria-label={t('conceptCloudLabel')}
    >
      {data.map((concept) => {
        const scale = 0.6 + (concept.frequency / maxFrequency) * 1.4
        return (
          <span
            key={concept.term}
            className="concept-cloud__term"
            style={{ fontSize: `${scale}em` }}
            title={t('conceptTooltip', {
              term: concept.term,
              count: concept.frequency,
            })}
          >
            {concept.term}
          </span>
        )
      })}
    </div>
  )
}

function QueryFamiliesPanel({
  dateRange,
  profileId,
  t,
}: {
  dateRange: DateRange
  profileId: string | null
  t: T
}) {
  const { data, loading, error } = useAsyncData(
    () => api.getQueryFamilies(dateRange, profileId, { page: 0, pageSize: 10 }),
    [dateRange, profileId],
  )

  if (loading) {
    return <div className="intelligence-skeleton intelligence-skeleton--list" />
  }

  if (error || !data || data.families.length === 0) {
    return (
      <div className="intelligence-empty">
        <p className="intelligence-empty__text">
          {error || t('queryFamiliesPlaceholder')}
        </p>
      </div>
    )
  }

  return (
    <div className="intelligence-section__scroll-region">
      <div className="query-families">
        {data.families.map((family) => (
          <QueryFamilyCard key={family.familyId} family={family} t={t} />
        ))}
      </div>
    </div>
  )
}

function QueryFamilyCard({ family, t }: { family: QueryFamily; t: T }) {
  const [expanded, setExpanded] = useState(false)
  const visibleQueries = expanded ? family.queries : family.queries.slice(0, 3)

  return (
    <div className="query-family-card">
      <div className="query-family-card__header">
        <span className="query-family-card__anchor">
          "{family.anchorQuery}"
        </span>
        <span className="query-family-card__engine">{family.searchEngine}</span>
        <span className="query-family-card__count">
          {family.memberCount} {t('queryFamilyMemberCount')}
        </span>
      </div>
      <div className="query-family-card__members">
        {visibleQueries.map((query, index) => (
          <span key={index} className="query-family-card__member">
            "{query}"
          </span>
        ))}
        {family.queries.length > 3 && !expanded ? (
          <button
            className="intelligence-link"
            type="button"
            onClick={() => setExpanded(true)}
          >
            +{family.queries.length - 3} {t('queryFamilyMore')}
          </button>
        ) : null}
      </div>
      <span className="query-family-card__dates">
        {family.firstSeenAt} - {family.lastSeenAt}
      </span>
      <ExplainabilityPanel
        entityType="query_family"
        entityId={family.familyId}
        t={t}
      />
    </div>
  )
}

function RefindPagesSection({
  dateRange,
  profileId,
  t,
}: {
  dateRange: DateRange
  profileId: string | null
  t: T
}) {
  const { data, loading } = useAsyncData(
    () => api.getRefindPages(dateRange, profileId, 5),
    [dateRange, profileId],
  )

  return (
    <section className="intelligence-section refind-section">
      <h2 className="intelligence-section__title">{t('refindTitle')}</h2>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--list" />
      ) : !data || data.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('refindEmpty')}</p>
        </div>
      ) : (
        <div className="refind-list">
          {data.map((page) => (
            <RefindCard
              key={page.canonicalUrl}
              page={page}
              profileId={profileId}
              t={t}
            />
          ))}
        </div>
      )}
    </section>
  )
}

function RefindCard({
  page,
  profileId,
  t,
}: {
  page: RefindPage
  profileId: string | null
  t: T
}) {
  const [showFactors, setShowFactors] = useState(false)
  const factors = [
    { label: t('refindFactorDays'), value: page.crossDayCount, weight: 3 },
    { label: t('refindFactorTrails'), value: page.trailCount, weight: 3 },
    {
      label: t('refindFactorSearch'),
      value: page.searchArrivalCount,
      weight: 2,
    },
    { label: t('refindFactorTyped'), value: page.typedRevisitCount, weight: 1 },
  ]
  const maxContribution = Math.max(
    ...factors.map((factor) => factor.value * factor.weight),
    1,
  )

  return (
    <div className="refind-card">
      <div className="refind-card__header">
        <span className="refind-card__icon">📄</span>
        <Link
          className="refind-card__title"
          to={evidenceHref({
            domain: page.registrableDomain,
            profileId,
            url: page.canonicalUrl,
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
      <button
        className="refind-card__expand-toggle"
        type="button"
        onClick={() => setShowFactors((value) => !value)}
      >
        <span>{showFactors ? '▾' : '▸'}</span>
        <span>{t('refindShowFactors')}</span>
        <span style={{ marginLeft: 'auto', fontFamily: 'var(--font-code)' }}>
          {t('refindScore')}: {page.refindScore.toFixed(1)}
        </span>
      </button>
      {showFactors ? (
        <div className="refind-card__factors">
          {factors.map((factor) => (
            <div key={factor.label} className="refind-card__factor">
              <span className="refind-card__factor-label">{factor.label}</span>
              <span
                className="refind-card__factor-bar"
                style={{
                  width: `${Math.round(((factor.value * factor.weight) / maxContribution) * 80)}px`,
                }}
              />
              <span className="refind-card__factor-value">
                {factor.value} ×{factor.weight}
              </span>
            </div>
          ))}
        </div>
      ) : null}
      <ExplainabilityPanel
        entityType="refind_page"
        entityId={page.canonicalUrl}
        t={t}
      />
    </div>
  )
}

function ActivityMixSection({
  dateRange,
  profileId,
  t,
}: {
  dateRange: DateRange
  profileId: string | null
  t: T
}) {
  const { data, loading } = useAsyncData(
    () => api.getActivityMix(dateRange, profileId),
    [dateRange, profileId],
  )

  return (
    <section className="intelligence-section activity-mix-section">
      <h2 className="intelligence-section__title">{t('activityMixTitle')}</h2>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--chart" />
      ) : !data || data.categories.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('activityMixEmpty')}</p>
        </div>
      ) : (
        <div className="activity-mix">
          {data.categories.map((category) => {
            const change = data.changeVsPrevious.find(
              (entry) => entry.domainCategory === category.domainCategory,
            )
            const changePoints = change?.changePoints ?? 0
            return (
              <div key={category.domainCategory} className="activity-mix__row">
                <span className="activity-mix__category">
                  {t(`category_${category.domainCategory}`) ||
                    category.domainCategory}
                </span>
                <span className="activity-mix__bar">
                  <span
                    className="activity-mix__bar-fill"
                    style={{ width: `${Math.round(category.share * 100)}%` }}
                    data-category={category.domainCategory}
                  />
                </span>
                <span className="activity-mix__share">
                  {Math.round(category.share * 100)}%
                </span>
                {changePoints !== 0 ? (
                  <span
                    className={`activity-mix__change activity-mix__change--${changePoints > 0 ? 'positive' : 'negative'}`}
                  >
                    {changePoints > 0 ? '+' : ''}
                    {Math.round(changePoints * 100)}%
                  </span>
                ) : null}
              </div>
            )
          })}
        </div>
      )}
    </section>
  )
}

function BrowsingRhythmSection({
  dateRange,
  profileId,
  t,
}: {
  dateRange: DateRange
  profileId: string | null
  t: T
}) {
  const [category, setCategory] = useState<string | undefined>(undefined)
  const { data, loading } = useAsyncData(
    () => api.getBrowsingRhythm(dateRange, profileId, category),
    [category, dateRange, profileId],
  )

  const days = [
    t('dow_sun'),
    t('dow_mon'),
    t('dow_tue'),
    t('dow_wed'),
    t('dow_thu'),
    t('dow_fri'),
    t('dow_sat'),
  ]
  const categoryOptions = [
    { value: '', label: t('rhythmAllCategories') },
    { value: 'developer', label: t('category_developer') },
    { value: 'docs', label: t('category_docs') },
    { value: 'social', label: t('category_social') },
    { value: 'shopping', label: t('category_shopping') },
    { value: 'news', label: t('category_news') },
    { value: 'entertainment', label: t('category_entertainment') },
    { value: 'search', label: t('category_search') },
    { value: 'ai', label: t('category_ai') },
  ]

  return (
    <section className="intelligence-section rhythm-section">
      <div className="intelligence-section__title-row">
        <h2 className="intelligence-section__title">{t('rhythmTitle')}</h2>
        <select
          className="top-sites-controls__sort"
          value={category ?? ''}
          onChange={(event) => setCategory(event.target.value || undefined)}
          aria-label={t('rhythmCategoryFilter')}
        >
          {categoryOptions.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
      </div>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--heatmap" />
      ) : !data || data.cells.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('rhythmEmpty')}</p>
        </div>
      ) : (
        <div
          className="rhythm-heatmap"
          role="img"
          aria-label={t('rhythmLabel')}
        >
          <div className="rhythm-heatmap__header">
            <span className="rhythm-heatmap__corner" />
            {Array.from({ length: 24 }).map((_, hour) => (
              <span key={hour} className="rhythm-heatmap__hour">
                {hour}
              </span>
            ))}
          </div>
          {days.map((dayLabel, dow) => (
            <div key={dow} className="rhythm-heatmap__row">
              <span className="rhythm-heatmap__day">{dayLabel}</span>
              {Array.from({ length: 24 }).map((_, hour) => {
                const cell = data.cells.find(
                  (entry) => entry.dow === dow && entry.hour === hour,
                )
                const intensity = cell
                  ? Math.min(1, cell.visitCount / data.maxCount)
                  : 0
                return (
                  <span
                    key={hour}
                    className="rhythm-heatmap__cell"
                    style={{
                      opacity: intensity > 0 ? 0.2 + intensity * 0.8 : 0.05,
                      backgroundColor:
                        intensity > 0 ? 'var(--accent)' : 'var(--text-faint)',
                    }}
                    title={
                      cell
                        ? t('rhythmCellTooltip', {
                            day: dayLabel,
                            hour,
                            count: cell.visitCount,
                          })
                        : ''
                    }
                  />
                )
              })}
            </div>
          ))}
        </div>
      )}
    </section>
  )
}

function StableSourcesSection({
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
  const { data, loading } = useAsyncData(
    () => api.getStableSources(dateRange, profileId),
    [dateRange, profileId],
  )
  const entries = data?.filter((source) => source.sourceRole === 'entry')
  const landings = data?.filter((source) => source.sourceRole === 'landing')

  return (
    <section className="intelligence-section stable-sources-section">
      <h2 className="intelligence-section__title">{t('stableSourcesTitle')}</h2>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--list" />
      ) : !data || data.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('stableSourcesEmpty')}</p>
        </div>
      ) : (
        <div className="stable-sources">
          <div className="stable-sources__column">
            <h3 className="stable-sources__subtitle">
              {t('stableSourcesEntry')}
            </h3>
            {entries?.slice(0, 5).map((source, index) => (
              <Link
                key={source.registrableDomain}
                className="stable-source-row"
                to={domainHref(source.registrableDomain)}
              >
                <span className="stable-source-row__rank">{index + 1}.</span>
                <span className="stable-source-row__domain">
                  {source.displayName ?? source.registrableDomain}
                </span>
                <span className="stable-source-row__count">
                  {source.trailCount} {t('stableSourcesTrails')}
                </span>
              </Link>
            ))}
          </div>
          <div className="stable-sources__column">
            <h3 className="stable-sources__subtitle">
              {t('stableSourcesLanding')}
            </h3>
            {landings?.slice(0, 5).map((source, index) => (
              <Link
                key={source.registrableDomain}
                className="stable-source-row"
                to={domainHref(source.registrableDomain)}
              >
                <span className="stable-source-row__rank">{index + 1}.</span>
                <span className="stable-source-row__domain">
                  {source.displayName ?? source.registrableDomain}
                </span>
                <span className="stable-source-row__count">
                  {source.stableLandingCount} {t('stableSourcesLandings')}
                </span>
              </Link>
            ))}
          </div>
        </div>
      )}
    </section>
  )
}

function SearchEffectivenessSection({
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
  const { data, loading } = useAsyncData(
    () => api.getSearchEffectiveness(dateRange, profileId),
    [dateRange, profileId],
  )

  return (
    <section className="intelligence-section search-effectiveness-section">
      <h2 className="intelligence-section__title">
        {t('searchEffectivenessTitle')}
      </h2>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--chart" />
      ) : !data || data.engineStats.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">
            {t('searchEffectivenessEmpty')}
          </p>
        </div>
      ) : (
        <div className="search-effectiveness">
          <div className="search-effectiveness__engines">
            {data.engineStats.map((engine) => {
              const maxReformulations = Math.max(
                ...data.engineStats.map((entry) => entry.avgReformulations),
                1,
              )
              const barWidth = Math.min(
                100,
                Math.round(
                  (engine.avgReformulations / maxReformulations) * 100,
                ),
              )

              return (
                <div
                  key={engine.searchEngine}
                  className="search-effectiveness__engine-row"
                >
                  <span className="search-effectiveness__engine-name">
                    {engine.displayName ?? engine.searchEngine}
                  </span>
                  <span className="search-effectiveness__engine-bar">
                    <span
                      className="search-effectiveness__engine-bar-fill"
                      style={{ width: `${barWidth}%` }}
                    />
                  </span>
                  <span className="search-effectiveness__engine-stat">
                    {engine.avgReformulations.toFixed(1)}{' '}
                    {t('searchEffectivenessReformulations')}
                  </span>
                </div>
              )
            })}
          </div>
          {data.topResolvingSources.length > 0 ? (
            <div className="search-effectiveness__sources">
              <h3 className="search-effectiveness__subtitle">
                {t('searchEffectivenessSources')}
              </h3>
              {data.topResolvingSources.slice(0, 5).map((source) => (
                <Link
                  key={`${source.sourceRole}:${source.registrableDomain}`}
                  className="search-effectiveness__source-row"
                  to={domainHref(source.registrableDomain)}
                >
                  <span className="search-effectiveness__source-domain">
                    {source.displayName ?? source.registrableDomain}
                  </span>
                  <span className="search-effectiveness__source-meta">
                    {source.stableLandingCount} {t('stableSourcesLandings')}
                  </span>
                </Link>
              ))}
            </div>
          ) : null}
          {data.hardestTopics.length > 0 ? (
            <div className="search-effectiveness__hard-topics">
              <h3 className="search-effectiveness__subtitle">
                {t('searchEffectivenessHardest')}
              </h3>
              {data.hardestTopics.slice(0, 3).map((topic) => (
                <div
                  key={topic.queryFamily}
                  className="search-effectiveness__topic-row"
                >
                  <span className="search-effectiveness__topic-query">
                    "{topic.queryFamily}"
                  </span>
                  <span className="search-effectiveness__topic-stat">
                    {topic.reformulationCount}{' '}
                    {t('searchEffectivenessReformulations')}
                  </span>
                </div>
              ))}
            </div>
          ) : null}
        </div>
      )}
    </section>
  )
}

function FrictionDetectionSection({
  dateRange,
  profileId,
  t,
}: {
  dateRange: DateRange
  profileId: string | null
  t: T
}) {
  const { data, loading } = useAsyncData(
    () => api.getFrictionSignals(dateRange, profileId),
    [dateRange, profileId],
  )

  return (
    <section className="intelligence-section friction-section">
      <h2 className="intelligence-section__title">{t('frictionTitle')}</h2>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--list" />
      ) : !data || data.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('frictionEmpty')}</p>
        </div>
      ) : (
        <div className="intelligence-section__scroll-region">
          <div className="friction-list">
            {data.slice(0, 8).map((signal, index) => (
              <FrictionSignalCard key={index} signal={signal} t={t} />
            ))}
          </div>
        </div>
      )}
    </section>
  )
}

function FrictionSignalCard({ signal, t }: { signal: FrictionSignal; t: T }) {
  return (
    <div className="friction-card">
      <div className="friction-card__header">
        <span
          className={`friction-card__evidence-badge friction-card__evidence-badge--${signal.evidenceType}`}
        >
          {signal.evidenceType === 'strong'
            ? t('frictionStrong')
            : t('frictionWeak')}
        </span>
        <span className="friction-card__domain">
          {signal.registrableDomain ?? signal.url ?? '—'}
        </span>
        <span className="friction-card__count">{signal.occurrenceCount}×</span>
      </div>
      <p className="friction-card__description">{signal.description}</p>
    </div>
  )
}

function ReopenedInvestigationsSection({
  dateRange,
  profileId,
  t,
}: {
  dateRange: DateRange
  profileId: string | null
  t: T
}) {
  const { data, loading } = useAsyncData(
    () => api.getReopenedInvestigations(dateRange, profileId),
    [dateRange, profileId],
  )

  return (
    <section className="intelligence-section reopened-section">
      <h2 className="intelligence-section__title">{t('reopenedTitle')}</h2>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--list" />
      ) : !data || data.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('reopenedEmpty')}</p>
        </div>
      ) : (
        <div className="intelligence-section__scroll-region">
          <div className="reopened-list">
            {data.slice(0, 8).map((item) => (
              <div key={item.investigationId} className="reopened-card">
                <div className="reopened-card__header">
                  <span
                    className={`reopened-card__anchor-badge reopened-card__anchor-badge--${item.anchorType}`}
                  >
                    {item.anchorType === 'query_family'
                      ? t('reopenedAnchorQuery')
                      : t('reopenedAnchorPage')}
                  </span>
                  <span className="reopened-card__label">
                    {item.anchorLabel}
                  </span>
                </div>
                <div className="reopened-card__meta">
                  <span>
                    {t('reopenedOccurrences', {
                      count: item.occurrenceCount,
                    })}
                  </span>
                  <span>
                    {t('reopenedDistinctDays', { days: item.distinctDays })}
                  </span>
                </div>
                <span className="reopened-card__dates">
                  {item.firstSeenAt} - {item.lastSeenAt}
                </span>
                <ExplainabilityPanel
                  entityType="reopened_investigation"
                  entityId={item.investigationId}
                  t={t}
                />
              </div>
            ))}
          </div>
        </div>
      )}
    </section>
  )
}

function DiscoveryTrendSection({
  dateRange,
  profileId,
  t,
}: {
  dateRange: DateRange
  profileId: string | null
  t: T
}) {
  const { data, loading } = useAsyncData(
    () => api.getDiscoveryTrend(dateRange, profileId, 'week'),
    [dateRange, profileId],
  )
  const maxRate = data
    ? Math.max(
        ...data.points.map((point: DiscoveryTrendPoint) => point.discoveryRate),
        0.01,
      )
    : 1
  const maxNewDomains = data
    ? Math.max(
        ...data.points.map(
          (point: DiscoveryTrendPoint) => point.newDomainCount,
        ),
        1,
      )
    : 1

  return (
    <section className="intelligence-section discovery-trend-section">
      <h2 className="intelligence-section__title">
        {t('discoveryTrendTitle')}
      </h2>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--chart" />
      ) : !data || data.points.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('discoveryTrendEmpty')}</p>
        </div>
      ) : (
        <div className="discovery-trend">
          <div className="discovery-trend__chart">
            {data.points.map((point) => {
              const rateHeight = Math.round(
                (point.discoveryRate / maxRate) * 100,
              )
              const barHeight = Math.round(
                (point.newDomainCount / maxNewDomains) * 100,
              )

              return (
                <div
                  key={point.dateKey}
                  className="discovery-trend__bar-group"
                  title={`${point.dateKey}: ${Math.round(point.discoveryRate * 100)}% · ${point.newDomainCount} ${t('discoveryTrendNewDomains')}`}
                >
                  <div className="discovery-trend__rate-bar-container">
                    <span
                      className="discovery-trend__rate-bar"
                      style={{ height: `${rateHeight}%` }}
                    />
                  </div>
                  <div className="discovery-trend__domain-bar-container">
                    <span
                      className="discovery-trend__domain-bar"
                      style={{ height: `${barHeight}%` }}
                    />
                  </div>
                  <span className="discovery-trend__date-label">
                    {point.dateKey.slice(5)}
                  </span>
                </div>
              )
            })}
          </div>
          <div className="discovery-trend__legend">
            <span className="discovery-trend__legend-item">
              <span className="discovery-trend__legend-swatch discovery-trend__legend-swatch--rate" />
              {t('discoveryTrendRateLabel')}
            </span>
            <span className="discovery-trend__legend-item">
              <span className="discovery-trend__legend-swatch discovery-trend__legend-swatch--domains" />
              {t('discoveryTrendDomainsLabel')}
            </span>
          </div>
        </div>
      )}
    </section>
  )
}

function BreadthIndexSection({
  dateRange,
  profileId,
  t,
}: {
  dateRange: DateRange
  profileId: string | null
  t: T
}) {
  const { data, loading } = useAsyncData(
    () => api.getBreadthIndex(dateRange, profileId),
    [dateRange, profileId],
  )

  return (
    <section className="intelligence-section breadth-section">
      <h2 className="intelligence-section__title">{t('breadthTitle')}</h2>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--card" />
      ) : !data ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('breadthEmpty')}</p>
        </div>
      ) : (
        <BreadthIndexBody data={data} t={t} />
      )}
    </section>
  )
}

function BreadthIndexBody({ data, t }: { data: BreadthIndex; t: T }) {
  const score = Math.max(0, Math.min(100, Math.round(data.breadthScore)))
  const verdictKey =
    score >= 70
      ? 'breadthVerdictBroad'
      : score >= 40
        ? 'breadthVerdictBalanced'
        : 'breadthVerdictFocused'

  return (
    <div className="breadth-index">
      <div className="breadth-index__score-block">
        <span className="breadth-index__score">{score}</span>
        <span className="breadth-index__score-label">
          {t('breadthScoreLabel')}
        </span>
      </div>
      <div className="breadth-index__meter">
        <span
          className="breadth-index__meter-fill"
          style={{ width: `${score}%` }}
        />
      </div>
      <p className="breadth-index__verdict">{t(verdictKey)}</p>
      <p className="breadth-index__detail">
        {t('breadthConcentrationDetail', {
          count: data.concentrationDomainCount,
        })}
      </p>
      <p className="breadth-index__meta">
        {t('breadthHhiLabel', { value: data.hhi.toFixed(3) })}
      </p>
    </div>
  )
}

function PathFlowsSection({
  dateRange,
  profileId,
  t,
}: {
  dateRange: DateRange
  profileId: string | null
  t: T
}) {
  const [stepCount, setStepCount] = useState<number>(3)
  const { data, loading } = useAsyncData(
    () => api.getPathFlows(dateRange, profileId, stepCount, 15),
    [dateRange, profileId, stepCount],
  )

  return (
    <section className="intelligence-section path-flows-section">
      <div className="intelligence-section__title-row">
        <h2 className="intelligence-section__title">{t('pathFlowsTitle')}</h2>
        <select
          className="top-sites-controls__sort"
          value={stepCount}
          onChange={(event) => setStepCount(Number(event.target.value))}
          aria-label={t('pathFlowsStepLabel')}
        >
          <option value={2}>{t('pathFlowsStep2')}</option>
          <option value={3}>{t('pathFlowsStep3')}</option>
        </select>
      </div>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--list" />
      ) : !data || data.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('pathFlowsEmpty')}</p>
        </div>
      ) : (
        <ul className="path-flows">
          {data.map((flow, index) => (
            <PathFlowRow
              key={`${flow.flowPattern}:${flow.stepCount}:${index}`}
              flow={flow}
              profileId={profileId}
              t={t}
            />
          ))}
        </ul>
      )}
    </section>
  )
}

function PathFlowRow({
  flow,
  profileId,
  t,
}: {
  flow: PathFlow
  profileId: string | null
  t: T
}) {
  const steps = flow.flowPattern.split(/\s*(?:->|→)\s*/).filter(Boolean)
  const explainEntityId = profileId
    ? `${profileId}::${flow.stepCount}::${flow.flowPattern}`
    : null

  return (
    <li className="path-flow-row">
      <div className="path-flow-row__chips">
        {steps.map((step, index) => (
          <span key={index} className="path-flow-row__group">
            <span className="path-flow-row__chip">{step}</span>
            {index < steps.length - 1 ? (
              <span className="path-flow-row__arrow" aria-hidden="true">
                →
              </span>
            ) : null}
          </span>
        ))}
      </div>
      <span className="path-flow-row__count">
        {t('pathFlowsOccurrences', { count: flow.occurrenceCount })}
      </span>
      {explainEntityId ? (
        <ExplainabilityPanel
          entityType="path_flow"
          entityId={explainEntityId}
          t={t}
        />
      ) : null}
    </li>
  )
}

function HabitsSection({
  dateRange,
  profileId,
  t,
}: {
  dateRange: DateRange
  profileId: string | null
  t: T
}) {
  const patterns = useAsyncData(
    () => api.getHabitPatterns(dateRange, profileId),
    [dateRange, profileId],
  )
  const interrupted = useAsyncData(
    () => api.getInterruptedHabits(profileId),
    [profileId],
  )
  const empty =
    !patterns.loading &&
    !interrupted.loading &&
    (!patterns.data || patterns.data.length === 0) &&
    (!interrupted.data || interrupted.data.length === 0)

  return (
    <section className="intelligence-section habits-section">
      <h2 className="intelligence-section__title">{t('habitsTitle')}</h2>
      {patterns.loading || interrupted.loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--list" />
      ) : empty ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('habitsEmpty')}</p>
        </div>
      ) : (
        <div className="habits-body">
          {interrupted.data && interrupted.data.length > 0 ? (
            <div className="habits-interrupted">
              <h3 className="habits-body__subtitle">
                {t('habitsInterruptedTitle')}
              </h3>
              <ul className="habits-interrupted__list">
                {interrupted.data.slice(0, 5).map((habit, index) => (
                  <InterruptedHabitRow
                    key={index}
                    habit={habit}
                    profileId={profileId}
                    t={t}
                  />
                ))}
              </ul>
            </div>
          ) : null}
          {patterns.data && patterns.data.length > 0 ? (
            <div className="habits-patterns">
              <h3 className="habits-body__subtitle">
                {t('habitsPatternsTitle')}
              </h3>
              <ul className="habits-patterns__list">
                {patterns.data.slice(0, 12).map((habit, index) => (
                  <HabitPatternRow
                    key={index}
                    habit={habit}
                    profileId={profileId}
                    t={t}
                  />
                ))}
              </ul>
            </div>
          ) : null}
        </div>
      )}
    </section>
  )
}

function HabitPatternRow({
  habit,
  profileId,
  t,
}: {
  habit: HabitPattern
  profileId: string | null
  t: T
}) {
  const explainEntityId = profileId
    ? `${profileId}::${habit.registrableDomain}`
    : null

  return (
    <li className="habit-row">
      <span className="habit-row__domain">
        {habit.displayName ?? habit.registrableDomain}
      </span>
      <span className={`habit-row__type habit-row__type--${habit.habitType}`}>
        {t(`habitType_${habit.habitType}`)}
      </span>
      <span className="habit-row__cadence">
        {t('habitCadence', {
          interval: habit.meanIntervalDays.toFixed(1),
        })}
      </span>
      <span className="habit-row__visits">
        {t('habitVisits', { count: habit.visitCount })}
      </span>
      {explainEntityId ? (
        <ExplainabilityPanel
          entityType="habit_pattern"
          entityId={explainEntityId}
          t={t}
        />
      ) : null}
    </li>
  )
}

function InterruptedHabitRow({
  habit,
  profileId,
  t,
}: {
  habit: InterruptedHabit
  profileId: string | null
  t: T
}) {
  const explainEntityId = profileId
    ? `${profileId}::${habit.registrableDomain}`
    : null

  return (
    <li className="habit-row habit-row--interrupted">
      <span className="habit-row__domain">
        {habit.displayName ?? habit.registrableDomain}
      </span>
      <span className="habit-row__type habit-row__type--interrupted">
        {t('habitInterruptedBadge')}
      </span>
      <span className="habit-row__cadence">
        {t('habitInterruptedDetail', {
          days: habit.daysSinceLastVisit,
          expected: habit.meanIntervalDays.toFixed(1),
        })}
      </span>
      {explainEntityId ? (
        <ExplainabilityPanel
          entityType="habit_pattern"
          entityId={explainEntityId}
          t={t}
        />
      ) : null}
    </li>
  )
}

function CompareSetsSection({
  dateRange,
  profileId,
  t,
}: {
  dateRange: DateRange
  profileId: string | null
  t: T
}) {
  const { data, loading } = useAsyncData(
    () => api.getCompareSets(dateRange, profileId),
    [dateRange, profileId],
  )

  return (
    <section className="intelligence-section compare-sets-section">
      <h2 className="intelligence-section__title">{t('compareSetsTitle')}</h2>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--list" />
      ) : !data || data.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('compareSetsEmpty')}</p>
        </div>
      ) : (
        <ul className="compare-sets">
          {data.slice(0, 6).map((set) => (
            <CompareSetCard key={set.compareSetId} set={set} t={t} />
          ))}
        </ul>
      )}
    </section>
  )
}

function CompareSetCard({ set, t }: { set: CompareSet; t: T }) {
  return (
    <li className="compare-set">
      <div className="compare-set__header">
        <span className="compare-set__query">{set.searchQuery}</span>
        <span className="compare-set__count">
          {t('compareSetsPages', { count: set.pages.length })}
        </span>
      </div>
      <ul className="compare-set__pages">
        {set.pages.slice(0, 4).map((page: CompareSetPage, index) => (
          <li
            key={index}
            className={`compare-set__page${page.isLanding ? ' compare-set__page--landing' : ''}`}
          >
            <span className="compare-set__page-domain">
              {page.registrableDomain}
            </span>
            <span className="compare-set__page-title" title={page.title ?? ''}>
              {page.title ?? page.url}
            </span>
            {page.isLanding ? (
              <span className="compare-set__landing-badge">
                {t('compareSetsLanding')}
              </span>
            ) : null}
          </li>
        ))}
      </ul>
    </li>
  )
}

function MultiBrowserDiffSection({
  dateRange,
  t,
}: {
  dateRange: DateRange
  t: T
}) {
  const { data, loading } = useAsyncData(
    () => api.getMultiBrowserDiff(dateRange),
    [dateRange],
  )

  return (
    <section className="intelligence-section multi-browser-section">
      <div className="intelligence-section__title-row">
        <h2 className="intelligence-section__title">
          {t('multiBrowserTitle')}
        </h2>
        <span className="status-badge status-info">
          {t('archiveWideBadge')}
        </span>
      </div>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--chart" />
      ) : !data || data.profiles.length < 2 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('multiBrowserEmpty')}</p>
        </div>
      ) : (
        <MultiBrowserDiffBody data={data} t={t} />
      )}
    </section>
  )
}

function MultiBrowserDiffBody({ data, t }: { data: BrowserDiff; t: T }) {
  const profileById = new Map<string, BrowserProfileSummary>(
    data.profiles.map((profile) => [profile.profileId, profile]),
  )
  const exclusiveByProfile = new Map<string, typeof data.exclusiveDomains>()
  for (const entry of data.exclusiveDomains) {
    const list = exclusiveByProfile.get(entry.profileId) ?? []
    list.push(entry)
    exclusiveByProfile.set(entry.profileId, list)
  }

  return (
    <div className="multi-browser">
      <div className="multi-browser__profiles">
        {data.profiles.map((profile) => (
          <div key={profile.profileId} className="multi-browser__profile">
            <span className="multi-browser__profile-name">
              {profile.profileName}
            </span>
            <span className="multi-browser__profile-family">
              {profile.browserFamily}
            </span>
            <span className="multi-browser__profile-stats">
              {t('multiBrowserVisits', { count: profile.visitCount })} ·{' '}
              {t('multiBrowserDomains', { count: profile.domainCount })}
            </span>
          </div>
        ))}
      </div>
      <div className="multi-browser__shared">
        <h3 className="multi-browser__subtitle">
          {t('multiBrowserShared', { count: data.sharedDomains.length })}
        </h3>
        <div className="multi-browser__shared-chips">
          {data.sharedDomains.slice(0, 10).map((domain) => (
            <span key={domain} className="multi-browser__chip">
              {domain}
            </span>
          ))}
        </div>
      </div>
      <div className="multi-browser__exclusive">
        <h3 className="multi-browser__subtitle">
          {t('multiBrowserExclusive')}
        </h3>
        <div className="multi-browser__exclusive-grid">
          {Array.from(exclusiveByProfile.entries()).map(
            ([profileId, entries]) => {
              const profile = profileById.get(profileId)
              return (
                <div
                  key={profileId}
                  className="multi-browser__exclusive-column"
                >
                  <span className="multi-browser__exclusive-header">
                    {profile?.profileName ?? profileId}
                  </span>
                  <ul className="multi-browser__exclusive-list">
                    {entries.slice(0, 5).map((entry) => (
                      <li
                        key={entry.registrableDomain}
                        className="multi-browser__exclusive-row"
                      >
                        <span>{entry.registrableDomain}</span>
                        <span className="multi-browser__exclusive-count">
                          {entry.visitCount}
                        </span>
                      </li>
                    ))}
                  </ul>
                </div>
              )
            },
          )}
        </div>
      </div>
      <div className="multi-browser__categories">
        <h3 className="multi-browser__subtitle">
          {t('multiBrowserCategories')}
        </h3>
        <MultiBrowserCategoryBars
          distributions={data.categoryDistributions}
          t={t}
        />
      </div>
    </div>
  )
}

function MultiBrowserCategoryBars({
  distributions,
  t,
}: {
  distributions: BrowserDiff['categoryDistributions']
  t: T
}) {
  const allCategories = new Set<string>()
  for (const distribution of distributions) {
    for (const category of distribution.categories) {
      allCategories.add(category.domainCategory)
    }
  }
  const categoryList = Array.from(allCategories)

  return (
    <div className="multi-browser__category-bars">
      {categoryList.map((category) => (
        <div key={category} className="multi-browser__category-row">
          <span className="multi-browser__category-label">
            {t(`category_${category}`) || category}
          </span>
          <div className="multi-browser__category-profiles">
            {distributions.map((distribution) => {
              const entry: CategoryMixEntry | undefined =
                distribution.categories.find(
                  (item) => item.domainCategory === category,
                )
              const share = entry ? Math.round(entry.share * 100) : 0
              return (
                <div
                  key={distribution.profileId}
                  className="multi-browser__category-bar"
                  title={`${distribution.profileName}: ${share}%`}
                >
                  <span
                    className="multi-browser__category-bar-fill"
                    style={{ width: `${share}%` }}
                  />
                  <span className="multi-browser__category-bar-meta">
                    {distribution.profileName} {share}%
                  </span>
                </div>
              )
            })}
          </div>
        </div>
      ))}
    </div>
  )
}

function ObservedInteractionsSection({
  dateRange,
  profileId,
  t,
}: {
  dateRange: DateRange
  profileId: string | null
  t: T
}) {
  const { data, loading } = useAsyncData(
    () => api.getObservedInteractions(dateRange, profileId),
    [dateRange, profileId],
  )

  return (
    <section className="intelligence-section observed-section">
      <div className="intelligence-section__title-row">
        <h2 className="intelligence-section__title">{t('observedTitle')}</h2>
        <span className="observed-section__badge">
          {t('observedCapabilityBadge')}
        </span>
      </div>
      <p className="observed-section__disclaimer">{t('observedDisclaimer')}</p>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--list" />
      ) : !data || data.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('observedEmpty')}</p>
        </div>
      ) : (
        <ul className="observed-list">
          {data.slice(0, 10).map((item) => (
            <ObservedInteractionRow key={item.visitId} item={item} t={t} />
          ))}
        </ul>
      )}
    </section>
  )
}

function ObservedInteractionRow({
  item,
  t,
}: {
  item: ObservedInteraction
  t: T
}) {
  const foreground =
    item.foregroundDurationMs != null
      ? formatDuration(item.foregroundDurationMs)
      : null
  const scroll =
    item.scrollingTimeMs != null ? formatDuration(item.scrollingTimeMs) : null

  return (
    <li className="observed-row">
      <div className="observed-row__main">
        <span className="observed-row__title" title={item.url}>
          {item.title ?? item.url}
        </span>
        <span className="observed-row__family">{item.browserFamily}</span>
      </div>
      <div className="observed-row__metrics">
        {foreground ? (
          <span className="observed-row__metric">
            {t('observedForeground', { duration: foreground })}
          </span>
        ) : null}
        {scroll ? (
          <span className="observed-row__metric">
            {t('observedScroll', { duration: scroll })}
          </span>
        ) : null}
        {item.keyPresses != null && item.keyPresses > 0 ? (
          <span className="observed-row__metric">
            {t('observedKeyPresses', { count: item.keyPresses })}
          </span>
        ) : null}
        {item.loadSuccessful === false ? (
          <span className="observed-row__metric observed-row__metric--warn">
            {t('observedLoadFailed')}
          </span>
        ) : null}
      </div>
    </li>
  )
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`
  const seconds = ms / 1000
  if (seconds < 60) return `${seconds.toFixed(1)}s`
  const minutes = seconds / 60
  if (minutes < 60) return `${minutes.toFixed(1)}m`
  const hours = minutes / 60
  return `${hours.toFixed(1)}h`
}

function formatNumber(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`
  return String(value)
}
