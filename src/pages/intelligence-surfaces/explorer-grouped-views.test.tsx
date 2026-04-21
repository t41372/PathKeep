/**
 * @file explorer-grouped-views.test.tsx
 * @description Protects Explorer grouped session and trail views plus the routed domain deep-dive handoff.
 * @module pages/intelligence-surfaces
 *
 * ## Responsibilities
 * - Preserve grouped session-view navigation tracing behavior.
 * - Preserve grouped trail-view detail-rail selection behavior.
 * - Preserve domain deep-dive route contracts for scoped date ranges and profile scope.
 *
 * ## Non-Responsibilities
 * - Does not own generic test harness setup; shared helpers provide that contract.
 * - Does not cover day/query/refind/session/trail entity routes outside this owned slice.
 * - Does not modify the original mega-suite or any shared route component.
 *
 * ## Dependencies
 * - Depends on the shared Intelligence surface harness in `test-helpers.tsx`.
 * - Uses the shipped Explorer and Intelligence route components as integration surfaces.
 * - Talks to `core-intelligence/api` mocks to keep route behavior deterministic.
 *
 * ## Performance Notes
 * - Reuses the seeded archive snapshot from the shared harness instead of rebuilding custom state per test.
 * - Keeps assertions route-level so the split suite stays faithful without adding extra render churn.
 */

import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Route, Routes } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import * as coreIntelligenceApi from '../../lib/core-intelligence/api'
import { createNamespaceTranslator } from '../../lib/i18n'
import { ExplorerPage } from '../explorer'
import { DomainDeepDiveRoutePage } from '../intelligence'
import {
  renderSurface,
  resetIntelligenceSurfaceHarness,
  seedArchiveState,
  wrapSection,
} from './test-helpers'

