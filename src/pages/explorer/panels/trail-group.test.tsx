/**
 * @file trail-group.test.tsx
 * @description Coverage for the Explorer search-trail workbench panel.
 * @module pages/explorer/panels
 *
 * ## Responsibilities
 * - Protect loading, empty/error, pagination, expansion, and visit-selection behavior.
 * - Keep search-trail display resilient across the engines PathKeep recognizes.
 * - Verify trail-member fallbacks without coupling the test to route chrome.
 *
 * ## Not responsible for
 * - Re-testing URL redaction, which lives in the privacy redaction suite.
 * - Re-testing the shared workbench primitive keyboard contract.
 *
 * ## Dependencies
 * - Uses the real i18n catalog and Core Intelligence API module with per-test spies.
 *
 * ## Performance notes
 * - Fixtures stay intentionally small; pagination is exercised by metadata, not by rendering dozens of rows.
 */

import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { createNamespaceTranslator } from '../../../lib/i18n'
import * as api from '../../../lib/core-intelligence/api'
import type {
  TrailListResult,
  TrailMember,
  TrailSummary,
} from '../../../lib/core-intelligence/types'
import { TrailGroupPanel } from './trail-group'

const explorerT = createNamespaceTranslator('en', 'explorer')
const intelligenceT = createNamespaceTranslator('en', 'intelligence')
const dateRange = { start: '2026-04-01', end: '2026-04-30' }

describe('TrailGroupPanel', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  test('shows the loading skeleton while trails are pending', () => {
    vi.spyOn(api, 'getSearchTrails').mockReturnValue(new Promise(() => {}))

    const { container } = renderTrailGroupPanel()

    expect(
      container.querySelector('.intelligence-skeleton--list'),
    ).toBeInTheDocument()
  })

  test('shows backend errors and empty trail copy', async () => {
    vi.spyOn(api, 'getSearchTrails').mockRejectedValueOnce(
      new Error('trail backend unavailable'),
    )

    const { unmount } = renderTrailGroupPanel()

    expect(await screen.findByText('trail backend unavailable')).toBeVisible()

    unmount()
    vi.restoreAllMocks()
    vi.spyOn(api, 'getSearchTrails').mockResolvedValueOnce(
      createTrailList([], { total: 0 }),
    )
    renderTrailGroupPanel()

    expect(
      await screen.findByText(intelligenceT('trailGroupEmpty')),
    ).toBeVisible()
  })

  test('renders engines, opens trail details, selects visits, and paginates', async () => {
    const user = userEvent.setup()
    const trails = [
      createTrail({ trailId: 'trail-google', searchEngine: 'Google' }),
      createTrail({ trailId: 'trail-bing', searchEngine: 'Bing' }),
      createTrail({ trailId: 'trail-youtube', searchEngine: 'YouTube' }),
      createTrail({ trailId: 'trail-bilibili', searchEngine: 'Bilibili' }),
      createTrail({ trailId: 'trail-github', searchEngine: 'GitHub' }),
      createTrail({ trailId: 'trail-duck', searchEngine: 'DuckDuckGo' }),
      createTrail({ trailId: 'trail-baidu', searchEngine: 'Baidu' }),
      createTrail({ trailId: 'trail-unknown', searchEngine: 'Kagi' }),
    ]
    const getSearchTrails = vi
      .spyOn(api, 'getSearchTrails')
      .mockResolvedValue(createTrailList(trails, { total: 41 }))
    const getTrailDetail = vi.spyOn(api, 'getTrailDetail').mockResolvedValue({
      trail: trails[0],
      members: createTrailMembers(),
    })
    const onSelectVisit = vi.fn()

    renderTrailGroupPanel({ onSelectVisit, profileId: 'chrome:Default' })

    expect(await screen.findByText('"Trail google"')).toBeVisible()
    expect(screen.getByText('"Trail bing"')).toBeVisible()
    expect(screen.getByText('"Trail youtube"')).toBeVisible()
    expect(screen.getByText('"Trail bilibili"')).toBeVisible()
    expect(screen.getByText('"Trail github"')).toBeVisible()
    expect(screen.getByText('"Trail duck"')).toBeVisible()
    expect(screen.getByText('"Trail baidu"')).toBeVisible()
    expect(screen.getByText('"Trail unknown"')).toBeVisible()
    expect(
      screen.getByText(
        intelligenceT('trailGroupSummary', { count: 41, page: 1 }),
      ),
    ).toBeVisible()

    await user.click(screen.getByText('"Trail google"'))

    expect(await screen.findByText('"Trail google refined"')).toBeVisible()
    await user.click(screen.getByText('"Search member"'))
    expect(onSelectVisit).toHaveBeenCalledWith(
      expect.objectContaining({
        domain: 'google.com',
        profileId: 'chrome:Default',
        title: '"Search member"',
        transition: 'search_event',
        url: 'https://www.google.com/search?q=trail',
        visitId: 101,
      }),
    )
    await user.click(screen.getByText('Landing article'))
    expect(onSelectVisit).toHaveBeenCalledWith(
      expect.objectContaining({
        domain: 'example.com',
        title: 'Landing article',
        transition: 'landing',
        visitId: 102,
      }),
    )
    expect(screen.getByText('example.com/page')).toBeVisible()
    await user.click(screen.getAllByText('"Trail google"')[0])
    await user.click(screen.getAllByText('"Trail google"')[0])
    await waitFor(() => {
      expect(getTrailDetail).toHaveBeenCalledTimes(1)
    })

    await user.click(
      screen.getByRole('button', { name: explorerT('nextPage') }),
    )
    await waitFor(() => {
      expect(getSearchTrails).toHaveBeenCalledWith(
        dateRange,
        'chrome:Default',
        undefined,
        { page: 1, pageSize: 20 },
      )
    })
    await user.click(
      screen.getByRole('button', { name: explorerT('previousPage') }),
    )
    await waitFor(() => {
      expect(getSearchTrails).toHaveBeenCalledWith(
        dateRange,
        'chrome:Default',
        undefined,
        { page: 0, pageSize: 20 },
      )
    })
  })

  test('keeps the expanded card usable when detail loading fails', async () => {
    const user = userEvent.setup()
    vi.spyOn(api, 'getSearchTrails').mockResolvedValue(
      createTrailList([createTrail()]),
    )
    vi.spyOn(api, 'getTrailDetail').mockRejectedValue(
      new Error('detail unavailable'),
    )

    renderTrailGroupPanel()

    await user.click(await screen.findByText('"Trail google"'))

    await waitFor(() => {
      expect(api.getTrailDetail).toHaveBeenCalledWith('trail-google')
    })
    expect(
      screen.getByText(intelligenceT('trailRouteOpenInsights')),
    ).toBeVisible()
    expect(screen.queryByText('detail unavailable')).not.toBeInTheDocument()
  })

  test('renders locale variants and trail-evolution landing fallbacks', async () => {
    const user = userEvent.setup()
    const trails = [
      createTrail({
        trailId: 'trail-no-domain',
        queries: ['Trail no domain', 'Trail no domain refined'],
        landingDomain: null,
        landingUrl: 'https://fallback.example/path',
      }),
      createTrail({
        trailId: 'trail-no-landing',
        queries: ['Trail no landing', 'Trail no landing refined'],
        landingDomain: null,
        landingUrl: undefined,
      }),
    ]
    vi.spyOn(api, 'getSearchTrails').mockResolvedValue(createTrailList(trails))
    vi.spyOn(api, 'getTrailDetail').mockResolvedValue({
      trail: trails[0],
      members: [],
    })

    const first = renderTrailGroupPanel({ language: 'zh-CN' })
    expect(await screen.findByText('"Trail no-domain"')).toBeVisible()
    await user.click(screen.getByText('"Trail no-domain"'))
    expect(screen.getByText(/fallback\.example\/path/)).toBeVisible()
    first.unmount()

    renderTrailGroupPanel({ language: 'zh-TW' })
    expect(await screen.findByText('"Trail no-landing"')).toBeVisible()
    await user.click(screen.getByText('"Trail no-landing"'))
    expect(screen.getByText('"Trail no landing refined"')).toBeVisible()
    expect(
      screen.queryByText(intelligenceT('trailLanding')),
    ).not.toBeInTheDocument()
  })
})

