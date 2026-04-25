/**
 * @file explorer-controls.test.tsx
 * @description Protects Explorer filter controls, shared profile scope sync, debounced query commits, and pagination controls.
 * @module pages/intelligence-surfaces
 *
 * ## Responsibilities
 * - Preserve the one-click date-range clearing contract.
 * - Preserve shared profile-scope reflection inside Explorer controls.
 * - Preserve debounced keyword-query commit behavior.
 * - Preserve pagination summary and page-size control behavior.
 *
 * ## Non-Responsibilities
 * - Does not own grouped Explorer view assertions.
 * - Does not own route-first Intelligence entity destination tests.
 * - Does not broaden the shared harness or mutate unrelated route surfaces.
 *
 * ## Dependencies
 * - Depends on the shared Intelligence surface harness in `test-helpers.tsx`.
 * - Uses the shipped Explorer page as the integration surface.
 * - Mocks `backend.queryHistory` only where the original suite already does so.
 *
 * ## Performance Notes
 * - Reuses the shared seeded archive fixture to keep test setup bounded.
 * - Verifies debounce and pagination through existing network-boundary spies instead of extra render loops.
 */

import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import { createNamespaceTranslator } from '../../lib/i18n'
import { ExplorerPage } from '../explorer'
import {
  renderSurface,
  resetIntelligenceSurfaceHarness,
  seedArchiveState,
} from './test-helpers'

