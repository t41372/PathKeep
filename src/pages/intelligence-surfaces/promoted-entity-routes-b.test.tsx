/**
 * @file promoted-entity-routes-b.test.tsx
 * @description Preserves the second half of route-first promoted Intelligence entity destinations and explainability hooks.
 * @module pages/intelligence-surfaces
 *
 * ## Responsibilities
 * - Keep session and trail route handoff behavior intact.
 * - Keep compare-set-focused trail context behavior intact.
 * - Keep explainability and supported path-flow grammar behavior intact.
 *
 * ## Non-Responsibilities
 * - Does not own day/query/refind/compare-set route assertions from the first promoted slice.
 * - Does not cover generic Explorer control behavior.
 * - Does not modify shared route helpers or add new public contracts.
 *
 * ## Dependencies
 * - Depends on the shared Intelligence surface harness in `test-helpers.tsx`.
 * - Uses shipped Intelligence route components for route-first entity destinations.
 * - Mocks `core-intelligence/api` so route assertions stay deterministic and reviewable.
 *
 * ## Performance Notes
 * - Reuses the seeded archive and shared harness to avoid duplicating bulky fixture setup.
 * - Keeps assertions focused on shipped route promises instead of broad re-render sweeps.
 */

import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Route, Routes } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import * as coreIntelligenceApi from '../../lib/core-intelligence/api'
import { createNamespaceTranslator } from '../../lib/i18n'
import {
  IntelligencePage,
  SessionInsightsRoutePage,
  TrailInsightsRoutePage,
} from '../intelligence'
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

  test('renders session insights as a route-first destination while keeping Explorer inline sessions', async () => {
    const { snapshot } = await seedArchiveState()
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
          trailId: 'trail-1',
          transitionType: 'link',
        },
      ],
      trails: [
        {
          trailId: 'trail-1',
          sessionId: 'session-1',
          initialQuery: 'sqlite wal',
          searchEngine: 'google',
          reformulationCount: 1,
          visitCount: 2,
          landingUrl: 'https://sqlite.org/wal.html',
          landingDomain: 'sqlite.org',
          firstVisitMs: Date.parse('2026-04-05T14:00:00Z'),
          lastVisitMs: Date.parse('2026-04-05T14:20:00Z'),
          maxDepth: 2,
          queries: ['sqlite wal', 'sqlite checkpoint'],
        },
      ],
    })

    renderSurface(
      <Routes>
        <Route
          path="/intelligence/session/:sessionId"
          element={<SessionInsightsRoutePage />}
        />
      </Routes>,
      {
        route:
          '/intelligence/session/session-1?range=custom&start=2026-04-01&end=2026-04-07&profileId=chrome:Default',
        snapshot,
      },
    )

    expect(
      await screen.findByRole('heading', { name: /SQLite WAL research/i }),
    ).toBeVisible()
    expect(
      screen.getByRole('link', { name: 'Open evidence in Explorer' }),
    ).toHaveAttribute(
      'href',
      '/explorer?profileId=chrome%3ADefault&start=2026-04-05&end=2026-04-05',
    )
    expect(screen.getByRole('link', { name: /sqlite wal/i })).toHaveAttribute(
      'href',
      '/intelligence/trail/trail-1?range=custom&start=2026-04-05&end=2026-04-05&profileId=chrome%3ADefault',
    )
  })

  test('keeps session route placeholders and empty optional sections honest', async () => {
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')
    const detailSpy = vi.spyOn(coreIntelligenceApi, 'getSessionDetail')

    const placeholder = renderSurface(
      <Routes>
        <Route
          path="/intelligence/session"
          element={<SessionInsightsRoutePage />}
        />
      </Routes>,
      {
        route: '/intelligence/session',
        snapshot,
      },
    )

    expect(
      await screen.findByText(intelligenceT('sessionGroupEmpty')),
    ).toBeVisible()
    expect(detailSpy).not.toHaveBeenCalled()
    placeholder.unmount()

    detailSpy.mockRejectedValueOnce(new Error('session unavailable'))
    const errorView = renderSurface(
      <Routes>
        <Route
          path="/intelligence/session/:sessionId"
          element={<SessionInsightsRoutePage />}
        />
      </Routes>,
      {
        route: '/intelligence/session/session-error',
        snapshot,
      },
    )

    expect(await screen.findByText('session unavailable')).toBeVisible()
    errorView.unmount()

    detailSpy.mockResolvedValueOnce({
      session: null,
      visits: [],
      trails: [],
    } as unknown as Awaited<
      ReturnType<typeof coreIntelligenceApi.getSessionDetail>
    >)
    const emptyDetail = renderSurface(
      <Routes>
        <Route
          path="/intelligence/session/:sessionId"
          element={<SessionInsightsRoutePage />}
        />
      </Routes>,
      {
        route: '/intelligence/session/session-empty-detail',
        snapshot,
      },
    )

    expect(
      await screen.findByText(intelligenceT('sessionGroupEmpty')),
    ).toBeVisible()
    emptyDetail.unmount()

    detailSpy.mockResolvedValueOnce({
      session: {
        sessionId: 'session-empty',
        firstVisitMs: Date.parse('2026-04-05T14:00:00Z'),
        lastVisitMs: Date.parse('2026-04-05T14:30:00Z'),
        visitCount: 1,
        searchCount: 0,
        domainCount: 1,
        isDeepDive: false,
        autoTitle: null,
      },
      visits: [
        {
          visitId: 104,
          url: 'https://www.sqlite.org/without-title.html',
          title: null,
          registrableDomain: 'sqlite.org',
          visitTimeMs: Date.parse('2026-04-05T14:05:00Z'),
          isSearchEvent: false,
          searchQuery: null,
          searchEngine: null,
          trailId: null,
          transitionType: 'typed',
        },
      ],
      trails: [],
    })

    renderSurface(
      <Routes>
        <Route
          path="/intelligence/session/:sessionId"
          element={<SessionInsightsRoutePage />}
        />
      </Routes>,
      {
        route:
          '/intelligence/session/session-empty?range=custom&start=2026-04-01&end=2026-04-07',
        snapshot,
      },
    )

    expect(
      await screen.findByRole('heading', {
        name: intelligenceT('sessionUntitled'),
      }),
    ).toBeVisible()
    expect(
      screen.getByText('https://www.sqlite.org/without-title.html'),
    ).toBeVisible()
    expect(screen.getByText(intelligenceT('trailGroupEmpty'))).toBeVisible()
  })

  test('renders trail insights with session handoff and member entity links', async () => {
    const { snapshot } = await seedArchiveState()
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
          visitId: 202,
          ordinal: 1,
          role: 'landing',
          url: 'https://www.sqlite.org/pragma.html',
          title: 'PRAGMA wal_checkpoint',
          registrableDomain: 'sqlite.org',
          visitTimeMs: Date.parse('2026-04-05T14:03:00Z'),
          searchQuery: null,
        },
      ],
    })

    renderSurface(
      <Routes>
        <Route
          path="/intelligence/trail/:trailId"
          element={<TrailInsightsRoutePage />}
        />
      </Routes>,
      {
        route:
          '/intelligence/trail/trail-1?range=custom&start=2026-04-01&end=2026-04-07&profileId=chrome:Default',
        snapshot,
      },
    )

    expect(
      await screen.findByRole('heading', { name: /sqlite wal checkpoint/i }),
    ).toBeVisible()
    expect(
      screen.getByRole('link', { name: 'Open session insights' }),
    ).toHaveAttribute(
      'href',
      '/intelligence/session/session-1?range=custom&start=2026-04-05&end=2026-04-05&profileId=chrome%3ADefault',
    )
    expect(screen.getByRole('link', { name: 'sqlite.org' })).toHaveAttribute(
      'href',
      '/intelligence/domain/sqlite.org?range=custom&start=2026-04-05&end=2026-04-05&profileId=chrome%3ADefault',
    )
  })

  test('keeps trail route placeholders and optional member actions honest', async () => {
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')
    const detailSpy = vi.spyOn(coreIntelligenceApi, 'getTrailDetail')

    const placeholder = renderSurface(
      <Routes>
        <Route
          path="/intelligence/trail"
          element={<TrailInsightsRoutePage />}
        />
      </Routes>,
      {
        route: '/intelligence/trail',
        snapshot,
      },
    )

    expect(
      await screen.findByText(intelligenceT('trailGroupEmpty')),
    ).toBeVisible()
    expect(detailSpy).not.toHaveBeenCalled()
    placeholder.unmount()

    detailSpy.mockRejectedValueOnce(new Error('trail unavailable'))
    const errorView = renderSurface(
      <Routes>
        <Route
          path="/intelligence/trail/:trailId"
          element={<TrailInsightsRoutePage />}
        />
      </Routes>,
      {
        route: '/intelligence/trail/trail-error',
        snapshot,
      },
    )

    expect(await screen.findByText('trail unavailable')).toBeVisible()
    errorView.unmount()

    detailSpy.mockResolvedValueOnce({
      trail: null,
      members: [],
    } as unknown as Awaited<
      ReturnType<typeof coreIntelligenceApi.getTrailDetail>
    >)
    const emptyDetail = renderSurface(
      <Routes>
        <Route
          path="/intelligence/trail/:trailId"
          element={<TrailInsightsRoutePage />}
        />
      </Routes>,
      {
        route: '/intelligence/trail/trail-empty-detail',
        snapshot,
      },
    )

    expect(
      await screen.findByText(intelligenceT('trailGroupEmpty')),
    ).toBeVisible()
    emptyDetail.unmount()

    detailSpy.mockResolvedValueOnce({
      trail: {
        trailId: 'trail-empty',
        sessionId: null,
        initialQuery: 'sqlite triggers',
        searchEngine: 'Google',
        reformulationCount: 0,
        visitCount: 2,
        landingUrl: 'https://www.sqlite.org/lang_createtrigger.html',
        landingDomain: 'sqlite.org',
        firstVisitMs: Date.parse('2026-04-05T14:00:00Z'),
        lastVisitMs: Date.parse('2026-04-05T14:20:00Z'),
        maxDepth: 1,
        queries: ['sqlite triggers'],
      },
      members: [
        {
          trailId: 'trail-empty',
          visitId: 302,
          ordinal: 1,
          role: 'search_event',
          url: 'https://google.com/search?q=sqlite+triggers',
          title: null,
          registrableDomain: null,
          visitTimeMs: Date.parse('2026-04-05T14:03:00Z'),
          searchQuery: 'sqlite triggers',
        },
        {
          trailId: 'trail-empty',
          visitId: 303,
          ordinal: 2,
          role: 'click',
          url: 'https://www.sqlite.org/lang_createtrigger.html',
          title: null,
          registrableDomain: null,
          visitTimeMs: Date.parse('2026-04-05T14:06:00Z'),
          searchQuery: null,
        },
      ],
    })

    renderSurface(
      <Routes>
        <Route
          path="/intelligence/trail/:trailId"
          element={<TrailInsightsRoutePage />}
        />
      </Routes>,
      {
        route:
          '/intelligence/trail/trail-empty?range=custom&start=2026-04-01&end=2026-04-07',
        snapshot,
      },
    )

    expect(
      await screen.findByRole('heading', { name: /sqlite triggers/i }),
    ).toBeVisible()
    expect(
      screen.queryByRole('link', {
        name: intelligenceT('trailRouteOpenSession'),
      }),
    ).not.toBeInTheDocument()
    expect(screen.getAllByText('"sqlite triggers"').length).toBeGreaterThan(0)
    expect(
      screen.getByText('https://www.sqlite.org/lang_createtrigger.html'),
    ).toBeVisible()
  })

  test('shows compare-set focus context inside trail insights and highlights matching members', async () => {
    const { snapshot } = await seedArchiveState()
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
          visitId: 202,
          ordinal: 1,
          role: 'landing',
          url: 'https://www.sqlite.org/pragma.html',
          canonicalUrl: 'https://www.sqlite.org/pragma.html',
          title: 'PRAGMA wal_checkpoint',
          registrableDomain: 'sqlite.org',
          visitTimeMs: Date.parse('2026-04-05T14:03:00Z'),
          searchQuery: null,
        },
        {
          trailId: 'trail-1',
          visitId: 203,
          ordinal: 2,
          role: 'click',
          url: 'https://www.sqlite.org/wal.html',
          canonicalUrl: 'https://www.sqlite.org/wal.html',
          title: 'WAL docs',
          registrableDomain: 'sqlite.org',
          visitTimeMs: Date.parse('2026-04-05T14:06:00Z'),
          searchQuery: null,
        },
      ],
    })
    vi.spyOn(coreIntelligenceApi, 'getCompareSetDetail').mockResolvedValue(
      wrapSection('compare-set-detail', {
        compareSet: {
          compareSetId: 'compare:trail-1:docs_page',
          trailId: 'trail-1',
          searchQuery: 'sqlite wal',
          pageCategory: 'docs_page',
          pages: [
            {
              canonicalUrl: 'https://www.sqlite.org/pragma.html',
              url: 'https://www.sqlite.org/pragma.html',
              title: 'PRAGMA wal_checkpoint',
              registrableDomain: 'sqlite.org',
              visitCount: 2,
              isLanding: true,
            },
          ],
        },
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
        session: null,
        recentDays: ['2026-04-05'],
      }),
    )

    renderSurface(
      <Routes>
        <Route
          path="/intelligence/trail/:trailId"
          element={<TrailInsightsRoutePage />}
        />
      </Routes>,
      {
        route:
          '/intelligence/trail/trail-1?range=custom&start=2026-04-01&end=2026-04-07&profileId=chrome:Default&focusType=compare-set&focusId=compare%3Atrail-1%3Adocs_page',
        snapshot,
      },
    )

    expect(await screen.findByText('Focused compare set')).toBeVisible()
    expect(
      screen
        .getAllByRole('link', { name: '2026-04-05' })
        .some(
          (link) =>
            link.getAttribute('href') ===
            '/intelligence/day/2026-04-05?profileId=chrome%3ADefault&focusType=compare-set&focusId=compare%3Atrail-1%3Adocs_page',
        ),
    ).toBe(true)
    expect(
      screen.getByText('PRAGMA wal_checkpoint').closest('.trail-member-row'),
    ).toHaveClass('trail-member-row--focused')
  })

  test('limits path-flow steps to supported values and wires explainability to supported intelligence entities', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')

    vi.spyOn(coreIntelligenceApi, 'getRefindPages').mockResolvedValue(
      wrapSection('refind-pages', [
        {
          canonicalUrl: 'https://example.com/reference',
          url: 'https://example.com/reference',
          title: 'Reference page',
          registrableDomain: 'example.com',
          crossDayCount: 4,
          trailCount: 3,
          searchArrivalCount: 2,
          typedRevisitCount: 1,
          refindScore: 0.9,
          firstSeenAt: '2026-04-01T00:00:00Z',
          lastSeenAt: '2026-04-07T00:00:00Z',
        },
      ]),
    )
    vi.spyOn(coreIntelligenceApi, 'getQueryFamilies').mockResolvedValue(
      wrapSection('search-activity', {
        families: [],
        total: 0,
        page: 0,
        pageSize: 10,
      }),
    )
    vi.spyOn(
      coreIntelligenceApi,
      'getReopenedInvestigations',
    ).mockResolvedValue(wrapSection('reopened-investigations', []))
    vi.spyOn(coreIntelligenceApi, 'getHabitPatterns').mockResolvedValue(
      wrapSection('habits', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getInterruptedHabits').mockResolvedValue(
      wrapSection('habits', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getPathFlows').mockResolvedValue(
      wrapSection('path-flows', []),
    )
    const explainSpy = vi
      .spyOn(coreIntelligenceApi, 'explainEntity')
      .mockResolvedValue({
        entityType: 'refind_page',
        entityId: 'https://example.com/reference',
        triggerRule: 'Refind score >= 0.7',
        factors: [],
        participatingVisitIds: [1, 2],
      })

    renderSurface(<IntelligencePage />, {
      route: '/intelligence?profileId=chrome:Default',
      snapshot,
    })

    expect(
      await screen.findByRole('button', {
        name: intelligenceT('explainTitle'),
      }),
    ).toBeVisible()
    expect(
      screen.queryByText(intelligenceT('pathFlowsStep4')),
    ).not.toBeInTheDocument()

    const refindSection = screen
      .getByText(intelligenceT('refindTitle'))
      .closest('section')
    expect(refindSection).not.toBeNull()
    if (!(refindSection instanceof HTMLElement)) {
      throw new Error('expected refind section')
    }

    expect(
      within(refindSection).getByRole('link', { name: 'Reference page' }),
    ).toHaveAttribute(
      'href',
      '/intelligence/refind/https%3A%2F%2Fexample.com%2Freference?range=all&profileId=chrome%3ADefault',
    )
    expect(
      within(refindSection).getByRole('button', {
        name: /Show score factors/i,
      }),
    ).toBeVisible()

    await user.click(
      within(refindSection).getByRole('button', {
        name: /Show score factors/i,
      }),
    )
    expect(within(refindSection).getByText('Cross-day revisits')).toBeVisible()

    await user.click(
      within(refindSection).getByRole('button', {
        name: intelligenceT('explainTitle'),
      }),
    )

    await waitFor(() =>
      expect(explainSpy).toHaveBeenCalledWith(
        'refind_page',
        'https://example.com/reference',
      ),
    )
  })
})
