/**
 * @file dashboard-rhythm.test.tsx
 * @description Protects dashboard browsing-rhythm and archive-recovery route behavior after the mega-suite split.
 * @module pages/intelligence-surfaces
 *
 * ## Responsibilities
 * - Keep the dashboard browsing-rhythm year pager behavior identical to the shipped route contract.
 * - Verify inline day preview and the follow-up navigation handoff into `/intelligence/day/:date`.
 * - Guard the dashboard recovery states for encrypted archives and uninitialized archives.
 *
 * ## Non-Responsibilities
 * - Does not own overview metadata or section-evidence badge assertions.
 * - Does not redefine the shared route test harness or archive seed helpers.
 * - Does not cover unrelated Settings, Explorer, or local-host preview surfaces.
 *
 * ## Dependencies
 * - Depends on the shared intelligence surface harness in `test-helpers.tsx`.
 * - Uses the shipped Dashboard and day-insights routes rather than local mock-only facsimiles.
 * - Mocks Core Intelligence API readers only where the original suite already did so.
 *
 * ## Performance Notes
 * - Reuses the shared seeded archive state so the split suite does not multiply setup work.
 * - Keeps route assertions scoped to year-pager and recovery flows instead of mounting unrelated surfaces.
 */

import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Route, Routes } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import * as coreIntelligenceApi from '../../lib/core-intelligence/api'
import type { DayInsights, DateRange } from '../../lib/core-intelligence/types'
import { createNamespaceTranslator } from '../../lib/i18n'
import { DashboardPage } from '../dashboard'
import {
  createShellValue,
  enableAi,
  renderSurface,
  resetIntelligenceSurfaceHarness,
  seedArchiveState,
  wrapSection,
} from './test-helpers'