describe('intelligence surfaces', () => {
  beforeEach(() => {
    resetIntelligenceSurfaceHarness()
    window.localStorage.removeItem('pathkeep.explorer.page-size')
  })

  test('clears both explorer date bounds in a single interaction', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const explorerT = createNamespaceTranslator('en', 'explorer')

    renderSurface(<ExplorerPage />, {
      language: 'en',
      route: '/explorer?start=2026-04-01&end=2026-04-07',
      snapshot,
    })

    const startInput = await screen.findByLabelText(explorerT('filterStart'))
    const endInput = await screen.findByLabelText(explorerT('filterEnd'))

    expect(startInput).toHaveValue('2026-04-01')
    expect(endInput).toHaveValue('2026-04-07')
    expect(
      screen.getByRole('button', {
        name: explorerT('removeFilter', {
          label: explorerT('filterStart'),
          value: '2026-04-01',
        }),
      }),
    ).toBeVisible()

    await user.click(screen.getByRole('button', { name: 'Clear range' }))

    await waitFor(() => {
      expect(startInput).toHaveValue('')
      expect(endInput).toHaveValue('')
    })
  })

  test('keeps the explorer profile control aligned with the shared profile scope', async () => {
    window.localStorage.setItem('pathkeep.profile-scope', 'chrome:Default')

    try {
      const { snapshot } = await seedArchiveState()
      const explorerT = createNamespaceTranslator('en', 'explorer')

      renderSurface(<ExplorerPage />, {
        language: 'en',
        route: '/explorer',
        snapshot,
      })

      expect(
        await screen.findByLabelText(explorerT('filterProfileAria')),
      ).toHaveValue('chrome:Default')
      expect(await screen.findByText(explorerT('scopeInherited'))).toBeVisible()
    } finally {
      window.localStorage.removeItem('pathkeep.profile-scope')
    }
  })

  test('debounces explorer keyword query commits while the user is typing', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const explorerT = createNamespaceTranslator('en', 'explorer')
    const querySpy = vi.spyOn(backend, 'queryHistory')

    renderSurface(<ExplorerPage />, {
      language: 'en',
      route: '/explorer',
      snapshot,
    })

    expect(await screen.findByTestId('explorer-page')).toBeInTheDocument()
    await waitFor(() =>
      expect(querySpy).toHaveBeenCalledWith(
        expect.objectContaining({ q: null, limit: 50 }),
      ),
    )
    const baselineCallCount = querySpy.mock.calls.length

    await user.type(
      screen.getByLabelText(explorerT('filterKeywordAria')),
      'sqlite',
    )

    expect(querySpy.mock.calls.length).toBe(baselineCallCount)

    await new Promise((resolve) => window.setTimeout(resolve, 220))
    await waitFor(() =>
      expect(querySpy).toHaveBeenCalledWith(
        expect.objectContaining({ q: 'sqlite', limit: 50 }),
      ),
    )
  })

  test('renders explorer chrome and a results skeleton before the first query settles', async () => {
    const { snapshot } = await seedArchiveState()
    const explorerT = createNamespaceTranslator('en', 'explorer')
    const commonT = createNamespaceTranslator('en', 'common')

    let resolveInitialQuery:
      | ((value: Awaited<ReturnType<typeof backend.queryHistory>>) => void)
      | undefined

    vi.spyOn(backend, 'queryHistory').mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveInitialQuery = resolve
        }),
    )

    renderSurface(<ExplorerPage />, {
      language: 'en',
      route: '/explorer',
      snapshot,
    })

    expect(await screen.findByTestId('explorer-page')).toBeInTheDocument()
    expect(screen.getByLabelText(explorerT('filterKeywordAria'))).toBeVisible()
    expect(
      await screen.findByTestId('explorer-results-skeleton'),
    ).toHaveAttribute('aria-label', commonT('loadingExplorerResults'))

    await waitFor(() => expect(resolveInitialQuery).toBeDefined())
    resolveInitialQuery?.({
      total: 1,
      page: 1,
      pageSize: 50,
      pageCount: 1,
      hasPrevious: false,
      hasNext: false,
      nextCursor: null,
      items: [
        {
          id: 1,
          profileId: 'chrome:Default',
          url: 'https://example.com/alpha',
          title: 'Alpha',
          domain: 'example.com',
          visitedAt: '2026-04-17T10:00:00Z',
          visitTime: Date.parse('2026-04-17T10:00:00Z'),
          transition: null,
          favicon: null,
          sourceVisitId: 1,
        },
      ],
    })

    expect((await screen.findAllByText('Alpha')).length).toBeGreaterThan(0)
  })

  test('renders history rows before lazy favicon payloads resolve', async () => {
    const { snapshot } = await seedArchiveState()

    let resolveFavicons:
      | ((
          value: Awaited<ReturnType<typeof backend.loadHistoryFavicons>>,
        ) => void)
      | undefined

    vi.spyOn(backend, 'queryHistory').mockResolvedValue({
      total: 1,
      page: 1,
      pageSize: 50,
      pageCount: 1,
      hasPrevious: false,
      hasNext: false,
      nextCursor: null,
      items: [
        {
          id: 1,
          profileId: 'chrome:Default',
          url: 'https://example.com/alpha',
          title: 'Alpha',
          domain: 'example.com',
          visitedAt: '2026-04-17T10:00:00Z',
          visitTime: Date.parse('2026-04-17T10:00:00Z'),
          transition: null,
          favicon: null,
          sourceVisitId: 1,
        },
      ],
    })
    const faviconSpy = vi
      .spyOn(backend, 'loadHistoryFavicons')
      .mockImplementation(
        () =>
          new Promise((resolve) => {
            resolveFavicons = resolve
          }),
      )

    renderSurface(<ExplorerPage />, {
      language: 'en',
      route: '/explorer',
      snapshot,
    })

    expect((await screen.findAllByText('Alpha')).length).toBeGreaterThan(0)
    await waitFor(() =>
      expect(faviconSpy).toHaveBeenCalledWith([
        {
          profileId: 'chrome:Default',
          url: 'https://example.com/alpha',
          visitTime: Date.parse('2026-04-17T10:00:00Z'),
        },
      ]),
    )
    expect(document.querySelector('.favicon-image')).toBeNull()

    expect(resolveFavicons).toBeDefined()
    if (!resolveFavicons) {
      throw new Error('expected favicon resolver to be captured')
    }
    resolveFavicons([
      {
        profileId: 'chrome:Default',
        url: 'https://example.com/alpha',
        visitTime: Date.parse('2026-04-17T10:00:00Z'),
        favicon: {
          dataUrl: 'data:image/png;base64,AQI=',
        },
      },
    ])

    await waitFor(() =>
      expect(document.querySelector('.favicon-image')).toHaveAttribute(
        'src',
        'data:image/png;base64,AQI=',
      ),
    )
  })

  test('shows the current page count and lets users change explorer rows per page', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const explorerT = createNamespaceTranslator('en', 'explorer')
    const scrollToSpy = vi.fn()
    window.scrollTo = scrollToSpy
    const querySpy = vi
      .spyOn(backend, 'queryHistory')
      .mockImplementation((query) =>
        Promise.resolve({
          total: 240,
          page: query.page ?? 1,
          pageSize: query.limit ?? 50,
          pageCount: Math.ceil(240 / (query.limit ?? 50)),
          hasPrevious: (query.page ?? 1) > 1,
          hasNext: (query.page ?? 1) < Math.ceil(240 / (query.limit ?? 50)),
          nextCursor: null,
          items: [
            {
              id: 1,
              profileId: 'chrome:Default',
              url: 'https://example.com/alpha',
              title: 'Alpha',
              domain: 'example.com',
              visitedAt: '2026-04-17T10:00:00Z',
              visitTime: Date.parse('2026-04-17T10:00:00Z'),
              transition: null,
              favicon: null,
              sourceVisitId: 1,
            },
            {
              id: 2,
              profileId: 'chrome:Default',
              url: 'https://example.com/beta',
              title: 'Beta',
              domain: 'example.com',
              visitedAt: '2026-04-17T11:00:00Z',
              visitTime: Date.parse('2026-04-17T11:00:00Z'),
              transition: null,
              favicon: null,
              sourceVisitId: 2,
            },
          ],
        }),
      )

    renderSurface(<ExplorerPage />, {
      language: 'en',
      route: '/explorer',
      snapshot,
    })

    expect((await screen.findAllByText('Page 1 of 5')).length).toBeGreaterThan(
      2,
    )
    expect(
      screen.getAllByRole('button', { name: explorerT('firstPage') }).length,
    ).toBe(2)
    expect(
      screen.getAllByText('Showing 2 of 240 results on this page').length,
    ).toBeGreaterThan(2)
    await waitFor(() =>
      expect(querySpy).toHaveBeenCalledWith(
        expect.objectContaining({ limit: 50 }),
      ),
    )

    await user.click(
      screen.getAllByRole('button', { name: explorerT('nextPage') })[0],
    )

    await waitFor(() =>
      expect(querySpy).toHaveBeenCalledWith(
        expect.objectContaining({ page: 2, limit: 50 }),
      ),
    )
    expect((await screen.findAllByText('Page 2 of 5')).length).toBeGreaterThan(
      2,
    )
    expect(scrollToSpy).not.toHaveBeenCalled()

    const pageSizeSelect = screen.getAllByRole('combobox', {
      name: explorerT('pageSizeLabel'),
    })[0]
    await waitFor(() => expect(pageSizeSelect).not.toBeDisabled())

    await user.selectOptions(pageSizeSelect, '100')

    await waitFor(() =>
      expect(querySpy).toHaveBeenCalledWith(
        expect.objectContaining({ limit: 100 }),
      ),
    )
    expect((await screen.findAllByText('Page 1 of 3')).length).toBeGreaterThan(
      2,
    )
    expect(scrollToSpy).not.toHaveBeenCalled()
  })

  test('persists the explorer page size across route remounts', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const explorerT = createNamespaceTranslator('en', 'explorer')
    const querySpy = vi
      .spyOn(backend, 'queryHistory')
      .mockImplementation((query) =>
        Promise.resolve({
          total: 400,
          page: query.page ?? 1,
          pageSize: query.limit ?? 50,
          pageCount: Math.ceil(400 / (query.limit ?? 50)),
          hasPrevious: (query.page ?? 1) > 1,
          hasNext: (query.page ?? 1) < Math.ceil(400 / (query.limit ?? 50)),
          nextCursor: null,
          items: [
            {
              id: 1,
              profileId: 'chrome:Default',
              url: 'https://example.com/alpha',
              title: 'Alpha',
              domain: 'example.com',
              visitedAt: '2026-04-17T10:00:00Z',
              visitTime: Date.parse('2026-04-17T10:00:00Z'),
              transition: null,
              favicon: null,
              sourceVisitId: 1,
            },
          ],
        }),
      )

    const firstRender = renderSurface(<ExplorerPage />, {
      language: 'en',
      route: '/explorer',
      snapshot,
    })

    const firstPageSizeSelect = (
      await screen.findAllByRole('combobox', {
        name: explorerT('pageSizeLabel'),
      })
    )[0]
    await waitFor(() => expect(firstPageSizeSelect).not.toBeDisabled())
    await user.selectOptions(firstPageSizeSelect, '200')

    await waitFor(() =>
      expect(querySpy).toHaveBeenCalledWith(
        expect.objectContaining({ limit: 200 }),
      ),
    )
    firstRender.unmount()

    querySpy.mockClear()

    renderSurface(<ExplorerPage />, {
      language: 'en',
      route: '/explorer',
      snapshot,
    })

    await waitFor(() =>
      expect(querySpy).toHaveBeenCalledWith(
        expect.objectContaining({ limit: 200 }),
      ),
    )
    expect(
      (
        await screen.findAllByRole('combobox', {
          name: explorerT('pageSizeLabel'),
        })
      )[0],
    ).toHaveValue('200')
  })

  test('prefetches as many adjacent pages as the current config allows', async () => {
    const { snapshot } = await seedArchiveState()
    snapshot.config.explorerBackgroundPrefetchPages = 2
    const querySpy = vi
      .spyOn(backend, 'queryHistory')
      .mockImplementation((query) =>
        Promise.resolve({
          total: 250,
          page: query.page ?? 1,
          pageSize: query.limit ?? 50,
          pageCount: 5,
          hasPrevious: (query.page ?? 1) > 1,
          hasNext: (query.page ?? 1) < 5,
          nextCursor: null,
          items: [
            {
              id: (query.page ?? 1) * 10,
              profileId: 'chrome:Default',
              url: `https://example.com/page-${query.page ?? 1}`,
              title: `Page ${(query.page ?? 1).toString()}`,
              domain: 'example.com',
              visitedAt: '2026-04-17T10:00:00Z',
              visitTime: Date.parse('2026-04-17T10:00:00Z'),
              transition: null,
              favicon: null,
              sourceVisitId: (query.page ?? 1) * 10,
            },
          ],
        }),
      )

    renderSurface(<ExplorerPage />, {
      language: 'en',
      route: '/explorer',
      snapshot,
    })

    expect((await screen.findAllByText('Page 1')).length).toBeGreaterThan(0)
    await waitFor(() =>
      expect(querySpy).toHaveBeenCalledWith(
        expect.objectContaining({ page: 2, limit: 50 }),
      ),
    )
    await waitFor(() =>
      expect(querySpy).toHaveBeenCalledWith(
        expect.objectContaining({ page: 3, limit: 50 }),
      ),
    )
  })

  test('reuses the prefetched adjacent page when users move one page forward', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    snapshot.config.explorerBackgroundPrefetchPages = 1
    const explorerT = createNamespaceTranslator('en', 'explorer')
    const querySpy = vi
      .spyOn(backend, 'queryHistory')
      .mockImplementation((query) =>
        Promise.resolve({
          total: 120,
          page: query.page ?? 1,
          pageSize: query.limit ?? 50,
          pageCount: 3,
          hasPrevious: (query.page ?? 1) > 1,
          hasNext: (query.page ?? 1) < 3,
          nextCursor: null,
          items: [
            {
              id: (query.page ?? 1) * 10,
              profileId: 'chrome:Default',
              url: `https://example.com/page-${query.page ?? 1}`,
              title: `Page ${(query.page ?? 1).toString()}`,
              domain: 'example.com',
              visitedAt: '2026-04-17T10:00:00Z',
              visitTime: Date.parse('2026-04-17T10:00:00Z'),
              transition: null,
              favicon: null,
              sourceVisitId: (query.page ?? 1) * 10,
            },
          ],
        }),
      )

    renderSurface(<ExplorerPage />, {
      language: 'en',
      route: '/explorer',
      snapshot,
    })

    expect((await screen.findAllByText('Page 1')).length).toBeGreaterThan(0)
    await waitFor(() =>
      expect(querySpy).toHaveBeenCalledWith(
        expect.objectContaining({ page: 2, limit: 50 }),
      ),
    )
    const callCountBeforeNavigation = querySpy.mock.calls.length

    await user.click(
      screen.getAllByRole('button', { name: explorerT('nextPage') })[0],
    )

    expect((await screen.findAllByText('Page 2')).length).toBeGreaterThan(0)
    const followupCalls = querySpy.mock.calls.slice(callCountBeforeNavigation)
    expect(followupCalls).toHaveLength(1)
    expect(followupCalls[0]?.[0]).toEqual(
      expect.objectContaining({ page: 3, limit: 50 }),
    )
    expect(
      screen.queryByTestId('explorer-results-skeleton'),
    ).not.toBeInTheDocument()
  })

  test('keeps explorer controls visible while page navigation stages a results skeleton', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const explorerT = createNamespaceTranslator('en', 'explorer')
    const commonT = createNamespaceTranslator('en', 'common')

    let resolvePageTwo:
      | ((value: Awaited<ReturnType<typeof backend.queryHistory>>) => void)
      | undefined

    const pageOneResult = {
      total: 120,
      page: 1,
      pageSize: 50,
      pageCount: 3,
      hasPrevious: false,
      hasNext: true,
      nextCursor: null,
      items: [
        {
          id: 1,
          profileId: 'chrome:Default',
          url: 'https://example.com/alpha',
          title: 'Alpha',
          domain: 'example.com',
          visitedAt: '2026-04-17T10:00:00Z',
          visitTime: Date.parse('2026-04-17T10:00:00Z'),
          transition: null,
          favicon: null,
          sourceVisitId: 1,
        },
      ],
    }
    const pageTwoResult = {
      ...pageOneResult,
      page: 2,
      hasPrevious: true,
      hasNext: true,
      items: [
        {
          id: 2,
          profileId: 'chrome:Default',
          url: 'https://example.com/bravo',
          title: 'Bravo',
          domain: 'example.com',
          visitedAt: '2026-04-17T11:00:00Z',
          visitTime: Date.parse('2026-04-17T11:00:00Z'),
          transition: null,
          favicon: null,
          sourceVisitId: 2,
        },
      ],
    }

    const querySpy = vi
      .spyOn(backend, 'queryHistory')
      .mockImplementation((query) => {
        if ((query.page ?? 1) === 2) {
          return new Promise((resolve) => {
            resolvePageTwo = resolve
          })
        }

        return Promise.resolve(pageOneResult)
      })

    renderSurface(<ExplorerPage />, {
      language: 'en',
      route: '/explorer',
      snapshot,
    })

    expect((await screen.findAllByText('Page 1 of 3')).length).toBeGreaterThan(
      0,
    )

    await user.click(
      screen.getAllByRole('button', { name: explorerT('nextPage') })[0],
    )

    await waitFor(() =>
      expect(querySpy).toHaveBeenCalledWith(
        expect.objectContaining({ page: 2, limit: 50 }),
      ),
    )
    expect(screen.getByLabelText(explorerT('filterKeywordAria'))).toBeVisible()
    expect(await screen.findAllByText('Page 2 of 3')).not.toHaveLength(0)
    expect(screen.getByTestId('explorer-results-skeleton')).toHaveAttribute(
      'aria-label',
      commonT('loadingExplorerResults'),
    )
    expect(
      screen.getAllByRole('combobox', { name: explorerT('pageSizeLabel') })[0],
    ).toBeDisabled()

    expect(resolvePageTwo).toBeDefined()
    if (!resolvePageTwo) {
      throw new Error('expected page-two resolver to be captured')
    }
    resolvePageTwo(pageTwoResult)

    expect((await screen.findAllByText('Page 2 of 3')).length).toBeGreaterThan(
      0,
    )
  })
})
