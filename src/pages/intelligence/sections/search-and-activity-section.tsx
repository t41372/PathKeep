/**
 * Search and activity-mix sections for `/intelligence`.
 *
 * Why this file exists:
 * - Search activity and activity mix are sibling half-width cards in the
 *   current IA, so keeping them together makes layout-level iteration easier.
 * - Pulling these interactive sections out of the route aggregator keeps the
 *   main coordinator readable.
 *
 * Main declarations:
 * - `SearchActivitySection`
 * - `ActivityMixSection`
 */

import { useEffect, useMemo, useState } from 'react'
import { Link } from 'react-router-dom'
import { ExplainabilityPanel } from '../../../components/intelligence/explainability-panel'
import { QueryFamilyCard } from '../../../components/intelligence/query-family-card'
import { SearchKeywordsBrowser } from '../../../components/intelligence/search-keywords-browser'
import { IntelligenceSectionMeta } from '../../../components/intelligence/section-meta'
import {
  useAsyncData,
  type DateRange,
  type EngineRanking,
  type SearchConcept,
  type TopSite,
} from '../../../lib/core-intelligence'
import * as api from '../../../lib/core-intelligence/api'
import type { ResolvedLanguage } from '../../../lib/i18n/catalog'
import { intelligenceCategoryLabel } from '../copy'
import { scheduleIdlePrefetch } from './idle-prefetch'
import { IntelligenceSectionBody } from './section-body'
import { firstSectionMeta, formatNumber, type T } from './shared'

