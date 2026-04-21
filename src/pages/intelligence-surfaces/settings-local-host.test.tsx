/**
 * @file settings-local-host.test.tsx
 * @description Protects the trusted local-host review and build flow assertions after the mega-suite split.
 * @module pages/intelligence-surfaces
 *
 * ## Responsibilities
 * - Preserve the shipped trusted local-host build/open flow assertions from the mega-suite.
 * - Verify non-English manual-review copy stays localized on the Settings external-output surface.
 * - Reuse the shared Intelligence surface harness and local-host preview fixture so split suites stay aligned.
 *
 * ## Non-Responsibilities
 * - Does not own the broader Settings external-output tab switching and scope refetch tests.
 * - Does not redefine shared archive seeding, shell context, or route render helpers.
 *
 * ## Dependencies
 * - Depends on Settings route rendering, shared test helpers, and the canonical local-host fixture builder.
 * - Uses backend open/build spies to keep the trusted local-host contract honest.
 *
 * ## Performance Notes
 * - Uses deterministic fixture payloads only and keeps setup bounded to one seeded archive per test.
 */

import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import * as coreIntelligenceApi from '../../lib/core-intelligence/api'
import type { IntelligenceLocalHostBuildResult } from '../../lib/core-intelligence/types'
import { createNamespaceTranslator } from '../../lib/i18n'
import { SettingsPage } from '../settings'
import { createLocalHostPreview } from './local-host-fixtures'
import {
  createEmptyRuntimeSnapshot,
  renderSurface,
  resetIntelligenceSurfaceHarness,
  seedArchiveState,
} from './test-helpers'

