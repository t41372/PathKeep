/**
 * @file overview-metadata.test.tsx
 * @description Protects overview-card scroll containment and section-metadata behavior after the mega-suite split.
 * @module pages/intelligence-surfaces
 *
 * ## Responsibilities
 * - Keep the top-sites scroll-region contract intact so long overview lists stay bounded.
 * - Verify stale, disabled, degraded, and ready section metadata on overview and day-insights surfaces.
 * - Guard metadata refresh behavior when profile scope or time range changes.
 *
 * ## Non-Responsibilities
 * - Does not own dashboard rhythm or archive recovery assertions.
 * - Does not redefine the shared archive seed or standard render harness for other suites.
 * - Does not modify production route behavior or the original mega-suite file.
 *
 * ## Dependencies
 * - Depends on the shared route test helpers for seeded archive state and standard provider wiring.
 * - Uses shipped `IntelligencePage` and `DayInsightsRoutePage` surfaces to preserve route-level behavior.
 * - Reuses production profile-scope context for the one scope-switching metadata regression.
 *
 * ## Performance Notes
 * - Keeps the custom provider stack limited to the profile-switch regression that truly needs it.
 * - Reuses shared section-envelope helpers so split suites do not duplicate mock-shape setup.
 */

import { useState } from 'react'
import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import * as coreIntelligenceApi from '../../lib/core-intelligence/api'
import type { DayInsights } from '../../lib/core-intelligence/types'
import { createNamespaceTranslator } from '../../lib/i18n'
import { I18nContext } from '../../lib/i18n/context'
import { ProfileScopeContext } from '../../lib/profile-scope-context'
import { DayInsightsRoutePage, IntelligencePage } from '../intelligence'
import {
  createI18nValue,
  createShellValue,
  renderSurface,
  resetIntelligenceSurfaceHarness,
  seedArchiveState,
  wrapSection,
} from './test-helpers'
import { ShellDataContext } from '../../app/shell-data-context'