export function SearchActivitySection({
  dateRange,
  language,
  profileId,
  queryFamilyHref,
  scopeLabel,
  trailHref,
  t,
}: {
  dateRange: DateRange
  language: ResolvedLanguage
  profileId: string | null
  queryFamilyHref: (familyId: string, profileId?: string | null) => string
  scopeLabel: string
  trailHref: (trailId: string, profileId?: string | null) => string
  t: T
}) {
  const [tab, setTab] = useState<
    'engines' | 'concepts' | 'queries' | 'families'
  >('engines')
  const engines = useAsyncData(
    () => api.getSearchEngineRanking(dateRange, profileId),
    [dateRange, profileId],
    {
      getCached: () => api.peekSearchEngineRanking(dateRange, profileId),
    },
  )
  const concepts = useAsyncData(
    () => api.getTopSearchConcepts(dateRange, profileId, 50),
    [dateRange, profileId],
    {
      getCached: () => api.peekTopSearchConcepts(dateRange, profileId, 50),
    },
  )
  const meta = firstSectionMeta(engines.data, concepts.data)

  useEffect(() => {
    return scheduleIdlePrefetch(() => {
      void api.getSearchQueries(dateRange, {
        profileId,
        pagination: { page: 0, pageSize: 20 },
      })
      void api.getQueryFamilies(dateRange, profileId, {
        page: 0,
        pageSize: 10,
      })
    })
  }, [dateRange, profileId])

  return (
    <section className="intelligence-section search-activity-section">
      <div className="intelligence-section__title-row">
        <h2 className="intelligence-section__title">
          {t('searchActivityTitle')}
        </h2>
        {meta ? (
          <IntelligenceSectionMeta meta={meta} scopeLabel={scopeLabel} />
        ) : null}
      </div>
      <div className="intelligence-tabs" role="tablist">
        {(['engines', 'concepts', 'queries', 'families'] as const).map(
          (key) => (
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
          ),
        )}
      </div>
      <IntelligenceSectionBody className="intelligence-tab-content">
        {tab === 'engines' ? (
          <EngineRankingPanel
            data={engines.data?.data ?? null}
            loading={engines.loading}
            t={t}
          />
        ) : null}
        {tab === 'concepts' ? (
          <ConceptBarChartPanel
            data={concepts.data?.data ?? null}
            loading={concepts.loading}
            t={t}
          />
        ) : null}
        {tab === 'queries' ? (
          <SearchKeywordsBrowser
            dateRange={dateRange}
            engineOptions={engines.data?.data ?? []}
            language={language}
            profileId={profileId}
            queryFamilyHref={queryFamilyHref}
            t={t}
            trailHref={trailHref}
          />
        ) : null}
        {tab === 'families' ? (
          <QueryFamiliesPanel
            dateRange={dateRange}
            profileId={profileId}
            queryFamilyHref={queryFamilyHref}
            t={t}
          />
        ) : null}
      </IntelligenceSectionBody>
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

  const max = Math.max(data[0].searchCount, 1)
  return (
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
  )
}

function ConceptBarChartPanel({
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
    <div className="search-concepts-chart">
      <p className="search-concepts-chart__summary">
        {t('conceptChartSummary')}
      </p>
      <div
        className="search-concepts-chart__bars"
        role="img"
        aria-label={t('conceptCloudLabel')}
      >
        {data.map((concept) => (
          <div key={concept.term} className="engine-ranking__row">
            <div className="search-concepts-chart__label-group">
              <span className="engine-ranking__name">{concept.term}</span>
              {concept.engines.length > 0 ? (
                <span className="search-concepts-chart__engines">
                  {concept.engines.join(', ')}
                </span>
              ) : null}
            </div>
            <span className="engine-ranking__bar">
              <span
                className="engine-ranking__bar-fill"
                style={{
                  width: `${Math.round((concept.frequency / maxFrequency) * 100)}%`,
                }}
                title={t('conceptTooltip', {
                  term: concept.term,
                  count: concept.frequency,
                })}
              />
            </span>
            <span className="engine-ranking__count">
              {formatNumber(concept.frequency)}
            </span>
          </div>
        ))}
      </div>
    </div>
  )
}

function QueryFamiliesPanel({
  dateRange,
  profileId,
  queryFamilyHref,
  t,
}: {
  dateRange: DateRange
  profileId: string | null
  queryFamilyHref: (familyId: string, profileId?: string | null) => string
  t: T
}) {
  const { data, loading, error } = useAsyncData(
    () => api.getQueryFamilies(dateRange, profileId, { page: 0, pageSize: 10 }),
    [dateRange, profileId],
    {
      getCached: () =>
        api.peekQueryFamilies(dateRange, profileId, { page: 0, pageSize: 10 }),
    },
  )
  const families = data?.data.families ?? []

  if (loading) {
    return <div className="intelligence-skeleton intelligence-skeleton--list" />
  }

  if (error || families.length === 0) {
    return (
      <div className="intelligence-empty">
        <p className="intelligence-empty__text">
          {error || t('queryFamiliesPlaceholder')}
        </p>
      </div>
    )
  }

  return (
    <div className="query-families">
      {families.map((family) => (
        <QueryFamilyCard
          key={family.familyId}
          family={family}
          footer={
            <ExplainabilityPanel
              entityType="query_family"
              entityId={family.familyId}
              t={t}
            />
          }
          href={queryFamilyHref(family.familyId)}
          linkMode="anchor"
          memberCountLabel={t('queryFamilyMemberCount')}
          moreLabel={(hiddenCount) => `+${hiddenCount} ${t('queryFamilyMore')}`}
          showDates
          showMembers
        />
      ))}
    </div>
  )
}

export function ActivityMixSection({
  dateRange,
  domainHref,
  language,
  profileId,
  scopeLabel,
  t,
}: {
  dateRange: DateRange
  domainHref: (domain: string) => string
  language: ResolvedLanguage
  profileId: string | null
  scopeLabel: string
  t: T
}) {
  const mixResult = useAsyncData(
    () => api.getActivityMix(dateRange, profileId),
    [dateRange, profileId],
    {
      getCached: () => api.peekActivityMix(dateRange, profileId),
    },
  )
  const topSitesResult = useAsyncData(
    () => api.getTopSites(dateRange, profileId, 'visit_count', 40),
    [dateRange, profileId],
    {
      getCached: () =>
        api.peekTopSites(dateRange, profileId, 'visit_count', 40),
    },
  )
  const mix = mixResult.data?.data ?? null

  const sortedCategories = useMemo(
    () =>
      mix
        ? [...mix.categories]
            .filter((c) => c.share > 0)
            .sort((a, b) => b.share - a.share)
        : [],
    [mix],
  )

  const examplesByCategory = useMemo(() => {
    const map = new Map<string, TopSite[]>()
    for (const site of topSitesResult.data?.data ?? []) {
      const current = map.get(site.domainCategory) ?? []
      if (
        !current.some(
          (entry) => entry.registrableDomain === site.registrableDomain,
        ) &&
        current.length < 3
      ) {
        current.push(site)
        map.set(site.domainCategory, current)
      }
    }
    return map
  }, [topSitesResult.data])

  const [hoveredCategory, setHoveredCategory] = useState<string | null>(null)
  const hoveredDetail = hoveredCategory
    ? sortedCategories.find((c) => c.domainCategory === hoveredCategory)
    : undefined

  return (
    <section className="intelligence-section activity-mix-section">
      <div className="intelligence-section__title-row">
        <h2 className="intelligence-section__title">{t('activityMixTitle')}</h2>
        {mixResult.data ? (
          <IntelligenceSectionMeta
            meta={mixResult.data.meta}
            scopeLabel={scopeLabel}
          />
        ) : null}
      </div>
      <p className="intelligence-section__help">{t('activityMixHelp')}</p>
      {mixResult.loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--chart" />
      ) : !mix || mix.categories.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('activityMixEmpty')}</p>
        </div>
      ) : (
        <IntelligenceSectionBody className="activity-mix">
          <div className="activity-mix__composition">
            <div
              className="activity-mix__stacked-bar"
              data-testid="activity-mix-stacked-bar"
              role="group"
              aria-label={t('activityMixTitle')}
            >
              {sortedCategories.map((category) => {
                const sharePercent = Math.round(category.share * 100)
                const label = intelligenceCategoryLabel(
                  language,
                  t,
                  category.domainCategory,
                )
                const tooltipText = t('activityMixTooltip', {
                  category: label,
                  percent: sharePercent,
                  count: category.visitCount,
                })
                return (
                  <span
                    key={category.domainCategory}
                    className={`activity-mix__segment${hoveredCategory === category.domainCategory ? ' activity-mix__segment--active' : ''}`}
                    data-category={category.domainCategory}
                    style={{ width: `${category.share * 100}%` }}
                    role="img"
                    tabIndex={0}
                    aria-label={tooltipText}
                    onMouseEnter={() =>
                      setHoveredCategory(category.domainCategory)
                    }
                    onMouseLeave={() => setHoveredCategory(null)}
                    onFocus={() => setHoveredCategory(category.domainCategory)}
                    onBlur={() => setHoveredCategory(null)}
                    title={tooltipText}
                  />
                )
              })}
            </div>
            {hoveredDetail ? (
              <div
                className="activity-mix__hover-detail"
                data-testid="activity-mix-hover-detail"
              >
                <span className="activity-mix__hover-category">
                  {intelligenceCategoryLabel(
                    language,
                    t,
                    hoveredDetail.domainCategory,
                  )}
                </span>
                <span className="activity-mix__hover-stats">
                  {Math.round(hoveredDetail.share * 100)}% &middot;{' '}
                  {formatNumber(hoveredDetail.visitCount)} {t('visits')}
                </span>
              </div>
            ) : null}
          </div>

          <div className="activity-mix__legend">
            {sortedCategories.map((category) => {
              const change = mix.changeVsPrevious.find(
                (entry) => entry.domainCategory === category.domainCategory,
              )
              const changePoints = change?.changePoints ?? 0
              const examples =
                examplesByCategory.get(category.domainCategory) ?? []
              const label = intelligenceCategoryLabel(
                language,
                t,
                category.domainCategory,
              )

              return (
                <div
                  key={category.domainCategory}
                  className="activity-mix__legend-row"
                >
                  <span className="activity-mix__legend-main">
                    <span
                      className="activity-mix__legend-swatch activity-mix__segment"
                      data-category={category.domainCategory}
                    />
                    <span className="activity-mix__legend-label">{label}</span>
                  </span>
                  <span className="activity-mix__share">
                    {Math.round(category.share * 100)}%
                  </span>
                  <span className="activity-mix__legend-detail">
                    {examples.length > 0 ? (
                      <span className="activity-mix__examples">
                        {t('activityMixExamples', {
                          domains: examples
                            .map(
                              (entry) =>
                                entry.displayName ?? entry.registrableDomain,
                            )
                            .join(', '),
                        })}
                        <span className="activity-mix__example-links">
                          {examples.map((entry) => (
                            <Link
                              key={entry.registrableDomain}
                              className="intelligence-link"
                              to={domainHref(entry.registrableDomain)}
                            >
                              {entry.displayName ?? entry.registrableDomain}
                            </Link>
                          ))}
                        </span>
                      </span>
                    ) : null}
                    {changePoints !== 0 ? (
                      <span
                        className={`activity-mix__change activity-mix__change--${changePoints > 0 ? 'positive' : 'negative'}`}
                      >
                        {changePoints > 0 ? '+' : ''}
                        {Math.round(changePoints * 100)}%
                      </span>
                    ) : null}
                  </span>
                </div>
              )
            })}
          </div>
        </IntelligenceSectionBody>
      )}
    </section>
  )
}