describe('intelligence surfaces', () => {
  beforeEach(() => {
    resetIntelligenceSurfaceHarness()
  })

  test('builds the trusted local host and exposes verify/open actions in settings', async () => {
    const user = userEvent.setup()
    const { snapshot, dashboard } = await seedArchiveState()
    const settingsT = createNamespaceTranslator('en', 'settings')
    const commonT = createNamespaceTranslator('en', 'common')
    const previewPayload = createLocalHostPreview('en')
    const builtPayload: IntelligenceLocalHostBuildResult = {
      ...createLocalHostPreview('en'),
      installedHost: {
        artifactRoot: previewPayload.artifactRoot,
        entryFilePath: previewPayload.entryFilePath,
        bundle: previewPayload.bundle,
      },
    }

    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
      createEmptyRuntimeSnapshot(),
    )
    vi.spyOn(
      coreIntelligenceApi,
      'getIntelligenceEmbedCards',
    ).mockResolvedValue([])
    vi.spyOn(
      coreIntelligenceApi,
      'getIntelligenceWidgetSnapshot',
    ).mockResolvedValue({
      generatedAt: '2026-04-17T09:45:00Z',
      dateRange: { start: '2026-03-17', end: '2026-04-17' },
      digestSummary: {
        dateRange: { start: '2026-03-17', end: '2026-04-17' },
        totalVisits: { value: 0, trend: 'flat' },
        totalSearches: { value: 0, trend: 'flat' },
        newDomains: { value: 0, trend: 'flat' },
        deepReadPages: { value: 0, trend: 'flat' },
        refindPages: { value: 0, trend: 'flat' },
      },
      highlights: [],
      notes: [],
    })
    vi.spyOn(
      coreIntelligenceApi,
      'getIntelligencePublicSnapshot',
    ).mockResolvedValue({
      generatedAt: '2026-04-17T09:45:00Z',
      dateRange: { start: '2026-03-17', end: '2026-04-17' },
      digestSummary: {
        dateRange: { start: '2026-03-17', end: '2026-04-17' },
        totalVisits: { value: 0, trend: 'flat' },
        totalSearches: { value: 0, trend: 'flat' },
        newDomains: { value: 0, trend: 'flat' },
        deepReadPages: { value: 0, trend: 'flat' },
        refindPages: { value: 0, trend: 'flat' },
      },
      topDomains: [],
      searchEngines: [],
      discoveryTrend: { points: [], availableYears: [] },
      notes: [],
    })
    const previewSpy = vi
      .spyOn(coreIntelligenceApi, 'previewIntelligenceLocalHost')
      .mockResolvedValue(previewPayload)
    const buildSpy = vi
      .spyOn(coreIntelligenceApi, 'buildIntelligenceLocalHost')
      .mockResolvedValue(builtPayload)
    const openExternalUrlSpy = vi
      .spyOn(backend, 'openExternalUrl')
      .mockResolvedValue('file:///tmp/pathkeep/index.html')
    const openPathSpy = vi
      .spyOn(backend, 'openPathInFileManager')
      .mockResolvedValue(previewPayload.artifactRoot)

    renderSurface(<SettingsPage />, {
      dashboard,
      language: 'en',
      route: '/settings',
      snapshot,
    })

    const panel = await screen.findByTestId('settings-external-outputs')
    await within(panel).findByText(
      settingsT('externalOutputsLocalHostSummaryTitle'),
    )

    await user.click(
      within(panel).getByRole('button', {
        name: settingsT('externalOutputsLocalHostCreateAction'),
      }),
    )

    await waitFor(() => {
      expect(buildSpy).toHaveBeenCalledWith(
        expect.objectContaining({
          start: expect.any(String),
          end: expect.any(String),
        }),
        'en',
        null,
      )
    })
    expect(
      await within(panel).findByText(
        settingsT('externalOutputsLocalHostBuilt'),
      ),
    ).toBeVisible()
    expect(
      within(panel).getByRole('button', {
        name: settingsT('externalOutputsLocalHostOpenAction'),
      }),
    ).toBeVisible()
    expect(
      within(panel).getByRole('button', { name: settingsT('openDirectory') }),
    ).toBeVisible()
    expect(
      within(panel).getAllByRole('button', { name: commonT('copyAction') })
        .length,
    ).toBeGreaterThan(0)

    await user.click(
      within(panel).getByRole('button', {
        name: settingsT('externalOutputsLocalHostOpenAction'),
      }),
    )
    expect(openExternalUrlSpy).toHaveBeenCalledWith(
      `file://${encodeURI(builtPayload.installedHost!.entryFilePath)}`,
    )

    await user.click(
      within(panel).getByRole('button', { name: settingsT('openDirectory') }),
    )
    expect(openPathSpy).toHaveBeenCalledWith(
      builtPayload.installedHost!.artifactRoot,
    )
    expect(previewSpy).toHaveBeenCalled()
  })

  test('localizes trusted local-host manual review copy in non-English settings surfaces', async () => {
    const { snapshot, dashboard } = await seedArchiveState()
    const settingsT = createNamespaceTranslator('zh-TW', 'settings')

    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
      createEmptyRuntimeSnapshot(),
    )
    vi.spyOn(
      coreIntelligenceApi,
      'getIntelligenceEmbedCards',
    ).mockResolvedValue([])
    vi.spyOn(
      coreIntelligenceApi,
      'getIntelligenceWidgetSnapshot',
    ).mockResolvedValue({
      generatedAt: '2026-04-17T09:45:00Z',
      dateRange: { start: '2026-03-17', end: '2026-04-17' },
      digestSummary: {
        dateRange: { start: '2026-03-17', end: '2026-04-17' },
        totalVisits: { value: 0, trend: 'flat' },
        totalSearches: { value: 0, trend: 'flat' },
        newDomains: { value: 0, trend: 'flat' },
        deepReadPages: { value: 0, trend: 'flat' },
        refindPages: { value: 0, trend: 'flat' },
      },
      highlights: [],
      notes: [],
    })
    vi.spyOn(
      coreIntelligenceApi,
      'getIntelligencePublicSnapshot',
    ).mockResolvedValue({
      generatedAt: '2026-04-17T09:45:00Z',
      dateRange: { start: '2026-03-17', end: '2026-04-17' },
      digestSummary: {
        dateRange: { start: '2026-03-17', end: '2026-04-17' },
        totalVisits: { value: 0, trend: 'flat' },
        totalSearches: { value: 0, trend: 'flat' },
        newDomains: { value: 0, trend: 'flat' },
        deepReadPages: { value: 0, trend: 'flat' },
        refindPages: { value: 0, trend: 'flat' },
      },
      topDomains: [],
      searchEngines: [],
      discoveryTrend: { points: [], availableYears: [] },
      notes: [],
    })
    vi.spyOn(
      coreIntelligenceApi,
      'previewIntelligenceLocalHost',
    ).mockResolvedValue(createLocalHostPreview('en'))

    renderSurface(<SettingsPage />, {
      dashboard,
      language: 'zh-TW',
      route: '/settings',
      snapshot,
    })

    const panel = await screen.findByTestId('settings-external-outputs')
    expect(
      await within(panel).findByText(
        settingsT('externalOutputsLocalHostManualReview'),
      ),
    ).toBeVisible()
    expect(
      within(panel).getByText(
        settingsT('externalOutputsLocalHostPurposeEntry'),
      ),
    ).toBeVisible()
    expect(
      within(panel).queryByText(
        'Review index.html and bundle.json before handing this folder to another trusted local tool.',
      ),
    ).not.toBeInTheDocument()
  })
})
