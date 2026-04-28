/**
 * @file search-keywords-browser.test.tsx
 * @description Interaction coverage for the shared Search Keywords browser.
 * @module components/intelligence
 *
 * ## Responsibilities
 * - Verify filter, sort, date-range, pagination, and link actions call the shared Core Intelligence API contract.
 * - Protect empty, hidden-empty, and error states used by Intelligence overview and domain deep dives.
 * - Keep search-keyword browsing behavior in the shared component instead of duplicating route tests.
 *
 * ## Not responsible for
 * - Re-testing route-specific section composition.
 * - Re-testing the backend search-query implementation.
 *
 * ## Dependencies
 * - Uses MemoryRouter for row action links and Core Intelligence API spies for request assertions.
 *
 * ## Performance notes
 * - The component only renders a bounded page of rows; tests keep fixtures intentionally small.
 */

import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import type {
  CoreIntelligenceSectionResult,
  DateRange,
  SearchQueryListResult,
  SearchQueryRow,
} from '../../lib/core-intelligence'
import * as api from '../../lib/core-intelligence/api'
import { SearchKeywordsBrowser } from './search-keywords-browser'
import { clampSearchKeywordPage } from './search-keywords-browser-helpers'

const parentRange: DateRange = {
  start: '2026-04-01',
  end: '2026-04-30',
}

const t = (key: string, params?: Record<string, string | number>) =>
  params ? `${key}:${JSON.stringify(params)}` : key

