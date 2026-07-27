/**
 * @file search-pagination.behavior.test.tsx
 * @description Behavioural contract for keyword/regex search-result pagination.
 * @module pages/explorer
 *
 * ## Responsibilities
 * - Drive the SHIPPED Explorer route (real `useExplorerUrlState`, real
 *   `useExplorerData`, real `MemoryRouter`) through the search pager and assert
 *   both the URL and the rendered page change together.
 * - Pin the `docs/features/recall.md` contract: a pagination bar above AND
 *   below the results, each with first / previous / next / last, a page-jump
 *   box, a rows-per-page selector, and a "page N of M" readout.
 * - Pin rows-per-page persistence across a remount (Explorer preference).
 *
 * ## Not responsible for
 * - Smart (relevance) cursor pagination — that pager has its own tests in
 *   `paper-search-view.test.tsx` / `paper-search-panel.test.tsx`.
 * - Pagination bar styling details.
 *
 * ## Dependencies
 * - The shared Intelligence-surface harness, which is the only route harness
 *   that mounts Explorer with its real URL-state hook. Mocking that hook is
 *   exactly how the "search only ever renders page 1" defect shipped, so this
 *   file deliberately does not.
 *
 * ## Performance notes
 * - One history row per page fixture; no IPC, no full archive scan.
 */

import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useLocation } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import { explorerPageSizeStorageKey } from './helpers'
import { createNamespaceTranslator } from '../../lib/i18n'
import type { HistoryQueryResponse } from '../../lib/types'
import { ExplorerPage } from './index'
import {
  renderSurface,
  resetIntelligenceSurfaceHarness,
  seedArchiveState,
} from '../intelligence-surfaces/test-helpers'

const TOTAL_HITS = 12431

const explorerT = createNamespaceTranslator('en', 'explorer')

/**
 * Renders the live router location so a test can assert the URL contract the
 * pager writes, not just the callbacks it fires.
 */
function LocationProbe() {
  const location = useLocation()
  return <span data-testid="location-search">{location.search}</span>
}

function pageFixture(
  page: number,
  pageSize: number,
  total = TOTAL_HITS,
): HistoryQueryResponse {
  const pageCount = Math.max(1, Math.ceil(total / pageSize))
  return {
    total,
    page,
    pageSize,
    pageCount,
    hasPrevious: page > 1,
    hasNext: page < pageCount,
    nextCursor: null,
    items: [
      {
        id: page * 1000,
        profileId: 'chrome:Default',
        url: `https://example.com/p/${page}`,
        title: `Archived page ${page}`,
        domain: 'example.com',
        visitedAt: '2026-04-17T10:00:00Z',
        visitTime: Date.parse('2026-04-17T10:00:00Z'),
        transition: null,
        favicon: null,
        sourceVisitId: page * 1000,
      },
    ],
  }
}

function emptyFixture(): HistoryQueryResponse {
  return {
    total: 0,
    page: 1,
    pageSize: 50,
    pageCount: 0,
    hasPrevious: false,
    hasNext: false,
    nextCursor: null,
    items: [],
  }
}

/**
 * The preview backend has no star transport, so without these the surface
 * spends the test showing a star-hydration error callout and settles state
 * after the test ends. Stars are not what this file is about.
 */
function mockQuietStars() {
  vi.spyOn(backend, 'getStarStatus').mockResolvedValue({})
  vi.spyOn(backend, 'getStarCounts').mockResolvedValue({ urls: 0, domains: 0 })
  vi.spyOn(backend, 'listStars').mockResolvedValue([])
}

function mockPagedHistory(total = TOTAL_HITS) {
  return vi
    .spyOn(backend, 'queryHistory')
    .mockImplementation((query) =>
      Promise.resolve(pageFixture(query.page ?? 1, query.limit ?? 50, total)),
    )
}

function topBar() {
  return within(screen.getByTestId('paper-search-pagination-top'))
}

function bottomBar() {
  return within(screen.getByTestId('paper-search-pagination-bottom'))
}

function locationSearch() {
  return screen.getByTestId('location-search').textContent ?? ''
}

