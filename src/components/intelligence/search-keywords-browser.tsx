/**
 * Shared search-keyword browser used by Intelligence overview and Domain Insights.
 *
 * Why this file exists:
 * - `Search Keywords` is now a bounded review surface shared across multiple
 *   intelligence consumers instead of an overview-only additive list.
 * - Keeping filters, pagination, and row actions in one place reduces drift
 *   between `/intelligence` and `/intelligence/domain/:domain`.
 *
 * Source-of-truth:
 * - `docs/design/search-activity-keyword-browser-tradeoff.md`
 * - `docs/design/screens-and-nav.md`
 * - `docs/features/core-intelligence-ultimate-design.md`
 */

import { useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import {
  useAsyncData,
  type DateRange,
  type SearchQueryRow,
  type SearchQuerySort,
} from '../../lib/core-intelligence'
import * as api from '../../lib/core-intelligence/api'
import type { ResolvedLanguage } from '../../lib/i18n'
import { evidenceHref } from '../../lib/intelligence-links'
import { clampSearchKeywordPage } from './search-keywords-browser-helpers'

type Translate = (
  key: string,
  params?: Record<string, string | number>,
) => string

const PAGE_SIZE_OPTIONS = [20, 50, 100]

interface SearchEngineOption {
  displayName?: string | null
  searchEngine: string
}

export function SearchKeywordsBrowser({
  className,
  dateRange,
  domain,
  engineOptions,
  help,
  hideWhenEmpty = false,
  language,
  profileId,
  queryFamilyHref,
  title,
  trailHref,
  t,
}: {
  className?: string
  dateRange: DateRange
  domain?: string | null
  engineOptions?: SearchEngineOption[]
  help?: string
  hideWhenEmpty?: boolean
  language: ResolvedLanguage
  profileId: string | null
  queryFamilyHref: (familyId: string, profileId?: string | null) => string
  title?: string
  trailHref: (trailId: string, profileId?: string | null) => string
  t: Translate
}) {
  const [sort, setSort] = useState<SearchQuerySort>('newest')
  const [engine, setEngine] = useState('')
  const [query, setQuery] = useState('')
  const [page, setPage] = useState(0)
  // Stryker disable next-line StringLiteral: the page-sync effect below immediately normalizes the rendered input for page 0.
  const [pageInput, setPageInput] = useState('1')
  const [pageSize, setPageSize] = useState(PAGE_SIZE_OPTIONS[0])
  const [nestedRange, setNestedRange] = useState<DateRange>(dateRange)
  const effectiveRange = clampNestedRange(nestedRange, dateRange)

  useEffect(() => {
    setNestedRange({
      start: dateRange.start,
      end: dateRange.end,
    })
    setPage(0)
    // Stryker disable next-line StringLiteral: resetting page to 0 triggers the page-sync effect, so the intermediate literal is not separately observable.
    setPageInput('1')
  }, [dateRange.end, dateRange.start, domain])

  useEffect(() => {
    setPage(0)
    // Stryker disable next-line StringLiteral: resetting page to 0 triggers the page-sync effect, so the intermediate literal is not separately observable.
    setPageInput('1')
  }, [
    domain,
    effectiveRange.end,
    effectiveRange.start,
    engine,
    pageSize,
    query,
    sort,
  ])

  useEffect(() => {
    setPageInput(String(page + 1))
  }, [page])

  const requestOptions = {
    profileId,
    domain,
    engine: engine || undefined,
    query: query || undefined,
    sort,
    pagination: { page, pageSize },
  }
  const { data, error, loading } = useAsyncData(
    () => api.getSearchQueries(effectiveRange, requestOptions),
    [domain, effectiveRange, page, pageSize, profileId, query, sort, engine],
    {
      getCached: () => api.peekSearchQueries(effectiveRange, requestOptions),
    },
  )
  const rows = data?.data.rows ?? []
  const total = data?.data.total ?? 0
  const pageCount = Math.max(1, Math.ceil(total / pageSize))
  const canGoPrevious = page > 0
  const canGoNext = page + 1 < pageCount

  const source = engineOptions?.length
    ? engineOptions
    : rows.map((row) => ({
        searchEngine: row.searchEngine,
        displayName: row.displayName,
      }))
  const dedupedEngines = new Map<string, SearchEngineOption>()
  for (const item of source) {
    if (!dedupedEngines.has(item.searchEngine)) {
      dedupedEngines.set(item.searchEngine, item)
    }
  }
  const availableEngines = [...dedupedEngines.values()].sort((left, right) =>
    (left.displayName ?? left.searchEngine).localeCompare(
      right.displayName ?? right.searchEngine,
    ),
  )

  if (hideWhenEmpty && !loading && !error && data && total === 0) {
    return null
  }

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

  return (
    <div
      className={`search-keywords-browser${className ? ` ${className}` : ''}`}
    >
      {title ? (
        <div className="search-keywords-browser__header">
          <h3 className="domain-deep-dive__section-title">{title}</h3>
          {help ? (
            <p className="search-keywords-browser__help">{help}</p>
          ) : null}
        </div>
      ) : null}
      <div className="search-keywords-browser__controls">
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
          {availableEngines.map((item) => (
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

      <div className="search-keywords-browser__subrange">
        <label className="search-keywords-browser__date-field">
          <span>{t('searchQueriesRangeStart')}</span>
          <input
            type="date"
            value={nestedRange.start}
            min={dateRange.start}
            max={dateRange.end}
            onChange={(event) =>
              setNestedRange((current) => ({
                ...current,
                start: event.target.value || dateRange.start,
              }))
            }
          />
        </label>
        <label className="search-keywords-browser__date-field">
          <span>{t('searchQueriesRangeEnd')}</span>
          <input
            type="date"
            value={nestedRange.end}
            min={dateRange.start}
            max={dateRange.end}
            onChange={(event) =>
              setNestedRange((current) => ({
                ...current,
                end: event.target.value || dateRange.end,
              }))
            }
          />
        </label>
        <button
          className="btn-secondary"
          type="button"
          onClick={() => setNestedRange(dateRange)}
        >
          {t('searchQueriesResetRange')}
        </button>
      </div>

      {rows.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('searchQueriesEmpty')}</p>
        </div>
      ) : (
        <>
          <div className="search-queries__list">
            {rows.map((row) => (
              <SearchKeywordRowCard
                key={`${row.searchEngine}:${row.normalizedQuery}:${row.visitId}:${row.searchedAtMs}`}
                dateRange={effectiveRange}
                language={language}
                queryFamilyHref={queryFamilyHref}
                row={row}
                t={t}
                trailHref={trailHref}
              />
            ))}
          </div>
          <div className="search-keywords-browser__pagination">
            <div className="search-keywords-browser__summary">
              <span>
                {t('searchQueriesPageCountSummary', {
                  current: page + 1,
                  total: pageCount,
                })}
              </span>
              <span>
                {t('searchQueriesResultsSummary', {
                  shown: rows.length,
                  total,
                })}
              </span>
            </div>
            <div className="search-keywords-browser__pagination-controls">
              <div className="search-keywords-browser__nav">
                <button
                  className="btn-secondary"
                  type="button"
                  onClick={() => setPage(0)}
                  disabled={!canGoPrevious}
                >
                  {t('searchQueriesFirstPage')}
                </button>
                <button
                  className="btn-secondary"
                  type="button"
                  onClick={() => setPage((current) => Math.max(0, current - 1))}
                  disabled={!canGoPrevious}
                >
                  {t('searchQueriesPreviousPage')}
                </button>
                <button
                  className="btn-secondary"
                  type="button"
                  onClick={() =>
                    // Stryker disable next-line ArithmeticOperator: canGoNext disables this control on the final page, so both clamps produce the same reachable pages.
                    setPage((current) => Math.min(pageCount - 1, current + 1))
                  }
                  disabled={!canGoNext}
                >
                  {t('searchQueriesNextPage')}
                </button>
                <button
                  className="btn-secondary"
                  type="button"
                  onClick={() => setPage(Math.max(0, pageCount - 1))}
                  disabled={!canGoNext}
                >
                  {t('searchQueriesLastPage')}
                </button>
              </div>
              <div className="search-keywords-browser__jump">
                <label className="search-keywords-browser__jump-field">
                  <span>{t('searchQueriesPageNumberLabel')}</span>
                  <input
                    inputMode="numeric"
                    min={1}
                    type="number"
                    value={pageInput}
                    onChange={(event) => setPageInput(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === 'Enter') {
                        event.preventDefault()
                        setPage(clampSearchKeywordPage(pageInput, pageCount))
                      }
                    }}
                  />
                </label>
                <button
                  className="btn-secondary"
                  type="button"
                  onClick={() =>
                    setPage(clampSearchKeywordPage(pageInput, pageCount))
                  }
                >
                  {t('searchQueriesJumpToPage')}
                </button>
                <label className="search-keywords-browser__jump-field">
                  <span>{t('searchQueriesPageSizeLabel')}</span>
                  <select
                    className="history-page-size__select"
                    value={pageSize}
                    onChange={(event) =>
                      setPageSize(Number(event.target.value))
                    }
                  >
                    {PAGE_SIZE_OPTIONS.map((option) => (
                      <option key={option} value={option}>
                        {t('searchQueriesPageSizeOption', { count: option })}
                      </option>
                    ))}
                  </select>
                </label>
              </div>
            </div>
          </div>
        </>
      )}
    </div>
  )
}

function SearchKeywordRowCard({
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
  t: Translate
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
        {[
          t('searchQueriesExactRepeat', { count: row.exactRepeatCount }),
          t('searchQueriesFamilyCount', { count: row.familyCount }),
        ].join(' · ')}
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

function clampNestedRange(range: DateRange, parent: DateRange): DateRange {
  const start = clampLocalDate(
    range.start || parent.start,
    parent.start,
    parent.end,
  )
  const end = clampLocalDate(range.end || parent.end, parent.start, parent.end)
  // Stryker disable next-line EqualityOperator: equal start/end returns the same single-day range through either branch.
  if (start <= end) {
    return { start, end }
  }
  return { start: end, end: start }
}

function clampLocalDate(value: string, min: string, max: string) {
  // Stryker disable next-line EqualityOperator: equality with the lower bound returns the same boundary value either way.
  if (value < min) {
    return min
  }
  // Stryker disable next-line EqualityOperator: equality with the upper bound returns the same boundary value either way.
  if (value > max) {
    return max
  }
  return value
}
