/**
 * @file settings-runtime-and-search-rules.test.tsx
 * @description Protects the shipped Settings runtime-review and search-rule flows after splitting the intelligence surface mega-suite.
 * @module pages/intelligence-surfaces
 *
 * ## Responsibilities
 * - Preserve the first-party runtime review assertions and plugin-toggle save flow.
 * - Keep the derived-state search-rule editor and rebuild queue contract under regression coverage.
 *
 * ## Non-Responsibilities
 * - Does not own the Settings general, analytics, or external-outputs suites.
 * - Does not redefine shared archive seeding, i18n, or render helpers.
 *
 * ## Dependencies
 * - Depends on the shared intelligence surface harness plus Core Intelligence API mocks.
 * - Uses the shipped Settings route and typed runtime snapshot contracts.
 *
 * ## Performance Notes
 * - Reuses shared seeded state and only overrides the runtime payload needed by this suite.
 */

import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import * as coreIntelligenceApi from '../../lib/core-intelligence/api'
import { createNamespaceTranslator } from '../../lib/i18n'
import type { IntelligenceRuntimeSnapshot } from '../../lib/types'
import { MaintenancePage } from '../maintenance'
import {
  createShellValue,
  renderSurface,
  resetIntelligenceSurfaceHarness,
  seedArchiveState,
} from './test-helpers'