describe('intelligence surfaces', () => {
  beforeEach(() => {
    resetIntelligenceSurfaceHarness()
  })

  test('renders explorer session view and keeps navigation tracing wired to the selected grouped visit', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')

    vi.spyOn(coreIntelligenceApi, 'getSessions').mockResolvedValue({
      sessions: [
        {
          sessionId: 'session-1',
          firstVisitMs: Date.parse('2026-04-05T14:00:00Z'),
          lastVisitMs: Date.parse('2026-04-05T14:30:00Z'),
          visitCount: 3,
          searchCount: 1,
          domainCount: 2,
          isDeepDive: true,
          autoTitle: 'SQLite WAL research',
        },
      ],
      total: 1,
      page: 0,
      pageSize: 20,
    })
    vi.spyOn(coreIntelligenceApi, 'getSessionDetail').mockResolvedValue({
      session: {
        sessionId: 'session-1',
        firstVisitMs: Date.parse('2026-04-05T14:00:00Z'),
        lastVisitMs: Date.parse('2026-04-05T14:30:00Z'),
        visitCount: 3,
        searchCount: 1,
        domainCount: 2,
        isDeepDive: true,
        autoTitle: 'SQLite WAL research',
      },
      visits: [
        {
          visitId: 101,
          url: 'https://www.sqlite.org/wal.html',
          title: 'SQLite WAL',
          registrableDomain: 'sqlite.org',
          visitTimeMs: Date.parse('2026-04-05T14:05:00Z'),
          isSearchEvent: false,
          searchQuery: null,
          searchEngine: null,
          trailId: null,
          transitionType: 'link',
        },
      ],
      trails: [],
    })
    vi.spyOn(coreIntelligenceApi, 'getNavigationPath').mockResolvedValue({
      targetVisitId: 101,
      steps: [
        {
          visitId: 100,
          url: 'https://www.google.com/search?q=sqlite+wal',
          title: 'Google',
          visitTimeMs: Date.parse('2026-04-05T14:04:00Z'),
          depth: 0,
        },
        {
          visitId: 101,
          url: 'https://www.sqlite.org/wal.html',
          title: 'SQLite WAL',
          visitTimeMs: Date.parse('2026-04-05T14:05:00Z'),
          depth: 1,
        },
      ],
    })

    renderSurface(<ExplorerPage />, {
      route: '/explorer?view=session&start=2026-04-01&end=2026-04-07',
      snapshot,
    })

    expect(await screen.findByText('SQLite WAL research')).toBeVisible()
    await user.click(
      screen.getByRole('button', { name: /SQLite WAL research/i }),
    )
    await user.click(await screen.findByText('SQLite WAL'))
    await user.click(
      screen.getByRole('button', { name: intelligenceT('tracerTitle') }),
    )
    expect(await screen.findByText('Google')).toBeVisible()
    expect(
      screen.getByText(new RegExp(intelligenceT('tracerHere'))),
    ).toBeVisible()
  })

  test('renders explorer trail view and keeps grouped selection wired to the detail rail', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')

    vi.spyOn(coreIntelligenceApi, 'getSearchTrails').mockResolvedValue({
      trails: [
        {
          trailId: 'trail-1',
          sessionId: 'session-1',
          initialQuery: 'sqlite wal checkpoint',
          searchEngine: 'Google',
          reformulationCount: 1,
          visitCount: 2,
          landingUrl: 'https://www.sqlite.org/pragma.html',
          landingDomain: 'sqlite.org',
          firstVisitMs: Date.parse('2026-04-05T14:00:00Z'),
          lastVisitMs: Date.parse('2026-04-05T14:20:00Z'),
          maxDepth: 2,
          queries: ['sqlite wal checkpoint', 'sqlite wal checkpoint passive'],
        },
      ],
      total: 1,
      page: 0,
      pageSize: 20,
    })
    vi.spyOn(coreIntelligenceApi, 'getTrailDetail').mockResolvedValue({
      trail: {
        trailId: 'trail-1',
        sessionId: 'session-1',
        initialQuery: 'sqlite wal checkpoint',
        searchEngine: 'Google',
        reformulationCount: 1,
        visitCount: 2,
        landingUrl: 'https://www.sqlite.org/pragma.html',
        landingDomain: 'sqlite.org',
        firstVisitMs: Date.parse('2026-04-05T14:00:00Z'),
        lastVisitMs: Date.parse('2026-04-05T14:20:00Z'),
        maxDepth: 2,
        queries: ['sqlite wal checkpoint', 'sqlite wal checkpoint passive'],
      },
      members: [
        {
          trailId: 'trail-1',
          visitId: 201,
          ordinal: 0,
          role: 'search_event',
          url: 'https://www.google.com/search?q=sqlite+wal+checkpoint+passive',
          title: 'Google',
          visitTimeMs: Date.parse('2026-04-05T14:02:00Z'),
          searchQuery: 'sqlite wal checkpoint passive',
        },
        {
          trailId: 'trail-1',
          visitId: 202,
          ordinal: 1,
          role: 'landing',
          url: 'https://www.sqlite.org/pragma.html',
          title: 'PRAGMA wal_checkpoint',
          visitTimeMs: Date.parse('2026-04-05T14:03:00Z'),
          searchQuery: null,
        },
      ],
    })
    vi.spyOn(coreIntelligenceApi, 'getNavigationPath').mockResolvedValue({
      targetVisitId: 202,
      steps: [
        {
          visitId: 201,
          url: 'https://www.google.com/search?q=sqlite+wal+checkpoint+passive',
          title: 'Google',
          visitTimeMs: Date.parse('2026-04-05T14:02:00Z'),
          depth: 0,
        },
        {
          visitId: 202,
          url: 'https://www.sqlite.org/pragma.html',
          title: 'PRAGMA wal_checkpoint',
          visitTimeMs: Date.parse('2026-04-05T14:03:00Z'),
          depth: 1,
        },
      ],
    })

    renderSurface(<ExplorerPage />, {
      route: '/explorer?view=trail&start=2026-04-01&end=2026-04-07',
      snapshot,
    })

    expect(await screen.findByText('"sqlite wal checkpoint"')).toBeVisible()
    await user.click(
      screen.getByRole('button', { name: /sqlite wal checkpoint/i }),
    )
    await user.click(await screen.findByText('PRAGMA wal_checkpoint'))
    await user.click(
      screen.getByRole('button', { name: intelligenceT('tracerTitle') }),
    )
    expect(
      (await screen.findAllByText('PRAGMA wal_checkpoint')).length,
    ).toBeGreaterThan(1)
  })

  test('keeps domain deep dives deep-linkable and preserves route-backed scope and date range', async () => {
    const { snapshot } = await seedArchiveState()

    const domainSpy = vi
      .spyOn(coreIntelligenceApi, 'getDomainDeepDive')
      .mockResolvedValue(
        wrapSection(
          'domain-deep-dive',
          {
            registrableDomain: 'github.com',
            displayName: 'GitHub',
            domainCategory: 'developer',
            totalVisits: 38,
            activeDays: 7,
            trailCount: 4,
            arrivalBreakdown: { search: 10, link: 12, typed: 8, other: 8 },
            topPages: [{ path: '/issues', visitCount: 12 }],
            topReferrers: [
              { domain: 'google.com', displayName: 'Google', count: 6 },
            ],
            topExits: [
              {
                domain: 'stackoverflow.com',
                displayName: 'Stack Overflow',
                count: 4,
              },
            ],
            visitTrend: [{ dateKey: '2026-04-05', visitCount: 6 }],
          },
          {
            moduleIds: ['daily-rollups', 'search-trails', 'domain-deep-dive'],
            sourceTables: [
              'visit_derived_facts',
              'domain_daily_rollups',
              'search_trails',
              'habit_patterns',
              'path_flows',
            ],
          },
        ),
      )
    const domainQueriesSpy = vi
      .spyOn(coreIntelligenceApi, 'getSearchQueries')
      .mockResolvedValue(
        wrapSection('search-activity', {
          page: 0,
          pageSize: 20,
          total: 1,
          rows: [
            {
              visitId: 200,
              profileId: 'chrome:Default',
              browserKind: 'chrome',
              searchEngine: 'github',
              displayName: 'GitHub',
              rawQuery: 'pathkeep sqlite',
              normalizedQuery: 'pathkeep sqlite',
              searchedAt: '2026-04-05T12:00:00Z',
              searchedAtMs: Date.parse('2026-04-05T12:00:00Z'),
              exactRepeatCount: 2,
              familyCount: 3,
              familyId: 'family-200',
              trailId: 'trail-200',
              trailInitialQuery: 'pathkeep sqlite',
              trailReformulationCount: 1,
            },
          ],
        }),
      )

    renderSurface(
      <Routes>
        <Route
          path="/intelligence/domain/:domain"
          element={<DomainDeepDiveRoutePage />}
        />
      </Routes>,
      {
        route:
          '/intelligence/domain/github.com?range=custom&start=2026-04-01&end=2026-04-07&profileId=chrome:Default',
        snapshot,
      },
    )

    expect(await screen.findByTestId('domain-deep-dive-page')).toBeVisible()
    expect(await screen.findByTestId('domain-scope-strip')).toBeVisible()
    await waitFor(() =>
      expect(domainSpy).toHaveBeenCalledWith(
        'github.com',
        { start: '2026-04-01', end: '2026-04-07' },
        'chrome:Default',
      ),
    )
    await waitFor(() =>
      expect(domainQueriesSpy).toHaveBeenCalledWith(
        { start: '2026-04-01', end: '2026-04-07' },
        expect.objectContaining({
          profileId: 'chrome:Default',
          domain: 'github.com',
          pagination: { page: 0, pageSize: 20 },
        }),
      ),
    )
    expect(
      screen.getByRole('heading', { name: 'What You Searched On This Site' }),
    ).toBeVisible()
    expect(screen.getByRole('link', { name: /Back/i })).toHaveAttribute(
      'href',
      '/intelligence?range=custom&start=2026-04-01&end=2026-04-07&profileId=chrome%3ADefault',
    )
  })
})