function renderTrailGroupPanel({
  language = 'en',
  onSelectVisit,
  profileId,
}: {
  language?: Parameters<typeof TrailGroupPanel>[0]['language']
  onSelectVisit?: Parameters<typeof TrailGroupPanel>[0]['onSelectVisit']
  profileId?: string | null
} = {}) {
  return render(
    <TrailGroupPanel
      dateRange={dateRange}
      explorerT={explorerT}
      intelligenceT={intelligenceT}
      language={language}
      onSelectVisit={onSelectVisit}
      profileId={profileId}
    />,
  )
}

function createTrailList(
  trails: TrailSummary[],
  overrides: Partial<TrailListResult> = {},
): TrailListResult {
  return {
    trails,
    total: trails.length,
    page: 0,
    pageSize: 20,
    ...overrides,
  }
}

function createTrail({
  landingDomain,
  landingUrl,
  queries,
  trailId = 'trail-google',
  searchEngine = 'Google',
}: {
  landingDomain?: string | null
  landingUrl?: string
  queries?: string[]
  trailId?: string
  searchEngine?: string
} = {}): TrailSummary {
  const suffix = trailId.replace('trail-', '')

  return {
    trailId,
    sessionId: `session-${suffix}`,
    initialQuery: `Trail ${suffix}`,
    searchEngine,
    reformulationCount: suffix === 'google' ? 1 : 0,
    visitCount: 3,
    landingUrl:
      landingUrl ??
      (suffix === 'google' ? 'https://example.com/landing' : undefined),
    landingDomain:
      landingDomain ?? (suffix === 'google' ? 'example.com' : null),
    firstVisitMs: Date.parse('2026-04-18T12:00:00Z'),
    lastVisitMs: Date.parse('2026-04-18T12:10:00Z'),
    maxDepth: 2,
    queries:
      queries ??
      (suffix === 'google'
        ? ['Trail google', 'Trail google refined']
        : [`Trail ${suffix}`]),
  }
}

function createTrailMembers(): TrailMember[] {
  return [
    {
      trailId: 'trail-google',
      visitId: 101,
      ordinal: 1,
      role: 'search_event',
      url: 'https://www.google.com/search?q=trail',
      title: null,
      visitTimeMs: Date.parse('2026-04-18T12:00:00Z'),
      searchQuery: 'Search member',
      registrableDomain: 'google.com',
    },
    {
      trailId: 'trail-google',
      visitId: 102,
      ordinal: 2,
      role: 'landing',
      url: 'https://example.com/landing',
      title: 'Landing article',
      visitTimeMs: Date.parse('2026-04-18T12:02:00Z'),
      registrableDomain: 'example.com',
    },
    {
      trailId: 'trail-google',
      visitId: 103,
      ordinal: 3,
      role: 'click',
      url: 'https://example.com/page',
      title: null,
      visitTimeMs: Date.parse('2026-04-18T12:04:00Z'),
      registrableDomain: null,
    },
  ]
}
