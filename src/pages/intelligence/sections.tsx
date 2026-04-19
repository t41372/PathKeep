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

import { useState } from 'react'
import { Link } from 'react-router-dom'
import { ExplainabilityPanel } from '../../components/intelligence/explainability-panel'
import { IntelligenceSectionMeta } from '../../components/intelligence/section-meta'
import {
  useAsyncData,
  type DateRange,
  type KpiMetric,
  type OnThisDayEntry,
  type RefindPage,
} from '../../lib/core-intelligence'
import * as api from '../../lib/core-intelligence/api'
import type { ResolvedLanguage } from '../../lib/i18n'
import { evidenceHref } from '../../lib/intelligence'
import { IntelligenceSectionBody } from './sections/section-body'
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
  dateRange: DateRange
  domainHref: (domain: string) => string
  language: ResolvedLanguage
  profileId: string | null
  scopeLabel: string
  t: T
}

/**
 * Renders the complete set of Core Intelligence overview sections.
 */
export function IntelligenceSections({
  dateRange,
  domainHref,
  language,
  profileId,
  scopeLabel,
  t,
}: IntelligenceSectionsProps) {
  return (
    <div className="intelligence-grid">
      <DigestSection
        dateRange={dateRange}
        profileId={profileId}
        scopeLabel={scopeLabel}
        t={t}
      />
      <div className="intelligence-row intelligence-row--two-col">
        <OnThisDaySection profileId={profileId} scopeLabel={scopeLabel} t={t} />
        <HabitsSection
          dateRange={dateRange}
          profileId={profileId}
          scopeLabel={scopeLabel}
          t={t}
        />
      </div>
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
          scopeLabel={scopeLabel}
          t={t}
        />
      </div>
      <div className="intelligence-row intelligence-row--two-col">
        <SearchActivitySection
          dateRange={dateRange}
          profileId={profileId}
          scopeLabel={scopeLabel}
          t={t}
        />
        <ActivityMixSection
          dateRange={dateRange}
          language={language}
          profileId={profileId}
          scopeLabel={scopeLabel}
          t={t}
        />
      </div>
      <BrowsingRhythmSection
        dateRange={dateRange}
        domainHref={domainHref}
        language={language}
        profileId={profileId}
        scopeLabel={scopeLabel}
        t={t}
      />
      <div className="intelligence-secondary-grid">
        <StableSourcesSection
          dateRange={dateRange}
          domainHref={domainHref}
          profileId={profileId}
          scopeLabel={scopeLabel}
          t={t}
        />
        <SearchEffectivenessSection
          dateRange={dateRange}
          domainHref={domainHref}
          profileId={profileId}
          scopeLabel={scopeLabel}
          t={t}
        />
        <FrictionDetectionSection
          dateRange={dateRange}
          profileId={profileId}
          scopeLabel={scopeLabel}
          t={t}
        />
        <ReopenedInvestigationsSection
          dateRange={dateRange}
          profileId={profileId}
          scopeLabel={scopeLabel}
          t={t}
        />
        <DiscoveryTrendSection
          dateRange={dateRange}
          profileId={profileId}
          scopeLabel={scopeLabel}
          t={t}
        />
        <BreadthIndexSection
          dateRange={dateRange}
          profileId={profileId}
          scopeLabel={scopeLabel}
          t={t}
        />
        <PathFlowsSection
          dateRange={dateRange}
          profileId={profileId}
          scopeLabel={scopeLabel}
          t={t}
        />
        <CompareSetsSection
          dateRange={dateRange}
          profileId={profileId}
          scopeLabel={scopeLabel}
          t={t}
        />
        <MultiBrowserDiffSection
          dateRange={dateRange}
          language={language}
          scopeLabel={scopeLabel}
          t={t}
        />
        <ObservedInteractionsSection
          dateRange={dateRange}
          profileId={profileId}
          scopeLabel={scopeLabel}
          t={t}
        />
      </div>
    </div>
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

  const cards: { icon: string; label: string; metric: KpiMetric }[] = [
    { icon: '📊', label: t('digestVisits'), metric: digest.totalVisits },
    { icon: '🔍', label: t('digestSearches'), metric: digest.totalSearches },
    { icon: '🌐', label: t('digestNewSites'), metric: digest.newDomains },
    { icon: '📖', label: t('digestDeepRead'), metric: digest.deepReadPages },
    { icon: '🔄', label: t('digestRefind'), metric: digest.refindPages },
  ]

  return (
    <section className="intelligence-section digest-section">
      <h2 className="intelligence-section__title">{t('digestTitle')}</h2>
      <IntelligenceSectionMeta meta={data!.meta} scopeLabel={scopeLabel} />
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
  scopeLabel,
  t,
}: {
  profileId: string | null
  scopeLabel: string
  t: T
}) {
  const { data, loading } = useAsyncData(
    () => api.getOnThisDay(profileId),
    [profileId],
  )
  const entries = data?.data ?? []
  const [expanded, setExpanded] = useState(false)
  const visibleEntries = expanded ? entries : entries.slice(0, 3)

  return (
    <section className="intelligence-section on-this-day-section">
      <h2 className="intelligence-section__title">{t('onThisDayTitle')}</h2>
      {data ? (
        <IntelligenceSectionMeta meta={data.meta} scopeLabel={scopeLabel} />
      ) : null}
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--card" />
      ) : entries.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__eyebrow">{t('onThisDayEyebrow')}</p>
          <p className="intelligence-empty__text">{t('onThisDayEmpty')}</p>
        </div>
      ) : (
        <IntelligenceSectionBody className="on-this-day-list">
          {visibleEntries.map((entry) => (
            <OnThisDayEntryCard key={entry.year} entry={entry} t={t} />
          ))}
          {entries.length > 3 ? (
            <button
              className="intelligence-link"
              type="button"
              onClick={() => setExpanded((value) => !value)}
            >
              {expanded ? t('onThisDayCollapse') : t('onThisDayMore')}
            </button>
          ) : null}
        </IntelligenceSectionBody>
      )}
    </section>
  )
}

function OnThisDayEntryCard({ entry, t }: { entry: OnThisDayEntry; t: T }) {
  return (
    <div className="on-this-day-entry">
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
            <span key={domain} className="on-this-day-entry__domain-tag">
              {domain}
            </span>
          ))}
        </div>
      ) : null}
    </div>
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
      <h2 className="intelligence-section__title">{t('topSitesTitle')}</h2>
      {data ? (
        <IntelligenceSectionMeta meta={data.meta} scopeLabel={scopeLabel} />
      ) : null}
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
  scopeLabel,
  t,
}: {
  dateRange: DateRange
  profileId: string | null
  scopeLabel: string
  t: T
}) {
  const { data, loading } = useAsyncData(
    () => api.getRefindPages(dateRange, profileId, 5),
    [dateRange, profileId],
  )
  const pages = data?.data ?? []

  return (
    <section className="intelligence-section refind-section">
      <h2 className="intelligence-section__title">{t('refindTitle')}</h2>
      {data ? (
        <IntelligenceSectionMeta meta={data.meta} scopeLabel={scopeLabel} />
      ) : null}
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