describe('SearchKeywordsBrowser', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    vi.spyOn(api, 'peekSearchQueries').mockReturnValue(null)
  })

  test('filters, sorts, clamps ranges, paginates, and renders row actions', async () => {
    const user = userEvent.setup()
    const getSearchQueries = vi
      .spyOn(api, 'getSearchQueries')
      .mockImplementation((dateRange, options) =>
        Promise.resolve(
          searchQueriesResult(rowsFixture(), {
            dateRange,
            page: options?.pagination?.page ?? 0,
            pageSize: options?.pagination?.pageSize ?? 20,
            total: 65,
          }),
        ),
      )

    renderBrowser()

    expect(await screen.findByText('"sqlite wal"')).toBeVisible()
    expect(document.querySelector('.search-keywords-browser')).toHaveAttribute(
      'class',
      'search-keywords-browser',
    )
    expect(getSearchQueries.mock.calls[0]?.[1]).toEqual(
      expect.objectContaining({
        engine: undefined,
        pagination: { page: 0, pageSize: 20 },
        query: undefined,
      }),
    )
    expect(
      screen.getByLabelText('searchQueriesFilterPlaceholder'),
    ).toHaveAttribute('placeholder', 'searchQueriesFilterPlaceholder')
    expect(
      screen.getByRole('option', { name: 'searchQueriesAllEngines' }),
    ).toHaveValue('')
    expect(
      screen.getByRole('option', { name: 'searchQueriesSortNewest' }),
    ).toHaveValue('newest')
    expect(
      screen.getByRole('option', {
        name: 'searchQueriesSortExactFrequency',
      }),
    ).toHaveValue('exact-frequency')
    expect(
      screen.getByRole('option', {
        name: 'searchQueriesSortFamilyFrequency',
      }),
    ).toHaveValue('family-frequency')
    expect(
      screen.getByRole('option', { name: 'searchQueriesSortAlphabetical' }),
    ).toHaveValue('alphabetical')
    expect(screen.getByText('searchQueriesRangeStart')).toBeVisible()
    expect(screen.getByText('searchQueriesRangeEnd')).toBeVisible()
    expect(screen.getByText('searchQueriesPageSizeLabel')).toBeVisible()
    expect(
      screen.getByText('searchQueriesPageSizeOption:{"count":20}'),
    ).toBeVisible()
    expect(
      screen.getByText('searchQueriesPageCountSummary:{"current":1,"total":4}'),
    ).toBeVisible()
    expect(
      screen.getByText('searchQueriesResultsSummary:{"shown":2,"total":65}'),
    ).toBeVisible()
    expect(screen.getByText('searchQueriesOpenQueryFamily')).toHaveAttribute(
      'href',
      '/query-family/family-1?profile=chrome%3ADefault',
    )
    expect(screen.getByText('searchQueriesOpenTrail')).toHaveAttribute(
      'href',
      '/trail/trail-1?profile=chrome%3ADefault',
    )
    expect(screen.getAllByText('searchQueriesOpenEvidence')[0]).toHaveAttribute(
      'href',
      '/explorer?profileId=chrome%3ADefault&start=2026-04-01&end=2026-04-30&q=sqlite+wal',
    )
    const firstMeta = document.querySelector('.search-query-card__meta')
    expect(firstMeta).toHaveTextContent('searchQueriesExactRepeat:{"count":3}')
    expect(firstMeta).toHaveTextContent('·')
    expect(firstMeta).toHaveTextContent('searchQueriesFamilyCount:{"count":5}')
    expect(
      screen.getByText(
        `searchQueriesSearchedAt:${JSON.stringify({
          time: formattedSearchTime('2026-04-20T10:00:00Z', 'en-US'),
        })}`,
      ),
    ).toBeVisible()
    expect(screen.getByText('searchQueriesNoTrail')).toBeVisible()
    expect(
      screen.getByRole('button', { name: 'searchQueriesFirstPage' }),
    ).toBeDisabled()
    expect(
      screen.getByRole('button', { name: 'searchQueriesPreviousPage' }),
    ).toBeDisabled()

    await user.type(
      screen.getByLabelText('searchQueriesFilterPlaceholder'),
      'duckdb',
    )
    await user.selectOptions(
      screen.getByLabelText('searchQueriesEngineFilter'),
      'github',
    )
    await user.selectOptions(
      screen.getByLabelText('searchQueriesSort'),
      'alphabetical',
    )
    await waitFor(() =>
      expect(getSearchQueries).toHaveBeenLastCalledWith(
        expect.anything(),
        expect.objectContaining({
          engine: 'github',
          query: 'duckdb',
          sort: 'alphabetical',
        }),
      ),
    )

    const [startInput, endInput] = screen.getAllByDisplayValue(/^2026-04-/)
    fireEvent.change(startInput, { target: { value: '' } })
    fireEvent.change(endInput, { target: { value: '' } })
    await waitFor(() =>
      expect(getSearchQueries).toHaveBeenLastCalledWith(
        parentRange,
        expect.anything(),
      ),
    )

    fireEvent.change(startInput, { target: { value: '2026-05-20' } })
    fireEvent.change(endInput, { target: { value: '2026-03-10' } })
    await waitFor(() =>
      expect(getSearchQueries).toHaveBeenLastCalledWith(
        { start: '2026-04-01', end: '2026-04-30' },
        expect.anything(),
      ),
    )

    await user.click(
      screen.getByRole('button', { name: 'searchQueriesResetRange' }),
    )
    await waitFor(() =>
      expect(getSearchQueries).toHaveBeenLastCalledWith(
        parentRange,
        expect.anything(),
      ),
    )

    const [validStartInput, validEndInput] =
      screen.getAllByDisplayValue(/^2026-04-/)
    fireEvent.change(validStartInput, { target: { value: '2026-04-10' } })
    await waitFor(() =>
      expect(getSearchQueries).toHaveBeenLastCalledWith(
        { start: '2026-04-10', end: '2026-04-30' },
        expect.anything(),
      ),
    )
    fireEvent.change(validEndInput, { target: { value: '2026-04-20' } })
    await waitFor(() =>
      expect(getSearchQueries).toHaveBeenLastCalledWith(
        { start: '2026-04-10', end: '2026-04-20' },
        expect.anything(),
      ),
    )
    fireEvent.change(validStartInput, { target: { value: '2026-03-01' } })
    await waitFor(() =>
      expect(getSearchQueries).toHaveBeenLastCalledWith(
        { start: '2026-04-01', end: '2026-04-20' },
        expect.anything(),
      ),
    )
    fireEvent.change(validEndInput, { target: { value: '2026-05-01' } })
    await waitFor(() =>
      expect(getSearchQueries).toHaveBeenLastCalledWith(
        parentRange,
        expect.anything(),
      ),
    )
    fireEvent.change(validStartInput, { target: { value: '2026-04-20' } })
    fireEvent.change(validEndInput, { target: { value: '2026-04-10' } })
    await waitFor(() =>
      expect(getSearchQueries).toHaveBeenLastCalledWith(
        { start: '2026-04-10', end: '2026-04-20' },
        expect.anything(),
      ),
    )

    await user.click(
      screen.getByRole('button', { name: 'searchQueriesNextPage' }),
    )
    await waitFor(() =>
      expect(getSearchQueries).toHaveBeenLastCalledWith(
        expect.anything(),
        expect.objectContaining({
          pagination: { page: 1, pageSize: 20 },
        }),
      ),
    )

    await user.click(
      screen.getByRole('button', { name: 'searchQueriesLastPage' }),
    )
    await waitFor(() =>
      expect(getSearchQueries).toHaveBeenLastCalledWith(
        expect.anything(),
        expect.objectContaining({
          pagination: { page: 3, pageSize: 20 },
        }),
      ),
    )
    expect(screen.getByLabelText('searchQueriesPageNumberLabel')).toHaveValue(4)
    expect(
      screen.getByRole('button', { name: 'searchQueriesNextPage' }),
    ).toBeDisabled()
    expect(
      screen.getByRole('button', { name: 'searchQueriesLastPage' }),
    ).toBeDisabled()

    await user.click(
      screen.getByRole('button', { name: 'searchQueriesPreviousPage' }),
    )
    await waitFor(() =>
      expect(getSearchQueries).toHaveBeenLastCalledWith(
        expect.anything(),
        expect.objectContaining({
          pagination: { page: 2, pageSize: 20 },
        }),
      ),
    )

    await user.click(
      screen.getByRole('button', { name: 'searchQueriesFirstPage' }),
    )
    await waitFor(() =>
      expect(getSearchQueries).toHaveBeenLastCalledWith(
        expect.anything(),
        expect.objectContaining({
          pagination: { page: 0, pageSize: 20 },
        }),
      ),
    )

    const pageInput = screen.getByLabelText<HTMLInputElement>(
      'searchQueriesPageNumberLabel',
    )
    const callsBeforeEscape = getSearchQueries.mock.calls.length
    fireEvent.change(pageInput, { target: { value: '2' } })
    fireEvent.keyDown(pageInput, { key: 'Escape' })
    await flushMicrotasks()
    expect(getSearchQueries.mock.calls).toHaveLength(callsBeforeEscape)

    fireEvent.change(pageInput, { target: { value: 'Infinity' } })
    fireEvent.keyDown(pageInput, { key: 'Escape' })
    await user.click(
      screen.getByRole('button', { name: 'searchQueriesJumpToPage' }),
    )
    expect(pageInput.value).toBe('')

    fireEvent.change(pageInput, { target: { value: 'not-a-page' } })
    fireEvent.keyDown(pageInput, { key: 'Enter' })
    await waitFor(() =>
      expect(getSearchQueries).toHaveBeenLastCalledWith(
        expect.anything(),
        expect.objectContaining({
          pagination: { page: 0, pageSize: 20 },
        }),
      ),
    )

    fireEvent.change(pageInput, { target: { value: '99' } })
    fireEvent.keyDown(pageInput, { key: 'Enter' })
    await waitFor(() =>
      expect(getSearchQueries).toHaveBeenLastCalledWith(
        expect.anything(),
        expect.objectContaining({
          pagination: { page: 3, pageSize: 20 },
        }),
      ),
    )

    await user.selectOptions(
      screen.getByLabelText('searchQueriesPageSizeLabel'),
      '50',
    )
    await waitFor(() =>
      expect(getSearchQueries).toHaveBeenLastCalledWith(
        expect.anything(),
        expect.objectContaining({
          pagination: { page: 0, pageSize: 50 },
        }),
      ),
    )
    expect(
      screen.getByText('searchQueriesPageSizeOption:{"count":50}'),
    ).toBeVisible()

    await user.click(
      screen.getByRole('button', { name: 'searchQueriesResetRange' }),
    )
    await waitFor(() =>
      expect(getSearchQueries).toHaveBeenLastCalledWith(
        parentRange,
        expect.anything(),
      ),
    )
  })

  test('uses cached query rows immediately and shows a skeleton only without cached data', () => {
    const cached = searchQueriesResult(rowsFixture(), { total: 2 })
    vi.spyOn(api, 'peekSearchQueries').mockReturnValue(cached)
    vi.spyOn(api, 'getSearchQueries').mockReturnValue(new Promise(() => {}))

    const { container, unmount } = renderBrowser()

    expect(screen.getByText('"sqlite wal"')).toBeVisible()
    expect(
      container.querySelector('.intelligence-skeleton--list'),
    ).not.toBeInTheDocument()

    unmount()
    vi.restoreAllMocks()
    vi.spyOn(api, 'peekSearchQueries').mockReturnValue(null)
    vi.spyOn(api, 'getSearchQueries').mockReturnValue(new Promise(() => {}))

    const loadingRender = renderBrowser()
    expect(
      loadingRender.container.querySelector('.intelligence-skeleton--list'),
    ).toBeInTheDocument()
  })

  test('uses stable keys for duplicate query rows', async () => {
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
    vi.spyOn(api, 'getSearchQueries').mockResolvedValue(
      searchQueriesResult([
        rowsFixture()[0],
        {
          ...rowsFixture()[0],
          visitId: 99,
          searchedAt: '2026-04-20T11:00:00Z',
        },
      ]),
    )

    try {
      renderBrowser()
      expect(await screen.findAllByText('"sqlite wal"')).toHaveLength(2)
      expect(errorSpy).not.toHaveBeenCalled()
    } finally {
      errorSpy.mockRestore()
    }
  })

  test('resets nested range and pagination when the parent scope changes', async () => {
    const user = userEvent.setup()
    const getSearchQueries = vi
      .spyOn(api, 'getSearchQueries')
      .mockImplementation((dateRange, options) =>
        Promise.resolve(
          searchQueriesResult(rowsFixture(), {
            dateRange,
            page: options?.pagination?.page ?? 0,
            pageSize: options?.pagination?.pageSize ?? 20,
            total: 65,
          }),
        ),
      )

    const { rerender } = render(
      <MemoryRouter>
        <SearchKeywordsBrowser
          className="search-keywords-browser--audit"
          dateRange={parentRange}
          domain="sqlite.org"
          language="en"
          profileId={null}
          queryFamilyHref={queryFamilyHref}
          t={t}
          trailHref={trailHref}
        />
      </MemoryRouter>,
    )

    expect(await screen.findByText('"sqlite wal"')).toBeVisible()
    expect(document.querySelector('.search-keywords-browser')).toHaveClass(
      'search-keywords-browser',
      'search-keywords-browser--audit',
    )
    await user.click(
      screen.getByRole('button', { name: 'searchQueriesLastPage' }),
    )
    await waitFor(() =>
      expect(getSearchQueries).toHaveBeenLastCalledWith(
        expect.anything(),
        expect.objectContaining({
          domain: 'sqlite.org',
          pagination: { page: 3, pageSize: 20 },
        }),
      ),
    )

    rerender(
      <MemoryRouter>
        <SearchKeywordsBrowser
          className="search-keywords-browser--audit"
          dateRange={{ start: '2026-05-01', end: '2026-05-31' }}
          domain="example.com"
          language="en"
          profileId={null}
          queryFamilyHref={queryFamilyHref}
          t={t}
          trailHref={trailHref}
        />
      </MemoryRouter>,
    )

    await waitFor(() =>
      expect(getSearchQueries).toHaveBeenLastCalledWith(
        { start: '2026-05-01', end: '2026-05-31' },
        expect.objectContaining({
          domain: 'example.com',
          pagination: { page: 0, pageSize: 20 },
        }),
      ),
    )
    expect(screen.getByLabelText('searchQueriesPageNumberLabel')).toHaveValue(1)
    expect(screen.getByDisplayValue('2026-05-01')).toBeVisible()
    expect(screen.getByDisplayValue('2026-05-31')).toBeVisible()
  })

  test('deduplicates row-derived engines and renders locale/fallback row variants', async () => {
    vi.spyOn(api, 'getSearchQueries').mockResolvedValue(
      searchQueriesResult(fallbackRowsFixture(), { total: 3 }),
    )

    const { rerender } = render(
      <MemoryRouter>
        <SearchKeywordsBrowser
          dateRange={parentRange}
          engineOptions={[
            { searchEngine: 'plain' },
            { searchEngine: 'plain', displayName: 'Duplicate plain' },
            { searchEngine: 'zulu' },
            { searchEngine: 'alpha' },
          ]}
          language="zh-CN"
          profileId={null}
          queryFamilyHref={queryFamilyHref}
          t={t}
          title="Search Keywords"
          trailHref={trailHref}
        />
      </MemoryRouter>,
    )

    expect(await screen.findByText('"fallback display"')).toBeVisible()
    const engineSelect = screen.getByLabelText('searchQueriesEngineFilter')
    if (!(engineSelect instanceof HTMLSelectElement)) {
      throw new Error('Expected engine filter to render as a select element')
    }
    const engineOptions = Array.from(engineSelect.options).map((option) => ({
      label: option.textContent,
      value: option.getAttribute('value'),
    }))
    expect(engineOptions.slice(1)).toEqual([
      { label: 'alpha', value: 'alpha' },
      { label: 'plain', value: 'plain' },
      { label: 'zulu', value: 'zulu' },
    ])
    expect(screen.getAllByText('plain').length).toBeGreaterThanOrEqual(2)
    expect(screen.queryByText('Duplicate plain')).not.toBeInTheDocument()
    expect(
      document.querySelector('.search-query-card__engine'),
    ).toHaveTextContent('plain')
    const fallbackMeta = document.querySelector('.search-query-card__meta')
    expect(fallbackMeta).toHaveTextContent(
      'searchQueriesExactRepeat:{"count":2}',
    )
    expect(fallbackMeta).toHaveTextContent('·')
    expect(fallbackMeta).toHaveTextContent(
      'searchQueriesFamilyCount:{"count":4}',
    )
    expect(
      screen.getByText(
        `searchQueriesSearchedAt:${JSON.stringify({
          time: formattedSearchTime('2026-04-22T10:00:00Z', 'zh-CN'),
        })}`,
      ),
    ).toBeVisible()
    expect(
      screen.getByText(
        'searchQueriesTrailContext:{"query":"duckdb","count":0}',
      ),
    ).toBeVisible()
    expect(screen.getByRole('option', { name: 'alpha' })).toBeVisible()
    expect(screen.getByRole('option', { name: 'zulu' })).toBeVisible()

    rerender(
      <MemoryRouter>
        <SearchKeywordsBrowser
          dateRange={parentRange}
          language="zh-TW"
          profileId={null}
          queryFamilyHref={queryFamilyHref}
          t={t}
          trailHref={trailHref}
        />
      </MemoryRouter>,
    )

    expect(await screen.findByText('"fallback display"')).toBeVisible()
    expect(screen.getAllByText('plain').length).toBeGreaterThanOrEqual(1)
    expect(
      screen.getByText(
        `searchQueriesSearchedAt:${JSON.stringify({
          time: formattedSearchTime('2026-04-22T10:00:00Z', 'zh-TW'),
        })}`,
      ),
    ).toBeVisible()
  })

  test('renders empty, hidden-empty, and error states', async () => {
    const getSearchQueries = vi.spyOn(api, 'getSearchQueries')

    getSearchQueries.mockResolvedValueOnce(
      searchQueriesResult([], { total: 0 }),
    )
    const { rerender, container } = renderBrowser()

    expect(await screen.findByText('searchQueriesEmpty')).toBeVisible()

    getSearchQueries.mockResolvedValueOnce(
      searchQueriesResult(rowsFixture(), { total: 2 }),
    )
    rerender(
      <MemoryRouter>
        <SearchKeywordsBrowser
          dateRange={{ start: '2026-04-02', end: '2026-04-30' }}
          hideWhenEmpty
          language="en"
          profileId={null}
          queryFamilyHref={queryFamilyHref}
          t={t}
          trailHref={trailHref}
        />
      </MemoryRouter>,
    )
    expect(await screen.findByText('"sqlite wal"')).toBeVisible()

    getSearchQueries.mockResolvedValueOnce(
      searchQueriesResult([], { total: 0 }),
    )
    rerender(
      <MemoryRouter>
        <SearchKeywordsBrowser
          dateRange={parentRange}
          hideWhenEmpty
          language="en"
          profileId={null}
          queryFamilyHref={queryFamilyHref}
          t={t}
          trailHref={trailHref}
        />
      </MemoryRouter>,
    )
    await waitFor(() =>
      expect(container.querySelector('.search-keywords-browser')).toBeNull(),
    )

    getSearchQueries.mockRejectedValueOnce(new Error('search unavailable'))
    rerender(
      <MemoryRouter>
        <SearchKeywordsBrowser
          dateRange={{ start: '2026-05-01', end: '2026-05-31' }}
          language="en"
          profileId={null}
          queryFamilyHref={queryFamilyHref}
          t={t}
          trailHref={trailHref}
        />
      </MemoryRouter>,
    )
    expect(await screen.findByText('search unavailable')).toBeVisible()
  })

  test('normalizes empty parent ranges and fractional page jumps', async () => {
    const getSearchQueries = vi
      .spyOn(api, 'getSearchQueries')
      .mockImplementation((dateRange, options) =>
        Promise.resolve(
          searchQueriesResult(rowsFixture(), {
            dateRange,
            page: options?.pagination?.page ?? 0,
            pageSize: options?.pagination?.pageSize ?? 20,
            total: 65,
          }),
        ),
      )

    render(
      <MemoryRouter>
        <SearchKeywordsBrowser
          dateRange={{ start: '', end: '' }}
          language="en"
          profileId={null}
          queryFamilyHref={queryFamilyHref}
          t={t}
          trailHref={trailHref}
        />
      </MemoryRouter>,
    )

    expect(await screen.findByText('"sqlite wal"')).toBeVisible()
    await waitFor(() =>
      expect(getSearchQueries).toHaveBeenLastCalledWith(
        { start: '', end: '' },
        expect.anything(),
      ),
    )

    fireEvent.change(screen.getByLabelText('searchQueriesPageNumberLabel'), {
      target: { value: '2.8' },
    })
    fireEvent.click(
      screen.getByRole('button', { name: 'searchQueriesJumpToPage' }),
    )
    await waitFor(() =>
      expect(getSearchQueries).toHaveBeenLastCalledWith(
        expect.anything(),
        expect.objectContaining({
          pagination: { page: 1, pageSize: 20 },
        }),
      ),
    )
  })

  test('clamps page jumps from defensive persisted input values', () => {
    expect(clampSearchKeywordPage('not-a-page', 4)).toBe(0)
    expect(clampSearchKeywordPage('2.8', 4)).toBe(1)
    expect(clampSearchKeywordPage('-12', 4)).toBe(0)
    expect(clampSearchKeywordPage('99', 4)).toBe(3)
    expect(clampSearchKeywordPage('2', 0)).toBe(0)
  })
})

