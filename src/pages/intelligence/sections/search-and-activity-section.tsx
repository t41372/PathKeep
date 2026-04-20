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

import { useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import { ExplainabilityPanel } from '../../../components/intelligence/explainability-panel'
import { QueryFamilyCard } from '../../../components/intelligence/query-family-card'
import { IntelligenceSectionMeta } from '../../../components/intelligence/section-meta'
import {
  useAsyncData,
  type DateRange,
  type EngineRanking,
  type SearchQueryRow,
  type SearchQuerySort,
  type SearchConcept,
  type TopSite,
} from '../../../lib/core-intelligence'
import * as api from '../../../lib/core-intelligence/api'
import type { ResolvedLanguage } from '../../../lib/i18n'
import { evidenceHref } from '../../../lib/intelligence'
import { intelligenceCategoryLabel } from '../copy'
import { IntelligenceSectionBody } from './section-body'
import { firstSectionMeta, formatNumber, type T } from './shared'

function scheduleIdlePrefetch(callback: () => void) {
  if (typeof window === 'undefined') {
    return () => {}
  }

  const idleWindow = window as Window & {
    requestIdleCallback?: (
      cb: IdleRequestCallback,
      options?: IdleRequestOptions,
    ) => number
    cancelIdleCallback?: (handle: number) => void
  }

  if (typeof idleWindow.requestIdleCallback === 'function') {
    const handle = idleWindow.requestIdleCallback(() => callback(), {
      timeout: 1200,
    })
    return () => idleWindow.cancelIdleCallback?.(handle)
  }

  const handle = window.setTimeout(callback, 160)
  return () => window.clearTimeout(handle)
}

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
          <ConceptCloudPanel
            data={concepts.data?.data ?? null}
            loading={concepts.loading}
            t={t}
          />
        ) : null}
        {tab === 'queries' ? (
          <RecentQueriesPanel
            dateRange={dateRange}
            engines={engines.data?.data ?? []}
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

  const max = Math.max(data[0]?.searchCount ?? 0, 1)
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

function RecentQueriesPanel({
  dateRange,
  engines,
  language,
  profileId,
  queryFamilyHref,
  t,
  trailHref,
}: {
  dateRange: DateRange
  engines: EngineRanking[]
  language: ResolvedLanguage
  profileId: string | null
  queryFamilyHref: (familyId: string, profileId?: string | null) => string
  t: T
  trailHref: (trailId: string, profileId?: string | null) => string
}) {
  const [sort, setSort] = useState<SearchQuerySort>('newest')
  const [engine, setEngine] = useState('')
  const [query, setQuery] = useState('')
  const [loadState, setLoadState] = useState({ key: '', segments: 1 })
  const loadKey = `${profileId ?? 'all'}:${dateRange.start}:${dateRange.end}:${engine}:${query}:${sort}`
  const loadSegments = loadState.key === loadKey ? loadState.segments : 1

  const { data, loading, error } = useAsyncData(
    () =>
      api.getSearchQueries(dateRange, {
        profileId,
        engine: engine || undefined,
        query: query || undefined,
        sort,
        pagination: { page: 0, pageSize: loadSegments * 20 },
      }),
    [dateRange, engine, loadSegments, profileId, query, sort],
    {
      getCached: () =>
        api.peekSearchQueries(dateRange, {
          profileId,
          engine: engine || undefined,
          query: query || undefined,
          sort,
          pagination: { page: 0, pageSize: loadSegments * 20 },
        }),
    },
  )

  if (loading && !data) {
    return <div className="intelligence-skeleton intelligence-skeleton--list" />
  }

  if (error) {
    return (
      <div className="intelligence-empty">
        <p className="intelligence-empty__text">{error}</p>
      </div>
    )
  }

  const rows = data?.data.rows ?? []
  const hasMore = Boolean(data && rows.length < data.data.total)

  return (
    <div className="search-queries">
      <div className="search-queries__controls">
        <input
          className="top-sites-controls__search"
          type="search"
          value={query}
          placeholder={t('searchQueriesFilterPlaceholder')}
          aria-label={t('searchQueriesFilterPlaceholder')}
          onChange={(event) => setQuery(event.target.value)}
        />
        <select
          className="top-sites-controls__sort"
          value={engine}
          aria-label={t('searchQueriesEngineFilter')}
          onChange={(event) => setEngine(event.target.value)}
        >
          <option value="">{t('searchQueriesAllEngines')}</option>
          {engines.map((item) => (
            <option key={item.searchEngine} value={item.searchEngine}>
              {item.displayName ?? item.searchEngine}
            </option>
          ))}
        </select>
        <select
          className="top-sites-controls__sort"
          value={sort}
          aria-label={t('searchQueriesSort')}
          onChange={(event) => setSort(event.target.value as SearchQuerySort)}
        >
          <option value="newest">{t('searchQueriesSortNewest')}</option>
          <option value="exact-frequency">
            {t('searchQueriesSortExactFrequency')}
          </option>
          <option value="family-frequency">
            {t('searchQueriesSortFamilyFrequency')}
          </option>
          <option value="alphabetical">
            {t('searchQueriesSortAlphabetical')}
          </option>
        </select>
      </div>
      {rows.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('searchQueriesEmpty')}</p>
        </div>
      ) : (
        <>
          <div className="search-queries__list">
            {rows.map((row) => (
              <SearchQueryRowCard
                key={`${row.searchEngine}:${row.normalizedQuery}`}
                dateRange={dateRange}
                language={language}
                queryFamilyHref={queryFamilyHref}
                row={row}
                t={t}
                trailHref={trailHref}
              />
            ))}
          </div>
          {hasMore ? (
            <div className="search-queries__footer">
              <button
                className="btn-secondary"
                type="button"
                onClick={() =>
                  setLoadState((current) => ({
                    key: loadKey,
                    segments:
                      current.key === loadKey ? current.segments + 1 : 2,
                  }))
                }
              >
                {t('searchQueriesLoadMore')}
              </button>
            </div>
          ) : null}
        </>
      )}
    </div>
  )
}

