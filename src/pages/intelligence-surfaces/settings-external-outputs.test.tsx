/**
 * @file settings-external-outputs.test.tsx
 * @description Protects the Settings external-output review surface after the mega-suite split.
 * @module pages/intelligence-surfaces
 *
 * ## Responsibilities
 * - Preserve the manual external-output review assertions that previously lived in the mega-suite.
 * - Verify archive-readiness gating, scoped refetch behavior, and tab switching without changing test titles.
 * - Reuse the shared Intelligence surface harness so split suites keep one setup contract.
 *
 * ## Non-Responsibilities
 * - Does not redefine the generic route render harness or archive seeding logic.
 * - Does not cover trusted local-host build/open actions; those live in the sibling local-host suite.
 *
 * ## Dependencies
 * - Depends on the shared Intelligence test helpers and local-host preview fixtures.
 * - Uses the shipped Settings route plus profile/i18n/shell contexts to keep behavior production-shaped.
 *
 * ## Performance Notes
 * - Reuses seeded archive fixtures and shared reset logic instead of rebuilding bespoke harness state.
 */

import { useState } from 'react'
import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { ShellDataContext } from '../../app/shell-data-context'
import { backend } from '../../lib/backend-client'
import * as coreIntelligenceApi from '../../lib/core-intelligence/api'
import { I18nContext } from '../../lib/i18n/context'
import { createNamespaceTranslator } from '../../lib/i18n'
import { ProfileScopeContext } from '../../lib/profile-scope-context'
import type { AppSnapshot } from '../../lib/types'
import { IntegrationsPage } from '../integrations'
import { createLocalHostPreview } from './local-host-fixtures'
import {
  createEmptyRuntimeSnapshot,
  createI18nValue,
  createShellValue,
  renderSurface,
  resetIntelligenceSurfaceHarness,
  seedArchiveState,
} from './test-helpers'

