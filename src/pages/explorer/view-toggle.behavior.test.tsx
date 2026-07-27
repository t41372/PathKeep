/**
 * @file view-toggle.behavior.test.tsx
 * @description Behavioural contract for the Browse view switch (Time / Session / Trail).
 * @module pages/explorer
 *
 * ## Responsibilities
 * - Prove the Session and Trail surfaces are REACHABLE from the UI. Both
 *   branches have always rendered, with detail rails, explainability panels and
 *   ~600 lines of their own tests — but `setView` had no caller anywhere in
 *   `src/`, so the only way in was hand-typing `?view=session` in the address
 *   bar. This file is the regression that keeps an entry point wired.
 * - Pin the round trip: entering a grouped view and coming back to Time.
 *
 * ## Not responsible for
 * - What the Session / Trail panels render internally — `panels/session-group`
 *   and `panels/trail-group` own that.
 *
 * ## Dependencies
 * - The shared Intelligence-surface harness: the only route harness that mounts
 *   Explorer with its REAL `useExplorerUrlState` and a real `MemoryRouter`.
 *   Asserting against a mocked URL hook would only prove the route called a
 *   spy, which is exactly how the un-reachable views shipped in the first
 *   place.
 *
 * ## Performance notes
 * - One history row; the grouped panels' backend reads resolve empty.
 */

import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useLocation } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import { createNamespaceTranslator } from '../../lib/i18n'
import type { HistoryQueryResponse } from '../../lib/types'
import { ExplorerPage } from './index'
import {
  renderSurface,
  resetIntelligenceSurfaceHarness,
  seedArchiveState,
} from '../intelligence-surfaces/test-helpers'

const explorerT = createNamespaceTranslator('en', 'explorer')

function LocationProbe() {
  const location = useLocation()
  return <span data-testid="location-search">{location.search}</span>
}

function locationSearch() {
  return screen.getByTestId('location-search').textContent ?? ''
}

function historyFixture(): HistoryQueryResponse {
  return {
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
        url: 'https://example.com/p/1',
        title: 'Archived page 1',
        domain: 'example.com',
        visitedAt: '2026-04-17T10:00:00Z',
        visitTime: Date.parse('2026-04-17T10:00:00Z'),
        transition: null,
        favicon: null,
        sourceVisitId: 1,
      },
    ],
  }
}

async function renderBrowse(route = '/explorer') {
  const { snapshot } = await seedArchiveState()
  return renderSurface(
    <>
      <ExplorerPage />
      <LocationProbe />
    </>,
    { language: 'en', route, snapshot },
  )
}

describe('Explorer view toggle', () => {
  beforeEach(() => {
    resetIntelligenceSurfaceHarness()
    vi.spyOn(backend, 'getStarStatus').mockResolvedValue({})
    vi.spyOn(backend, 'getStarCounts').mockResolvedValue({
      urls: 0,
      domains: 0,
    })
    vi.spyOn(backend, 'listStars').mockResolvedValue([])
    vi.spyOn(backend, 'queryHistory').mockResolvedValue(historyFixture())
  })

  test('offers Time / Session / Trail and puts the choice in the URL', async () => {
    const user = userEvent.setup()
    await renderBrowse()

    expect(await screen.findByText('Archived page 1')).toBeVisible()
    const toggle = await screen.findByTestId('explorer-view-toggle')
    expect(toggle).toHaveAccessibleName(explorerT('viewModeLabel'))

    // All three grouping modes are offered, and Time is the active one.
    const timeTab = screen.getByRole('tab', { name: explorerT('viewModeTime') })
    const sessionTab = screen.getByRole('tab', {
      name: explorerT('viewModeSession'),
    })
    expect(
      screen.getByRole('tab', { name: explorerT('viewModeTrail') }),
    ).toBeVisible()
    expect(timeTab).toHaveAttribute('aria-selected', 'true')
    expect(locationSearch()).not.toContain('view=')

    await user.click(sessionTab)

    // The URL is the source of truth, and `setView` also seeds the default
    // recent window the grouped reads need — both written in ONE navigation.
    await waitFor(() => expect(locationSearch()).toContain('view=session'))
    expect(locationSearch()).toContain('start=')
    expect(locationSearch()).toContain('end=')
    // The Time surface is gone; the grouped surface is up.
    expect(screen.queryByText('Archived page 1')).toBeNull()
  })

  test('the grouped views carry the toggle so entering one is not a one-way trip', async () => {
    const user = userEvent.setup()
    await renderBrowse('/explorer?view=trail&start=2026-04-01&end=2026-04-30')

    const trailTab = await screen.findByRole('tab', {
      name: explorerT('viewModeTrail'),
    })
    expect(trailTab).toHaveAttribute('aria-selected', 'true')

    await user.click(
      screen.getByRole('tab', { name: explorerT('viewModeTime') }),
    )

    // Back to Time: the `view` param is removed rather than set to 'time', so
    // the default surface has one canonical URL.
    await waitFor(() => expect(locationSearch()).not.toContain('view='))
    expect(await screen.findByText('Archived page 1')).toBeVisible()
  })
})