describe('intelligence surfaces', () => {
  beforeEach(() => {
    resetIntelligenceSurfaceHarness()
  })

  test('renders top-sites inside a scroll region so long lists do not stretch the section', async () => {
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')
    vi.spyOn(coreIntelligenceApi, 'getTopSites').mockResolvedValue(
      wrapSection(
        'top-sites',
        Array.from({ length: 20 }, (_, index) => ({
          registrableDomain: `example-${index + 1}.com`,
          displayName: `Example ${index + 1}`,
          domainCategory: 'reference',
          visitCount: 100 - index,
          uniqueDays: 20 - Math.floor(index / 2),
          averageDailyVisits: Number((5 - index * 0.1).toFixed(1)),
          uniqueUrls: 10 - Math.floor(index / 3),
        })),
        {
          moduleIds: ['daily-rollups'],
          sourceTables: ['domain_daily_rollups'],
        },
      ),
    )

    renderSurface(<IntelligencePage />, {
      route: '/intelligence?profileId=chrome:Default',
      snapshot,
    })

    expect(await screen.findByText('Example 1')).toBeVisible()

    const topSitesSection = screen
      .getByRole('heading', { name: intelligenceT('topSitesTitle') })
      .closest('section')
    expect(topSitesSection).not.toBeNull()
    if (!(topSitesSection instanceof HTMLElement)) {
      throw new Error('expected top sites section')
    }

    expect(
      topSitesSection.querySelector('.intelligence-section__scroll-region'),
    ).not.toBeNull()
  })

  test('renders section-level metadata for stale, disabled, and degraded intelligence sections', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')
    const settingsT = createNamespaceTranslator('en', 'settings')

    vi.spyOn(coreIntelligenceApi, 'getTopSites').mockResolvedValue(
      wrapSection('top-sites', [], {
        state: 'stale',
        stateReason: 'Visibility changed after the last deterministic rebuild.',
        moduleIds: ['daily-rollups'],
        sourceTables: ['domain_daily_rollups'],
        notes: [
          'Manual rebuild required before these summaries are fresh again.',
        ],
        window: {
          kind: 'date-range',
          dateRange: { start: '2026-04-01', end: '2026-04-07' },
        },
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getStableSources').mockResolvedValue(
      wrapSection('stable-sources', [], {
        state: 'disabled',
        stateReason: 'Disabled in Settings.',
        moduleIds: ['refind-pages'],
        sourceTables: ['source_effectiveness'],
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getObservedInteractions').mockResolvedValue(
      wrapSection('observed-interactions', [], {
        state: 'degraded',
        stateReason:
          'No supported browser-reported interaction evidence is available for this scope yet.',
        sourceTables: ['visit_engagement_evidence'],
      }),
    )

    renderSurface(<IntelligencePage />, {
      route:
        '/intelligence?range=custom&start=2026-04-01&end=2026-04-07&profileId=chrome:Default',
      snapshot,
    })

    expect(
      await screen.findByTestId('intelligence-section-meta-top-sites'),
    ).toBeVisible()

    const topSitesSection = screen
      .getByRole('heading', { name: intelligenceT('topSitesTitle') })
      .closest('section')
    if (!(topSitesSection instanceof HTMLElement)) {
      throw new Error('expected top sites section')
    }
    const topSitesMeta = within(topSitesSection).getByTestId(
      'intelligence-section-meta-top-sites',
    )
    await user.click(
      within(topSitesMeta).getByRole('button', {
        name: intelligenceT('sectionMetaOpenPanelAria'),
      }),
    )
    expect(
      within(topSitesMeta).getAllByText(settingsT('deterministicModuleStale'))
        .length,
    ).toBeGreaterThan(0)
    expect(within(topSitesMeta).getByText('domain_daily_rollups')).toBeVisible()
    expect(
      within(topSitesMeta).getByText(
        'Visibility changed after the last deterministic rebuild.',
      ),
    ).toBeVisible()

    const stableSourcesSection = screen
      .getByRole('heading', { name: intelligenceT('stableSourcesTitle') })
      .closest('section')
    if (!(stableSourcesSection instanceof HTMLElement)) {
      throw new Error('expected stable sources section')
    }
    const stableSourcesMeta = within(stableSourcesSection).getByTestId(
      'intelligence-section-meta-stable-sources',
    )
    await user.click(
      within(stableSourcesMeta).getByRole('button', {
        name: intelligenceT('sectionMetaOpenPanelAria'),
      }),
    )
    expect(
      within(stableSourcesMeta).getAllByText(
        settingsT('deterministicModuleDisabled'),
      ).length,
    ).toBeGreaterThan(0)

    const observedSection = screen
      .getByRole('heading', { name: intelligenceT('observedTitle') })
      .closest('section')
    if (!(observedSection instanceof HTMLElement)) {
      throw new Error('expected observed interactions section')
    }
    const observedMeta = within(observedSection).getByTestId(
      'intelligence-section-meta-observed-interactions',
    )
    await user.click(
      within(observedMeta).getByRole('button', {
        name: intelligenceT('sectionMetaOpenPanelAria'),
      }),
    )
    expect(
      within(observedMeta).getAllByText(
        intelligenceT('sectionMetaStateDegraded'),
      ).length,
    ).toBeGreaterThan(0)
    expect(
      within(observedMeta).getByText(
        'No supported browser-reported interaction evidence is available for this scope yet.',
      ),
    ).toBeVisible()
  })

  test('renders compact evidence badges on overview and day-insights surfaces', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')
    const settingsT = createNamespaceTranslator('en', 'settings')

    vi.spyOn(coreIntelligenceApi, 'getTopSites').mockResolvedValue(
      wrapSection(
        'top-sites',
        [
          {
            registrableDomain: 'sqlite.org',
            displayName: 'SQLite',
            domainCategory: 'docs',
            visitCount: 3,
            uniqueDays: 2,
            averageDailyVisits: 1.5,
            uniqueUrls: 2,
          },
        ],
        {
          moduleIds: ['daily-rollups'],
          sourceTables: ['domain_daily_rollups'],
        },
      ),
    )

    const overview = renderSurface(<IntelligencePage />, {
      route: '/intelligence?profileId=chrome:Default',
      snapshot,
    })

    const overviewMeta = await screen.findByTestId(
      'intelligence-section-meta-top-sites',
    )
    expect(
      within(overviewMeta).getByText(intelligenceT('sectionMetaTitle')),
    ).toBeVisible()
    expect(
      within(overviewMeta).getByText(settingsT('deterministicModuleReady')),
    ).toBeVisible()

    await user.click(
      within(overviewMeta).getByRole('button', {
        name: intelligenceT('sectionMetaOpenPanelAria'),
      }),
    )
    expect(
      within(overviewMeta).getByText(intelligenceT('sectionMetaGeneratedAt')),
    ).toBeVisible()

    overview.unmount()

    vi.spyOn(coreIntelligenceApi, 'getDayInsights').mockResolvedValue(
      wrapSection<DayInsights>(
        'day-insights',
        {
          date: '2026-04-18',
          digestSummary: {
            dateRange: { start: '2026-04-18', end: '2026-04-18' },
            totalVisits: { value: 8, trend: 'flat' },
            totalSearches: { value: 3, trend: 'flat' },
            newDomains: { value: 2, trend: 'flat' },
            deepReadPages: { value: 4, trend: 'flat' },
            refindPages: { value: 1, trend: 'flat' },
          },
          topSites: [],
          activityMix: {
            categories: [{ domainCategory: 'docs', visitCount: 8, share: 1 }],
            changeVsPrevious: [],
          },
          refindPages: [],
          queryFamilies: {
            families: [],
            total: 0,
            page: 0,
            pageSize: 8,
          },
          hourlyActivity: Array.from({ length: 24 }, (_, hour) => ({
            hour,
            visitCount: hour === 10 ? 4 : 0,
          })),
          drilldown: {
            explorerDateRange: { start: '2026-04-18', end: '2026-04-18' },
          },
        },
        {
          moduleIds: ['daily-rollups', 'activity-mix'],
          sourceTables: ['daily_summary_rollups', 'category_daily_rollups'],
        },
      ),
    )

    renderSurface(
      <Routes>
        <Route
          path="/intelligence/day/:date"
          element={<DayInsightsRoutePage />}
        />
      </Routes>,
      {
        route: '/intelligence/day/2026-04-18?profileId=chrome:Default',
        snapshot,
      },
    )

    const dayMeta = await screen.findByTestId(
      'intelligence-section-meta-day-insights',
    )
    expect(
      within(dayMeta).getByText(intelligenceT('sectionMetaTitle')),
    ).toBeVisible()
    expect(
      within(dayMeta).getByText(settingsT('deterministicModuleReady')),
    ).toBeVisible()
  })

  test('refreshes section metadata when intelligence scope or time range changes', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')

    const topSitesSpy = vi
      .spyOn(coreIntelligenceApi, 'getTopSites')
      .mockImplementation((dateRange, profileId) =>
        Promise.resolve(
          wrapSection('top-sites', [], {
            state: 'stale',
            stateReason: 'Scope-sensitive test metadata.',
            moduleIds: ['daily-rollups'],
            sourceTables: ['domain_daily_rollups'],
            window: {
              kind: 'date-range',
              dateRange,
            },
            notes: [profileId ?? 'archive-wide'],
          }),
        ),
      )

    function ScopedIntelligenceHarness() {
      const [activeProfileId, setActiveProfileId] = useState<string | null>(
        'chrome:Default',
      )

      return (
        <MemoryRouter
          initialEntries={[
            '/intelligence?range=custom&start=2026-04-01&end=2026-04-07',
          ]}
        >
          <I18nContext.Provider value={createI18nValue('en')}>
            <ProfileScopeContext.Provider
              value={{ activeProfileId, setActiveProfileId }}
            >
              <ShellDataContext.Provider value={createShellValue(snapshot)}>
                <button
                  type="button"
                  onClick={() => setActiveProfileId('firefox:Research')}
                >
                  Switch profile
                </button>
                <Routes>
                  <Route path="/intelligence" element={<IntelligencePage />} />
                </Routes>
              </ShellDataContext.Provider>
            </ProfileScopeContext.Provider>
          </I18nContext.Provider>
        </MemoryRouter>
      )
    }

    render(<ScopedIntelligenceHarness />)

    const topSitesMeta = await screen.findByTestId(
      'intelligence-section-meta-top-sites',
    )
    await user.click(
      within(topSitesMeta).getByRole('button', {
        name: intelligenceT('sectionMetaOpenPanelAria'),
      }),
    )
    expect(
      within(topSitesMeta).getByText('2026-04-01 → 2026-04-07'),
    ).toBeVisible()
    expect(within(topSitesMeta).getByText('chrome:Default')).toBeVisible()

    await user.click(screen.getByRole('button', { name: 'Switch profile' }))
    await waitFor(() => {
      expect(
        topSitesSpy.mock.calls.some(
          (call) =>
            call[0]?.start === '2026-04-01' &&
            call[0]?.end === '2026-04-07' &&
            call[1] === 'firefox:Research',
        ),
      ).toBe(true)
    })
    expect(
      await screen.findByText(/Core Intelligence is only reading Research/i),
    ).toBeVisible()

    const previousTopSitesCallCount = topSitesSpy.mock.calls.length

    await user.click(
      screen.getByRole('button', { name: intelligenceT('rangeWeek') }),
    )
    await waitFor(() => {
      expect(topSitesSpy.mock.calls.length).toBeGreaterThan(
        previousTopSitesCallCount,
      )
      const matchingWeekCalls = topSitesSpy.mock.calls.filter(
        (call) =>
          call[1] === 'firefox:Research' &&
          (call[0]?.start !== '2026-04-01' || call[0]?.end !== '2026-04-07'),
      )
      expect(matchingWeekCalls.length).toBeGreaterThan(0)
    })
    expect(
      screen.queryByText('2026-04-01 → 2026-04-07'),
    ).not.toBeInTheDocument()
  })
})