describe('intelligence surfaces', () => {
  beforeEach(() => {
    resetIntelligenceSurfaceHarness()
  })

  test('renders settings manual external outputs review and lets the user switch surfaces', async () => {
    const user = userEvent.setup()
    const { snapshot, dashboard } = await seedArchiveState()
    const settingsT = createNamespaceTranslator('en', 'settings')
    const commonT = createNamespaceTranslator('en', 'common')

    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
      createEmptyRuntimeSnapshot(),
    )
    const embedSpy = vi
      .spyOn(coreIntelligenceApi, 'getIntelligenceEmbedCards')
      .mockResolvedValue([
        {
          cardId: 'digest:visits',
          cardType: 'digest',
          title: 'Visits',
          eyebrow: '2026-03-17 → 2026-04-17',
          body: 'Total visits in the selected intelligence window.',
          metricLabel: 'visit_count',
          metricValue: '128',
          href: null,
          internalOnly: false,
        },
        {
          cardId: 'refind:sqlite',
          cardType: 'refind_page',
          title: 'SQLite WAL guide',
          eyebrow: 'Refind',
          body: 'This page kept resurfacing across 4 days and 3 trails.',
          metricLabel: 'refind_score',
          metricValue: '0.82',
          href: 'https://sqlite.org/wal.html',
          primaryTarget: {
            kind: 'refindPage',
            canonicalUrl: 'https://sqlite.org/wal.html',
          },
          secondaryTargets: [
            {
              kind: 'domain',
              domain: 'sqlite.org',
            },
            {
              kind: 'day',
              date: '2026-04-14',
            },
          ],
          internalOnly: true,
        },
      ])
    const widgetSpy = vi
      .spyOn(coreIntelligenceApi, 'getIntelligenceWidgetSnapshot')
      .mockResolvedValue({
        generatedAt: '2026-04-17T09:45:00Z',
        dateRange: { start: '2026-03-17', end: '2026-04-17' },
        digestSummary: {
          dateRange: { start: '2026-03-17', end: '2026-04-17' },
          totalVisits: {
            value: 128,
            previousValue: 120,
            changePercent: 7,
            trend: 'up',
          },
          totalSearches: {
            value: 32,
            previousValue: 28,
            changePercent: 14,
            trend: 'up',
          },
          newDomains: {
            value: 9,
            previousValue: 8,
            changePercent: 13,
            trend: 'up',
          },
          deepReadPages: {
            value: 5,
            previousValue: 4,
            changePercent: 25,
            trend: 'up',
          },
          refindPages: {
            value: 3,
            previousValue: 2,
            changePercent: 50,
            trend: 'up',
          },
        },
        highlights: [
          {
            cardId: 'refind:sqlite',
            cardType: 'refind_page',
            title: 'SQLite WAL guide',
            eyebrow: 'Refind',
            body: 'This page kept resurfacing across 4 days and 3 trails.',
            metricLabel: 'refind_score',
            metricValue: '0.82',
            href: 'https://sqlite.org/wal.html',
            internalOnly: true,
          },
        ],
        notes: [
          'Widget snapshots only expose aggregate Core Intelligence read models.',
        ],
      })
    const publicSpy = vi
      .spyOn(coreIntelligenceApi, 'getIntelligencePublicSnapshot')
      .mockResolvedValue({
        generatedAt: '2026-04-17T09:45:00Z',
        dateRange: { start: '2026-03-17', end: '2026-04-17' },
        digestSummary: {
          dateRange: { start: '2026-03-17', end: '2026-04-17' },
          totalVisits: {
            value: 128,
            previousValue: 120,
            changePercent: 7,
            trend: 'up',
          },
          totalSearches: {
            value: 32,
            previousValue: 28,
            changePercent: 14,
            trend: 'up',
          },
          newDomains: {
            value: 9,
            previousValue: 8,
            changePercent: 13,
            trend: 'up',
          },
          deepReadPages: {
            value: 5,
            previousValue: 4,
            changePercent: 25,
            trend: 'up',
          },
          refindPages: {
            value: 3,
            previousValue: 2,
            changePercent: 50,
            trend: 'up',
          },
        },
        topDomains: ['sqlite.org', 'github.com'],
        searchEngines: [
          {
            searchEngine: 'google',
            displayName: 'Google',
            searchCount: 18,
          },
        ],
        discoveryTrend: {
          availableYears: [],
          points: [
            {
              dateKey: '2026-04-07',
              discoveryRate: 0.35,
              newDomainCount: 4,
              totalVisits: 22,
            },
            {
              dateKey: '2026-04-14',
              discoveryRate: 0.41,
              newDomainCount: 5,
              totalVisits: 24,
            },
          ],
        },
        notes: [
          'Public snapshots intentionally omit visit-level identifiers and direct page URLs.',
        ],
      })
    const localHostPreviewSpy = vi
      .spyOn(coreIntelligenceApi, 'previewIntelligenceLocalHost')
      .mockResolvedValue(createLocalHostPreview('en'))

    renderSurface(<IntegrationsPage />, {
      dashboard,
      language: 'en',
      route: '/integrations',
      snapshot,
    })

    const panel = await screen.findByTestId('settings-external-outputs')
    await waitFor(() => {
      expect(embedSpy).toHaveBeenCalledTimes(1)
      expect(widgetSpy).toHaveBeenCalledTimes(1)
      expect(publicSpy).toHaveBeenCalledTimes(1)
      expect(localHostPreviewSpy).toHaveBeenCalledTimes(1)
    })
    expect(
      within(panel).getByText(settingsT('externalOutputsSummaryTitle')),
    ).toBeVisible()
    expect(within(panel).getByText('SQLite WAL guide')).toBeVisible()
    expect(
      within(panel).getByText(settingsT('externalOutputsTrustedOnlyBadge')),
    ).toBeVisible()
    expect(within(panel).getByText('sqlite.org')).toBeVisible()
    expect(within(panel).getByText('2026-04-14')).toBeVisible()

    await user.click(
      within(panel).getByRole('tab', {
        name: settingsT('externalOutputsTabWidget'),
      }),
    )
    expect(
      await within(panel).findByText(
        settingsT('externalOutputsWidgetTrustedTitle'),
      ),
    ).toBeVisible()
    await user.click(
      within(panel).getAllByRole('button', { name: commonT('copyAction') })[0],
    )
    expect(
      await within(panel).findByText(commonT('copiedNotice')),
    ).toBeVisible()

    await user.click(
      within(panel).getByRole('tab', {
        name: settingsT('externalOutputsTabPublic'),
      }),
    )
    expect(
      await within(panel).findByText(
        settingsT('externalOutputsPublicRedactedTitle'),
      ),
    ).toBeVisible()
    expect(within(panel).getByText('sqlite.org')).toBeVisible()
    expect(
      within(panel).getByText(
        settingsT('externalOutputsLocalHostSummaryTitle'),
      ),
    ).toBeVisible()
    expect(
      within(panel).getByRole('button', {
        name: settingsT('externalOutputsLocalHostCreateAction'),
      }),
    ).toBeVisible()
  })

  test.each([
    {
      expectedTitleKey: 'externalOutputsUnlockTitle',
      mutate: (snapshot: AppSnapshot) => {
        snapshot.archiveStatus.unlocked = false
      },
    },
    {
      expectedTitleKey: 'externalOutputsNeedsArchiveTitle',
      mutate: (snapshot: AppSnapshot) => {
        snapshot.config.initialized = false
      },
    },
  ])(
    'keeps settings manual external outputs gated behind archive readiness truth ($expectedTitleKey)',
    async ({ expectedTitleKey, mutate }) => {
      const { snapshot, dashboard } = await seedArchiveState()
      const settingsT = createNamespaceTranslator('en', 'settings')
      mutate(snapshot)

      const embedSpy = vi.spyOn(
        coreIntelligenceApi,
        'getIntelligenceEmbedCards',
      )
      const widgetSpy = vi.spyOn(
        coreIntelligenceApi,
        'getIntelligenceWidgetSnapshot',
      )
      const publicSpy = vi.spyOn(
        coreIntelligenceApi,
        'getIntelligencePublicSnapshot',
      )
      const localHostPreviewSpy = vi.spyOn(
        coreIntelligenceApi,
        'previewIntelligenceLocalHost',
      )
      vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
        createEmptyRuntimeSnapshot(),
      )

      renderSurface(<IntegrationsPage />, {
        dashboard,
        language: 'en',
        route: '/integrations',
        snapshot,
      })

      const panel = await screen.findByTestId('settings-external-outputs')
      expect(within(panel).getByText(settingsT(expectedTitleKey))).toBeVisible()
      expect(embedSpy).not.toHaveBeenCalled()
      expect(widgetSpy).not.toHaveBeenCalled()
      expect(publicSpy).not.toHaveBeenCalled()
      expect(localHostPreviewSpy).not.toHaveBeenCalled()
    },
  )

  test('refetches settings manual external outputs when shared scope or time range changes', async () => {
    const user = userEvent.setup()
    const { snapshot, dashboard } = await seedArchiveState()
    const settingsT = createNamespaceTranslator('en', 'settings')
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')

    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
      createEmptyRuntimeSnapshot(),
    )
    const embedSpy = vi
      .spyOn(coreIntelligenceApi, 'getIntelligenceEmbedCards')
      .mockResolvedValue([])
    const widgetSpy = vi
      .spyOn(coreIntelligenceApi, 'getIntelligenceWidgetSnapshot')
      .mockResolvedValue({
        generatedAt: '2026-04-17T09:45:00Z',
        dateRange: { start: '2026-03-17', end: '2026-04-17' },
        digestSummary: {
          dateRange: { start: '2026-03-17', end: '2026-04-17' },
          totalVisits: {
            value: 128,
            previousValue: 120,
            changePercent: 7,
            trend: 'up',
          },
          totalSearches: {
            value: 32,
            previousValue: 28,
            changePercent: 14,
            trend: 'up',
          },
          newDomains: {
            value: 9,
            previousValue: 8,
            changePercent: 13,
            trend: 'up',
          },
          deepReadPages: {
            value: 5,
            previousValue: 4,
            changePercent: 25,
            trend: 'up',
          },
          refindPages: {
            value: 3,
            previousValue: 2,
            changePercent: 50,
            trend: 'up',
          },
        },
        highlights: [],
        notes: [],
      })
    const localHostPreviewSpy = vi
      .spyOn(coreIntelligenceApi, 'previewIntelligenceLocalHost')
      .mockResolvedValue(createLocalHostPreview('en'))
    const publicSpy = vi
      .spyOn(coreIntelligenceApi, 'getIntelligencePublicSnapshot')
      .mockResolvedValue({
        generatedAt: '2026-04-17T09:45:00Z',
        dateRange: { start: '2026-03-17', end: '2026-04-17' },
        digestSummary: {
          dateRange: { start: '2026-03-17', end: '2026-04-17' },
          totalVisits: {
            value: 128,
            previousValue: 120,
            changePercent: 7,
            trend: 'up',
          },
          totalSearches: {
            value: 32,
            previousValue: 28,
            changePercent: 14,
            trend: 'up',
          },
          newDomains: {
            value: 9,
            previousValue: 8,
            changePercent: 13,
            trend: 'up',
          },
          deepReadPages: {
            value: 5,
            previousValue: 4,
            changePercent: 25,
            trend: 'up',
          },
          refindPages: {
            value: 3,
            previousValue: 2,
            changePercent: 50,
            trend: 'up',
          },
        },
        topDomains: [],
        searchEngines: [],
        discoveryTrend: {
          availableYears: [],
          points: [],
        },
        notes: [],
      })

    function ScopedSettingsHarness() {
      const [activeProfileId, setActiveProfileId] = useState<string | null>(
        'chrome:Default',
      )

      return (
        <MemoryRouter initialEntries={['/integrations']}>
          <I18nContext.Provider value={createI18nValue('en')}>
            <ProfileScopeContext.Provider
              value={{ activeProfileId, setActiveProfileId }}
            >
              <ShellDataContext.Provider
                value={createShellValue(snapshot, dashboard)}
              >
                <button
                  type="button"
                  onClick={() => setActiveProfileId('firefox:Research')}
                >
                  {settingsT('externalOutputsScopedTitle')}
                </button>
                <IntegrationsPage />
              </ShellDataContext.Provider>
            </ProfileScopeContext.Provider>
          </I18nContext.Provider>
        </MemoryRouter>
      )
    }

    render(<ScopedSettingsHarness />)

    await screen.findByTestId('settings-external-outputs')
    await waitFor(() => {
      expect(embedSpy).toHaveBeenLastCalledWith(
        expect.objectContaining({
          start: expect.any(String),
          end: expect.any(String),
        }),
        'chrome:Default',
        6,
      )
      expect(widgetSpy).toHaveBeenLastCalledWith(
        expect.any(Object),
        'chrome:Default',
        4,
      )
      expect(publicSpy).toHaveBeenLastCalledWith(
        expect.any(Object),
        'chrome:Default',
      )
      expect(localHostPreviewSpy).toHaveBeenLastCalledWith(
        expect.any(Object),
        'en',
        'chrome:Default',
      )
    })

    const initialRange = embedSpy.mock.calls.at(-1)?.[0]

    await user.click(
      screen.getByRole('button', {
        name: settingsT('externalOutputsScopedTitle'),
      }),
    )
    await waitFor(() => {
      expect(embedSpy).toHaveBeenLastCalledWith(
        expect.any(Object),
        'firefox:Research',
        6,
      )
      expect(widgetSpy).toHaveBeenLastCalledWith(
        expect.any(Object),
        'firefox:Research',
        4,
      )
      expect(publicSpy).toHaveBeenLastCalledWith(
        expect.any(Object),
        'firefox:Research',
      )
      expect(localHostPreviewSpy).toHaveBeenLastCalledWith(
        expect.any(Object),
        'en',
        'firefox:Research',
      )
    })

    await user.click(
      within(screen.getByTestId('settings-external-outputs')).getByRole(
        'button',
        { name: intelligenceT('rangeWeek') },
      ),
    )
    await waitFor(() => {
      const latestRange = embedSpy.mock.calls.at(-1)?.[0]
      expect(latestRange).toEqual(
        expect.objectContaining({
          start: expect.any(String),
          end: expect.any(String),
        }),
      )
      expect(latestRange).not.toEqual(initialRange)
      expect(localHostPreviewSpy).toHaveBeenLastCalledWith(
        expect.objectContaining({
          start: expect.any(String),
          end: expect.any(String),
        }),
        'en',
        'firefox:Research',
      )
    })
  })
})