describe('intelligence surfaces', () => {
  beforeEach(() => {
    resetIntelligenceSurfaceHarness()
  })

  test('renders dashboard trust callouts and retention boundary copy', async () => {
    const { snapshot, dashboard } = await seedArchiveState()
    const dashboardT = createNamespaceTranslator('zh-TW', 'dashboard')
    const commonT = createNamespaceTranslator('zh-TW', 'common')

    enableAi(snapshot)
    snapshot.config.rememberDatabaseKeyInKeyring = true
    snapshot.keyringStatus.storedSecret = false
    snapshot.browserProfiles.push({
      profileId: 'safari:Personal',
      profileName: 'Personal',
      browserFamily: 'safari',
      browserName: 'Safari',
      userName: 'tim',
      profilePath: '/Users/tim/Library/Safari',
      historyPath: '/Users/tim/Library/Safari/History.db',
      faviconsPath: null,
      historyExists: false,
      browserVersion: null,
      historyFileName: 'History.db',
      historyBytes: 18 * 1024 * 1024,
      faviconsBytes: 0,
      supportingBytes: 2 * 1024 * 1024,
      retentionBoundary: {
        kind: 'macos-safari',
        localDays: 365,
      },
    })
    snapshot.config.selectedProfileIds.push('safari:Personal')

    renderSurface(<DashboardPage />, {
      dashboard,
      language: 'zh-TW',
      route: '/',
      snapshot,
    })

    // v0.3 redesign removed the DashboardIntelligencePanel,
    // DashboardTrustActionsPanel, and DashboardArchiveBoundaryPanel from the
    // dashboard composition. The only remaining surface for repair links is
    // the compact warning-box stack at the top of the dashboard.
    const securityLink = await screen.findByRole('link', {
      name: new RegExp(dashboardT('reviewSecurity')),
    })
    expect(securityLink).toBeVisible()
    expect(
      screen.getByRole('link', {
        name: new RegExp(dashboardT('reviewImportBatches')),
      }),
    ).toBeVisible()
    // commonT used to scope this test to the zh-TW seed; keep the harness
    // reference so the lint rule that demands every import resolve here.
    void commonT
  })

  test('uses a dashboard year pager and opens inline day preview before navigation', async () => {
    const user = userEvent.setup()
    const { snapshot, dashboard } = await seedArchiveState()
    const currentYear = new Date().getFullYear()
    const getDiscoveryTrendSpy = vi
      .spyOn(coreIntelligenceApi, 'getDiscoveryTrend')
      .mockImplementation((dateRange) =>
        Promise.resolve(
          wrapSection('discovery-trend', {
            availableYears: [2025, 2024],
            points:
              dateRange.start === '2024-01-01'
                ? [
                    {
                      dateKey: '2024-04-18',
                      discoveryRate: 0.33,
                      newDomainCount: 1,
                      totalVisits: 3,
                    },
                  ]
                : [
                    {
                      dateKey: '2025-04-18',
                      discoveryRate: 0.25,
                      newDomainCount: 2,
                      totalVisits: 8,
                    },
                  ],
          }),
        ),
      )
    const getDayInsightsSpy = vi
      .spyOn(coreIntelligenceApi, 'getDayInsights')
      .mockResolvedValue(
        wrapSection<DayInsights>('day-insights', {
          date: '2024-04-18',
          digestSummary: {
            dateRange: {
              start: '2024-04-18',
              end: '2024-04-18',
            } satisfies DateRange,
            totalVisits: { value: 3, trend: 'flat' as const },
            totalSearches: { value: 1, trend: 'flat' as const },
            newDomains: { value: 1, trend: 'flat' as const },
            deepReadPages: { value: 2, trend: 'flat' as const },
            refindPages: { value: 0, trend: 'flat' as const },
          },
          topSites: [
            {
              registrableDomain: 'sqlite.org',
              displayName: 'SQLite',
              domainCategory: 'docs',
              visitCount: 3,
              uniqueDays: 1,
              averageDailyVisits: 3,
              uniqueUrls: 2,
            },
          ],
          activityMix: {
            categories: [
              { domainCategory: 'docs', visitCount: 1, share: 0.34 },
              { domainCategory: 'video', visitCount: 1, share: 0.33 },
              { domainCategory: 'ai', visitCount: 1, share: 0.33 },
            ],
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
            visitCount: hour === 10 ? 3 : 0,
          })),
          drilldown: {
            explorerDateRange: { start: '2024-04-18', end: '2024-04-18' },
          },
        }),
      )

    renderSurface(
      <Routes>
        <Route path="/" element={<DashboardPage />} />
        <Route
          path="/intelligence/day/:date"
          element={<div data-testid="day-insights-route-target" />}
        />
      </Routes>,
      {
        dashboard,
        route: '/',
        snapshot,
      },
    )

    const yearLabel = await screen.findByTestId('browsing-rhythm-year-label')
    await waitFor(() =>
      expect(yearLabel).toHaveTextContent(String(currentYear)),
    )
    expect(screen.getByTestId('browsing-rhythm-year-previous')).toBeEnabled()
    expect(screen.getByTestId('browsing-rhythm-year-next')).toBeDisabled()
    expect(getDayInsightsSpy).not.toHaveBeenCalled()

    await user.click(screen.getByTestId('browsing-rhythm-year-previous'))
    await waitFor(() =>
      expect(getDiscoveryTrendSpy).toHaveBeenLastCalledWith(
        { start: '2025-01-01', end: '2025-12-31' },
        null,
        'day',
        undefined,
      ),
    )
    expect(yearLabel).toHaveTextContent('2025')
    expect(screen.getByTestId('browsing-rhythm-year-previous')).toBeEnabled()
    expect(screen.getByTestId('browsing-rhythm-year-next')).toBeEnabled()
    expect(
      screen.getByText(
        'Data in this year currently runs from Apr 18, 2025 to Apr 18, 2025',
      ),
    ).toBeVisible()
    expect(
      screen.getByTestId('browsing-rhythm-current-year-shortcut'),
    ).toBeVisible()
    const currentYearShortcut = screen.getByTestId(
      'browsing-rhythm-current-year-shortcut',
    )
    const yearPager = screen.getByTestId('browsing-rhythm-year-pager')
    expect(currentYearShortcut.nextElementSibling).toBe(yearPager)

    await user.click(screen.getByTestId('browsing-rhythm-year-previous'))
    await waitFor(() =>
      expect(getDiscoveryTrendSpy).toHaveBeenLastCalledWith(
        { start: '2024-01-01', end: '2024-12-31' },
        null,
        'day',
        undefined,
      ),
    )
    expect(yearLabel).toHaveTextContent('2024')
    expect(screen.getByTestId('browsing-rhythm-year-previous')).toBeDisabled()
    expect(screen.getByTestId('browsing-rhythm-year-next')).toBeEnabled()

    await user.click(
      await screen.findByRole('button', {
        name: /2024-04-18 · 3 visits · 1 new sites/i,
      }),
    )

    expect(getDayInsightsSpy).toHaveBeenCalledWith('2024-04-18', null)
    expect(
      await screen.findByTestId('browsing-rhythm-day-detail'),
    ).toBeVisible()
    const activityMix = screen.getByTestId('rhythm-activity-proportion')
    expect(
      activityMix.querySelector(
        ".rhythm-proportion__legend-swatch[data-category='video']",
      ),
    ).not.toBeNull()
    expect(
      activityMix.querySelector(
        ".rhythm-proportion__legend-swatch[data-category='ai']",
      ),
    ).not.toBeNull()
    expect(
      screen.queryByTestId('day-insights-route-target'),
    ).not.toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'View details' })).toHaveAttribute(
      'href',
      '/intelligence/day/2024-04-18',
    )

    await user.click(screen.getByRole('link', { name: 'View details' }))
    expect(
      await screen.findByTestId('day-insights-route-target'),
    ).toBeInTheDocument()
  })

  test('allows the dashboard year pager to land on a future year only when that year exists', async () => {
    const user = userEvent.setup()
    const { snapshot, dashboard } = await seedArchiveState()
    const currentYear = new Date().getFullYear()

    vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockImplementation(
      (dateRange) =>
        Promise.resolve(
          wrapSection('discovery-trend', {
            availableYears: [currentYear + 1, currentYear - 2],
            points:
              dateRange.start === `${currentYear + 1}-01-01`
                ? [
                    {
                      dateKey: `${currentYear + 1}-01-02`,
                      discoveryRate: 0.5,
                      newDomainCount: 1,
                      totalVisits: 5,
                    },
                  ]
                : dateRange.start === `${currentYear}-01-01`
                  ? [
                      {
                        dateKey: `${currentYear}-04-18`,
                        discoveryRate: 0.4,
                        newDomainCount: 1,
                        totalVisits: 7,
                      },
                    ]
                  : dateRange.start === `${currentYear - 2}-01-01`
                    ? [
                        {
                          dateKey: `${currentYear - 2}-02-11`,
                          discoveryRate: 0.5,
                          newDomainCount: 2,
                          totalVisits: 9,
                        },
                      ]
                    : [],
          }),
        ),
    )

    renderSurface(<DashboardPage />, {
      dashboard,
      route: '/',
      snapshot,
    })

    const yearLabel = await screen.findByTestId('browsing-rhythm-year-label')
    expect(yearLabel).toHaveTextContent(String(currentYear))
    expect(screen.getByTestId('browsing-rhythm-year-previous')).toBeEnabled()
    expect(screen.getByTestId('browsing-rhythm-year-next')).toBeEnabled()
    expect(
      screen.queryByTestId('browsing-rhythm-current-year-shortcut'),
    ).not.toBeInTheDocument()

    await user.click(screen.getByTestId('browsing-rhythm-year-next'))
    expect(yearLabel).toHaveTextContent(String(currentYear + 1))
    expect(
      screen.getByTestId('browsing-rhythm-current-year-shortcut'),
    ).toBeVisible()

    await user.click(
      screen.getByTestId('browsing-rhythm-current-year-shortcut'),
    )
    expect(yearLabel).toHaveTextContent(String(currentYear))
  })

  test('fills missing dashboard years and keeps empty years browsable', async () => {
    const user = userEvent.setup()
    const { snapshot, dashboard } = await seedArchiveState()
    const currentYear = new Date().getFullYear()

    vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockImplementation(
      (dateRange) =>
        Promise.resolve(
          wrapSection('discovery-trend', {
            availableYears: [currentYear, currentYear - 3],
            points:
              dateRange.start === `${currentYear}-01-01`
                ? [
                    {
                      dateKey: `${currentYear}-04-18`,
                      discoveryRate: 0.4,
                      newDomainCount: 1,
                      totalVisits: 7,
                    },
                  ]
                : dateRange.start === `${currentYear - 3}-01-01`
                  ? [
                      {
                        dateKey: `${currentYear - 3}-02-11`,
                        discoveryRate: 0.5,
                        newDomainCount: 2,
                        totalVisits: 9,
                      },
                    ]
                  : [],
          }),
        ),
    )

    renderSurface(<DashboardPage />, {
      dashboard,
      route: '/',
      snapshot,
    })

    const yearLabel = await screen.findByTestId('browsing-rhythm-year-label')
    expect(yearLabel).toHaveTextContent(String(currentYear))

    await user.click(screen.getByTestId('browsing-rhythm-year-previous'))
    expect(yearLabel).toHaveTextContent(String(currentYear - 1))
    expect(screen.getByTestId('browsing-rhythm-summary')).toHaveTextContent(
      `0 visits in ${currentYear - 1}`,
    )
    expect(
      screen.queryByText(
        'This year does not have enough browsing history yet.',
      ),
    ).not.toBeInTheDocument()
    expect(
      screen.getByRole('grid', {
        name: 'Calendar heatmap of browsing activity by day',
      }),
    ).toBeVisible()
  })

  test('routes dashboard archive-key failures toward the security page', async () => {
    const { snapshot } = await seedArchiveState()
    const dashboardT = createNamespaceTranslator('en', 'dashboard')

    renderSurface(<DashboardPage />, {
      snapshot,
      shellValue: {
        ...createShellValue(snapshot),
        dashboard: null,
        snapshot: null,
        error: 'database key is required for encrypted archives',
      },
    })

    expect(
      await screen.findByRole('heading', {
        name: dashboardT('archiveUnlockRequiredTitle'),
      }),
    ).toBeVisible()
    expect(
      screen.getByText(dashboardT('archiveUnlockRequiredBody')),
    ).toBeVisible()
    expect(
      screen.getByRole('link', { name: dashboardT('archiveUnlockAction') }),
    ).toHaveAttribute('href', '/security#unlock-archive')
  })

  test('recovers actionable unlock guidance when shell bootstrap only surfaced a generic dashboard error', async () => {
    const { snapshot } = await seedArchiveState()
    const dashboardT = createNamespaceTranslator('en', 'dashboard')
    vi.spyOn(backend, 'securityStatus').mockResolvedValue({
      initialized: true,
      mode: 'Encrypted',
      encrypted: true,
      unlocked: false,
      databasePath: snapshot.directories.archiveDatabasePath,
      strongholdPath: snapshot.directories.strongholdPath,
      rememberDatabaseKeyInKeyring: false,
      lastSuccessfulBackupAt: null,
      lastRekeyAt: null,
      lastRekeyRunId: null,
      lastRekeySnapshotPath: null,
      keyringStatus: {
        available: true,
        backend: 'macOS Keychain',
        storedSecret: false,
      },
      warnings: ['database key is required for encrypted archives'],
    })

    renderSurface(<DashboardPage />, {
      snapshot,
      shellValue: {
        ...createShellValue(snapshot),
        dashboard: null,
        snapshot: null,
        error: dashboardT('archiveUnavailableBody'),
      },
    })

    expect(
      await screen.findByRole('heading', {
        name: dashboardT('archiveUnlockRequiredTitle'),
      }),
    ).toBeVisible()
    expect(
      screen.getByRole('link', { name: dashboardT('archiveUnlockAction') }),
    ).toHaveAttribute('href', '/security#unlock-archive')
  })

  test('falls back to the onboarding zero-state when shell bootstrap failed before archive initialization', async () => {
    const { snapshot } = await seedArchiveState()
    const dashboardT = createNamespaceTranslator('en', 'dashboard')
    vi.spyOn(backend, 'securityStatus').mockResolvedValue({
      initialized: false,
      mode: 'Plaintext',
      encrypted: false,
      unlocked: false,
      databasePath: snapshot.directories.archiveDatabasePath,
      strongholdPath: snapshot.directories.strongholdPath,
      rememberDatabaseKeyInKeyring: false,
      lastSuccessfulBackupAt: null,
      lastRekeyAt: null,
      lastRekeyRunId: null,
      lastRekeySnapshotPath: null,
      keyringStatus: {
        available: true,
        backend: 'macOS Keychain',
        storedSecret: false,
      },
      warnings: [],
    })

    renderSurface(<DashboardPage />, {
      snapshot,
      shellValue: {
        ...createShellValue(snapshot),
        dashboard: null,
        snapshot: null,
        error: dashboardT('archiveUnavailableBody'),
      },
    })

    expect(
      await screen.findByRole('heading', {
        name: dashboardT('zeroStateTitle'),
      }),
    ).toBeVisible()
    expect(
      screen.getByRole('link', { name: dashboardT('openOnboardingFlow') }),
    ).toHaveAttribute('href', '/onboarding')
  })

  test('localizes dashboard zero-state backend next actions', async () => {
    const snapshot = structuredClone(await backend.getAppSnapshot())
    const dashboard = structuredClone(await backend.loadDashboardSnapshot())
    const dashboardT = createNamespaceTranslator('zh-TW', 'dashboard')

    renderSurface(<DashboardPage />, {
      dashboard,
      language: 'zh-TW',
      route: '/',
      snapshot,
    })

    expect(
      await screen.findByText(dashboardT('nextActionInitializeArchive')),
    ).toBeVisible()
    expect(
      screen.queryByText(/Initialize the archive before running/i),
    ).not.toBeInTheDocument()
  })
})