function renderBrowser() {
  return render(
    <MemoryRouter>
      <SearchKeywordsBrowser
        dateRange={parentRange}
        engineOptions={[
          { searchEngine: 'google', displayName: 'Google' },
          { searchEngine: 'github', displayName: 'GitHub' },
        ]}
        help="Searches from the active scope"
        language="en"
        profileId="chrome:Default"
        queryFamilyHref={queryFamilyHref}
        t={t}
        title="Search Keywords"
        trailHref={trailHref}
      />
    </MemoryRouter>,
  )
}

function queryFamilyHref(familyId: string, profileId?: string | null) {
  return `/query-family/${familyId}${
    profileId ? `?profile=${encodeURIComponent(profileId)}` : ''
  }`
}

function trailHref(trailId: string, profileId?: string | null) {
  return `/trail/${trailId}${
    profileId ? `?profile=${encodeURIComponent(profileId)}` : ''
  }`
}

function formattedSearchTime(
  date: string,
  locale: 'en-US' | 'zh-CN' | 'zh-TW',
) {
  return new Date(date).toLocaleString(locale, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  })
}

async function flushMicrotasks() {
  await Promise.resolve()
  await Promise.resolve()
  await Promise.resolve()
}

function searchQueriesResult(
  rows: SearchQueryRow[],
  overrides: {
    dateRange?: DateRange
    page?: number
    pageSize?: number
    total?: number
  } = {},
): CoreIntelligenceSectionResult<SearchQueryListResult> {
  const dateRange = overrides.dateRange ?? parentRange
  const pageSize = overrides.pageSize ?? 20
  return {
    data: {
      rows,
      total: overrides.total ?? rows.length,
      page: overrides.page ?? 0,
      pageSize,
    },
    meta: {
      sectionId: 'search-activity',
      generatedAt: '2026-04-25T10:00:00Z',
      window: { kind: 'date-range', dateRange },
      moduleIds: ['search-trails'],
      sourceTables: ['search_events'],
      includesEnrichment: false,
      state: 'ready',
      stateReason: null,
      notes: [],
    },
  }
}