function SearchQueryRowCard({
  dateRange,
  language,
  queryFamilyHref,
  row,
  t,
  trailHref,
}: {
  dateRange: DateRange
  language: ResolvedLanguage
  queryFamilyHref: (familyId: string, profileId?: string | null) => string
  row: SearchQueryRow
  t: T
  trailHref: (trailId: string, profileId?: string | null) => string
}) {
  const locale =
    language === 'zh-CN' ? 'zh-CN' : language === 'zh-TW' ? 'zh-TW' : 'en-US'
  const searchedAt = new Date(row.searchedAt).toLocaleString(locale, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  })

  return (
    <article className="search-query-card">
      <div className="search-query-card__header">
        <div className="search-query-card__title-group">
          <strong className="search-query-card__query">"{row.rawQuery}"</strong>
          <span className="search-query-card__engine">
            {row.displayName ?? row.searchEngine}
          </span>
        </div>
        <span className="search-query-card__timestamp">
          {t('searchQueriesSearchedAt', { time: searchedAt })}
        </span>
      </div>
      <p className="search-query-card__meta">
        {t('searchQueriesExactRepeat', { count: row.exactRepeatCount })} ·{' '}
        {t('searchQueriesFamilyCount', { count: row.familyCount })}
      </p>
      <p className="search-query-card__context">
        {row.trailInitialQuery
          ? t('searchQueriesTrailContext', {
              query: row.trailInitialQuery,
              count: row.trailReformulationCount ?? 0,
            })
          : t('searchQueriesNoTrail')}
      </p>
      <div className="search-query-card__actions">
        {row.familyId ? (
          <Link
            className="intelligence-link"
            to={queryFamilyHref(row.familyId, row.profileId)}
          >
            {t('searchQueriesOpenQueryFamily')}
          </Link>
        ) : null}
        {row.trailId ? (
          <Link
            className="intelligence-link"
            to={trailHref(row.trailId, row.profileId)}
          >
            {t('searchQueriesOpenTrail')}
          </Link>
        ) : null}
        <Link
          className="intelligence-link"
          to={evidenceHref({
            dateRange,
            profileId: row.profileId,
            title: row.rawQuery,
          })}
        >
          {t('searchQueriesOpenEvidence')}
        </Link>
      </div>
    </article>
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
  const examplesByCategory = new Map<string, TopSite[]>()

  for (const site of topSitesResult.data?.data ?? []) {
    const current = examplesByCategory.get(site.domainCategory) ?? []
    if (
      !current.some(
        (entry) => entry.registrableDomain === site.registrableDomain,
      ) &&
      current.length < 3
    ) {
      current.push(site)
      examplesByCategory.set(site.domainCategory, current)
    }
  }

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
          {mix.categories.map((category) => {
            const change = mix.changeVsPrevious.find(
              (entry) => entry.domainCategory === category.domainCategory,
            )
            const changePoints = change?.changePoints ?? 0
            const examples =
              examplesByCategory.get(category.domainCategory) ?? []
            return (
              <div key={category.domainCategory} className="activity-mix__row">
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
                <div className="activity-mix__meta">
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
                </div>
              </div>
            )
          })}
        </IntelligenceSectionBody>
      )}
    </section>
  )
}
