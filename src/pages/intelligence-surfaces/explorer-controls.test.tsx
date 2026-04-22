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
    await waitFor(() => expect(querySpy).toHaveBeenCalledTimes(1))

    await user.type(
      screen.getByLabelText(explorerT('filterKeywordAria')),
      'sqlite',
    )

    expect(querySpy).toHaveBeenCalledTimes(1)

    await new Promise((resolve) => window.setTimeout(resolve, 220))
    await waitFor(() => expect(querySpy).toHaveBeenCalledTimes(2))
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
      screen.getAllByText('Showing 2 of 240 results on this page').length,
    ).toBeGreaterThan(2)
    await waitFor(() =>
      expect(querySpy).toHaveBeenLastCalledWith(
        expect.objectContaining({ limit: 50 }),
      ),
    )

    await user.click(
      screen.getByRole('button', { name: explorerT('nextPage') }),
    )

    await waitFor(() =>
      expect(querySpy).toHaveBeenLastCalledWith(
        expect.objectContaining({ page: 2, limit: 50 }),
      ),
    )
    expect((await screen.findAllByText('Page 2 of 5')).length).toBeGreaterThan(
      2,
    )
    expect(scrollToSpy).not.toHaveBeenCalled()

    await user.selectOptions(
      screen.getByRole('combobox', { name: explorerT('pageSizeLabel') }),
      '100',
    )

    await waitFor(() =>
      expect(querySpy).toHaveBeenLastCalledWith(
        expect.objectContaining({ limit: 100 }),
      ),
    )
    expect((await screen.findAllByText('Page 1 of 3')).length).toBeGreaterThan(
      2,
    )
    expect(scrollToSpy).not.toHaveBeenCalled()
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
      screen.getByRole('button', { name: explorerT('nextPage') }),
    )

    await waitFor(() =>
      expect(querySpy).toHaveBeenLastCalledWith(
        expect.objectContaining({ page: 2, limit: 50 }),
      ),
    )
    expect(screen.getByLabelText(explorerT('filterKeywordAria'))).toBeVisible()
    expect(screen.getByTestId('explorer-results-skeleton')).toHaveAttribute(
      'aria-label',
      commonT('loadingExplorerResults'),
    )

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