function rowsFixture(): SearchQueryRow[] {
  return [
    {
      visitId: 1,
      profileId: 'chrome:Default',
      browserKind: 'chromium',
      searchEngine: 'google',
      displayName: 'Google',
      rawQuery: 'sqlite wal',
      normalizedQuery: 'sqlite wal',
      searchedAt: '2026-04-20T10:00:00Z',
      searchedAtMs: Date.parse('2026-04-20T10:00:00Z'),
      exactRepeatCount: 3,
      familyCount: 5,
      familyId: 'family-1',
      trailId: 'trail-1',
      trailInitialQuery: 'sqlite',
      trailReformulationCount: 2,
    },
    {
      visitId: 2,
      profileId: 'chrome:Default',
      browserKind: 'chromium',
      searchEngine: 'github',
      displayName: 'GitHub',
      rawQuery: 'react router',
      normalizedQuery: 'react router',
      searchedAt: '2026-04-21T12:00:00Z',
      searchedAtMs: Date.parse('2026-04-21T12:00:00Z'),
      exactRepeatCount: 1,
      familyCount: 1,
      familyId: null,
      trailId: null,
      trailInitialQuery: null,
      trailReformulationCount: null,
    },
  ]
}

function fallbackRowsFixture(): SearchQueryRow[] {
  return [
    {
      visitId: 3,
      profileId: '',
      browserKind: 'chromium',
      searchEngine: 'plain',
      displayName: null,
      rawQuery: 'fallback display',
      normalizedQuery: 'fallback display',
      searchedAt: '2026-04-22T10:00:00Z',
      searchedAtMs: Date.parse('2026-04-22T10:00:00Z'),
      exactRepeatCount: 2,
      familyCount: 4,
      familyId: 'family-fallback',
      trailId: 'trail-fallback',
      trailInitialQuery: 'duckdb',
      trailReformulationCount: null,
    },
    {
      visitId: 4,
      profileId: '',
      browserKind: 'chromium',
      searchEngine: 'plain',
      displayName: 'Plain Search',
      rawQuery: 'duplicate engine',
      normalizedQuery: 'duplicate engine',
      searchedAt: '2026-04-23T10:00:00Z',
      searchedAtMs: Date.parse('2026-04-23T10:00:00Z'),
      exactRepeatCount: 1,
      familyCount: 1,
      familyId: null,
      trailId: null,
      trailInitialQuery: null,
      trailReformulationCount: null,
    },
  ]
}