describe('intelligence surfaces settings runtime and search rules', () => {
  beforeEach(() => {
    resetIntelligenceSurfaceHarness()
  })

  test('renders settings enrichment runtime review and syncs plugin toggles', async () => {
    const user = userEvent.setup()
    const { snapshot, dashboard } = await seedArchiveState()
    const settingsT = createNamespaceTranslator('en', 'settings')
    const runtimeSnapshot: IntelligenceRuntimeSnapshot = {
      queue: {
        queued: 1,
        running: 0,
        succeeded: 1,
        failed: 1,
        cancelled: 0,
        lastActivityAt: '2026-04-10T16:30:00Z',
      },
      plugins: [
        {
          pluginId: 'title-normalization',
          sourceKind: 'local',
          enabled: true,
          storedRecords: 42,
          queuedJobs: 0,
          runningJobs: 0,
          failedJobs: 0,
          lastCompletedAt: '2026-04-10T16:20:00Z',
          lastError: null,
        },
        {
          pluginId: 'readable-content-refetch',
          sourceKind: 'network',
          enabled: true,
          storedRecords: 8,
          queuedJobs: 1,
          runningJobs: 0,
          failedJobs: 1,
          lastCompletedAt: '2026-04-10T15:40:00Z',
          lastError: '429 from upstream host',
        },
      ],
      modules: [
        {
          moduleId: 'search-trails',
          enabled: true,
          version: 'ci-v1',
          status: 'ready',
          dependsOn: ['visit-derived-facts', 'sessions'],
          derivedTables: [
            'search_trails',
            'search_trail_members',
            'search_events',
            'search_event_terms',
            'query_families',
          ],
          lastRunId: 12,
          lastBuiltAt: '2026-04-10T16:25:00Z',
          lastInvalidatedAt: null,
          staleReason: null,
          notes: [
            'Search trails and query families reflect the latest normalized visits.',
          ],
        },
        {
          moduleId: 'refind-pages',
          enabled: true,
          version: 'ci-v1',
          status: 'stale',
          dependsOn: ['visit-derived-facts', 'search-trails'],
          derivedTables: ['refind_pages', 'source_effectiveness'],
          lastRunId: 11,
          lastBuiltAt: '2026-04-09T16:25:00Z',
          lastInvalidatedAt: '2026-04-10T16:28:00Z',
          staleReason:
            'Visibility changed after the last deterministic rebuild.',
          notes: [
            'Manual rebuild required before refind pages and source effectiveness are fresh again.',
          ],
        },
      ],
      recentJobs: [
        {
          id: 411,
          jobType: 'enrichment-plugin',
          pluginId: 'readable-content-refetch',
          state: 'failed',
          historyId: 2,
          profileId: 'chrome:Default',
          url: 'https://example.com/article',
          title: 'Article',
          attempt: 2,
          createdAt: '2026-04-10T15:35:00Z',
          startedAt: '2026-04-10T15:36:00Z',
          finishedAt: '2026-04-10T15:37:00Z',
          updatedAt: '2026-04-10T15:37:00Z',
          heartbeatAt: null,
          progressLabel: null,
          progressDetail: null,
          progressCurrent: null,
          progressTotal: null,
          progressPercent: null,
          lastError: '429 from upstream host',
          retryable: true,
          cancellable: false,
        },
      ],
      notes: [
        'Browser preview mode shows a deterministic queue/runtime fixture.',
      ],
    }

    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
      runtimeSnapshot,
    )
    const shellValue = createShellValue(snapshot, dashboard)
    shellValue.saveConfig = vi.fn().mockResolvedValue(snapshot)

    renderSurface(<MaintenancePage />, {
      dashboard,
      language: 'en',
      route: '/maintenance',
      shellValue,
      snapshot,
    })

    expect(
      await screen.findByText(settingsT('firstPartyRuntimeTitle')),
    ).toBeVisible()
    expect(screen.getByText('Title normalization')).toBeVisible()
    expect(screen.getByText('Search trails')).toBeVisible()
    expect(screen.getByText('Refind pages')).toBeVisible()
    expect(screen.getAllByText('Page content fetcher').length).toBeGreaterThan(
      0,
    )
    expect(
      screen.getAllByText('1 queued / 0 running / 1 failed').length,
    ).toBeGreaterThan(0)

    const titleNormalizationRow = screen
      .getByText('Title normalization')
      .closest('.result-row')
    expect(titleNormalizationRow).not.toBeNull()
    if (!(titleNormalizationRow instanceof HTMLElement)) {
      throw new Error('expected title normalization row')
    }
    await user.click(
      within(titleNormalizationRow).getByRole('button', {
        name: settingsT('disablePlugin'),
      }),
    )

    await waitFor(() => expect(shellValue.saveConfig).toHaveBeenCalledTimes(1))
    expect(shellValue.saveConfig).toHaveBeenCalledWith(
      expect.objectContaining({
        enrichment: {
          plugins: expect.arrayContaining([
            expect.objectContaining({
              id: 'title-normalization',
              enabled: false,
            }),
          ]),
        },
        ai: expect.objectContaining({
          enrichmentPlugins: expect.arrayContaining([
            expect.objectContaining({
              pluginId: 'title-normalization',
              enabled: false,
            }),
          ]),
        }),
      }),
    )
  })

  test('renders settings search rules review and saves custom rules through the derived-state surface', async () => {
    const user = userEvent.setup()
    const { snapshot, dashboard } = await seedArchiveState()
    const settingsT = createNamespaceTranslator('en', 'settings')
    const queueSpy = vi
      .spyOn(coreIntelligenceApi, 'queueCoreIntelligenceRebuild')
      .mockResolvedValue({
        jobId: 77,
        state: 'queued',
        notes: ['Queued rebuild after search rule update.'],
      })

    renderSurface(<MaintenancePage />, {
      dashboard,
      language: 'en',
      route: '/maintenance',
      snapshot,
    })

    expect(
      await screen.findByRole('heading', {
        name: settingsT('searchRulesTitle'),
      }),
    ).toBeVisible()
    expect(screen.getByText('Docs Search')).toBeVisible()

    await user.click(
      screen.getByRole('button', { name: settingsT('searchRulesAdd') }),
    )
    const editor = await screen.findByRole('region', {
      name: settingsT('searchRulesEditorTitle'),
    })
    await user.type(
      within(editor).getByRole('textbox', {
        name: settingsT('searchRulesDisplayName'),
      }),
      'MDN Search',
    )
    await user.type(
      within(editor).getByRole('textbox', {
        name: settingsT('searchRulesEngineId'),
      }),
      'mdn-search',
    )
    await user.type(
      within(editor).getByRole('textbox', {
        name: settingsT('searchRulesHostPattern'),
      }),
      'developer.mozilla.org',
    )
    await user.type(
      within(editor).getByRole('textbox', {
        name: settingsT('searchRulesPathPrefix'),
      }),
      '/search',
    )
    await user.type(
      within(editor).getByRole('textbox', {
        name: settingsT('searchRulesQueryParam'),
      }),
      'q',
    )
    await user.click(
      within(editor).getByRole('button', {
        name: settingsT('searchRulesSave'),
      }),
    )

    await waitFor(() => expect(queueSpy).toHaveBeenCalledTimes(1))
    expect(await screen.findByText('MDN Search')).toBeVisible()

    const docsRuleRow = screen.getByText('Docs Search').closest('.result-row')
    expect(docsRuleRow).not.toBeNull()
    if (!(docsRuleRow instanceof HTMLElement)) {
      throw new Error('expected docs search rule row')
    }
    await user.click(
      within(docsRuleRow).getByRole('button', {
        name: settingsT('searchRulesDelete'),
      }),
    )

    await waitFor(() => expect(queueSpy).toHaveBeenCalledTimes(2))
    await waitFor(() => {
      expect(screen.queryByText('Docs Search')).not.toBeInTheDocument()
    })
  })
})