async function renderSearchSurface(route = '/search?q=rust') {
  const { snapshot } = await seedArchiveState()
  const result = renderSurface(
    <>
      <ExplorerPage />
      <LocationProbe />
    </>,
    { language: 'en', route, snapshot },
  )
  return result
}

describe('Explorer keyword search pagination', () => {
  beforeEach(() => {
    resetIntelligenceSurfaceHarness()
    mockQuietStars()
  })

  test('mounts a pager above and below the results with an honest page readout', async () => {
    mockPagedHistory()
    await renderSearchSurface()

    expect(await screen.findByText('Archived page 1')).toBeVisible()

    // recall.md: both bars, both showing "current page / total pages".
    const expectedSummary = explorerT('pageCountSummary')
      .replace('{current}', '1')
      .replace('{total}', '249')
    expect(
      topBar().getByTestId('paper-search-pagination-top-summary'),
    ).toHaveTextContent(expectedSummary)
    expect(
      bottomBar().getByTestId('paper-search-pagination-bottom-summary'),
    ).toHaveTextContent(expectedSummary)

    // Every affordance the doc requires is present in each bar.
    for (const bar of [topBar(), bottomBar()]) {
      expect(bar.getByRole('button', { name: /First page/ })).toBeVisible()
      expect(bar.getByRole('button', { name: /Previous page/ })).toBeVisible()
      expect(bar.getByRole('button', { name: /Next page/ })).toBeVisible()
      expect(bar.getByRole('button', { name: /Last page/ })).toBeVisible()
      expect(bar.getByLabelText(explorerT('pageNumberLabel'))).toBeVisible()
      expect(
        bar.getByRole('option', {
          name: explorerT('pageSizeOption').replace('{count}', '200'),
        }),
      ).toBeInTheDocument()
    }

    // Page 1: backwards controls are dead ends, forwards ones are live.
    expect(topBar().getByRole('button', { name: /First page/ })).toBeDisabled()
    expect(
      topBar().getByRole('button', { name: /Previous page/ }),
    ).toBeDisabled()
    expect(topBar().getByRole('button', { name: /Next page/ })).toBeEnabled()
    expect(topBar().getByRole('button', { name: /Last page/ })).toBeEnabled()
  })

  test('next / previous / last / first move both the URL page and the rendered results', async () => {
    const user = userEvent.setup()
    const querySpy = mockPagedHistory()
    await renderSearchSurface()

    expect(await screen.findByText('Archived page 1')).toBeVisible()
    expect(locationSearch()).not.toContain('page=')

    await user.click(topBar().getByRole('button', { name: /Next page/ }))

    await waitFor(() => expect(locationSearch()).toContain('page=2'))
    expect(await screen.findByText('Archived page 2')).toBeVisible()
    expect(screen.queryByText('Archived page 1')).not.toBeInTheDocument()
    expect(querySpy).toHaveBeenCalledWith(
      expect.objectContaining({ page: 2, limit: 50 }),
    )
    expect(
      topBar().getByTestId('paper-search-pagination-top-summary'),
    ).toHaveTextContent(
      explorerT('pageCountSummary')
        .replace('{current}', '2')
        .replace('{total}', '249'),
    )
    // The jump box follows the page the user is actually on.
    expect(
      bottomBar().getByLabelText(explorerT('pageNumberLabel')),
    ).toHaveValue('2')

    await user.click(bottomBar().getByRole('button', { name: /Last page/ }))
    await waitFor(() => expect(locationSearch()).toContain('page=249'))
    expect(await screen.findByText('Archived page 249')).toBeVisible()
    // End of the set: forwards controls are now the dead ends.
    expect(
      bottomBar().getByRole('button', { name: /Next page/ }),
    ).toBeDisabled()
    expect(
      bottomBar().getByRole('button', { name: /Last page/ }),
    ).toBeDisabled()

    await user.click(bottomBar().getByRole('button', { name: /Previous page/ }))
    await waitFor(() => expect(locationSearch()).toContain('page=248'))
    expect(await screen.findByText('Archived page 248')).toBeVisible()

    await user.click(topBar().getByRole('button', { name: /First page/ }))
    await waitFor(() => expect(locationSearch()).not.toContain('page='))
    expect(await screen.findByText('Archived page 1')).toBeVisible()
  })

  test('the page-jump box navigates and clamps to the last page', async () => {
    const user = userEvent.setup()
    mockPagedHistory()
    await renderSearchSurface()

    expect(await screen.findByText('Archived page 1')).toBeVisible()

    const jumpInput = bottomBar().getByLabelText(explorerT('pageNumberLabel'))
    await user.clear(jumpInput)
    await user.type(jumpInput, '7')
    await user.click(
      bottomBar().getByRole('button', { name: explorerT('jumpToPage') }),
    )

    await waitFor(() => expect(locationSearch()).toContain('page=7'))
    expect(await screen.findByText('Archived page 7')).toBeVisible()

    // Out-of-range input is clamped into the set instead of stranding the user.
    const topJump = topBar().getByLabelText(explorerT('pageNumberLabel'))
    await user.clear(topJump)
    await user.type(topJump, '9999')
    await user.click(
      topBar().getByRole('button', { name: explorerT('jumpToPage') }),
    )

    await waitFor(() => expect(locationSearch()).toContain('page=249'))
    expect(await screen.findByText('Archived page 249')).toBeVisible()

    // Non-numeric input is rejected without navigating away.
    const resetJump = topBar().getByLabelText(explorerT('pageNumberLabel'))
    await user.clear(resetJump)
    await user.type(resetJump, 'abc')
    await user.click(
      topBar().getByRole('button', { name: explorerT('jumpToPage') }),
    )

    await waitFor(() => expect(resetJump).toHaveValue('249'))
    expect(locationSearch()).toContain('page=249')
  })

  test('rows-per-page requeries, resets to page 1, and survives a remount', async () => {
    const user = userEvent.setup()
    const querySpy = mockPagedHistory()
    const first = await renderSearchSurface()

    expect(await screen.findByText('Archived page 1')).toBeVisible()
    await user.click(topBar().getByRole('button', { name: /Next page/ }))
    await waitFor(() => expect(locationSearch()).toContain('page=2'))

    await user.selectOptions(
      topBar().getByTestId('paper-search-pagination-top-page-size'),
      '200',
    )

    await waitFor(() =>
      expect(querySpy).toHaveBeenCalledWith(
        expect.objectContaining({ limit: 200 }),
      ),
    )
    // Changing the page size must not leave the user on a page that no longer
    // exists at the new size.
    await waitFor(() => expect(locationSearch()).not.toContain('page='))
    expect(locationSearch()).toContain('pageSize=200')
    expect(window.localStorage.getItem(explorerPageSizeStorageKey)).toBe('200')

    first.unmount()
    querySpy.mockClear()

    // recall.md: the preference outlives the page — a fresh mount with no
    // `pageSize` in the URL still queries 200 rows.
    await renderSearchSurface()
    await waitFor(() =>
      expect(querySpy).toHaveBeenCalledWith(
        expect.objectContaining({ limit: 200 }),
      ),
    )
    expect(
      await screen.findByTestId('paper-search-pagination-top-page-size'),
    ).toHaveValue('200')
  })

  test('regex mode gets the same pager as keyword mode', async () => {
    const user = userEvent.setup()
    mockPagedHistory()
    await renderSearchSurface('/search?q=ru.%2Ast&regex=1')

    expect(await screen.findByText('Archived page 1')).toBeVisible()
    expect(screen.getByTestId('paper-search-pagination-top')).toBeVisible()

    await user.click(bottomBar().getByRole('button', { name: /Next page/ }))
    await waitFor(() => expect(locationSearch()).toContain('page=2'))
    expect(await screen.findByText('Archived page 2')).toBeVisible()
  })

  test('a search with no matches renders no pager at all', async () => {
    vi.spyOn(backend, 'queryHistory').mockResolvedValue(emptyFixture())
    await renderSearchSurface()

    expect(await screen.findByTestId('paper-search-no-matches')).toBeVisible()
    expect(
      screen.queryByTestId('paper-search-pagination-top'),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByTestId('paper-search-pagination-bottom'),
    ).not.toBeInTheDocument()
  })
})
