/**
 * @file derived-runtime-review.test.tsx
 * @description Render-level coverage for Settings derived-runtime review cards.
 * @module pages/settings
 *
 * ## Responsibilities
 * - Verify deterministic runtime notes are localized and module toggles stay wired.
 * - Keep Settings route tests from needing to mount the full derived-state page for card internals.
 *
 * ## Not responsible for
 * - Re-testing the Settings hook that mutates runtime state.
 * - Re-testing backend deterministic rebuild behavior.
 *
 * ## Dependencies
 * - Uses the shipped i18n provider and preview snapshot fixture.
 * - Uses MemoryRouter because review cards include Jobs/Audit links.
 *
 * ## Performance notes
 * - Uses a single runtime fixture so card rendering remains cheap.
 */

import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test, vi } from 'vitest'
import { mockSnapshot } from '../../lib/backend-preview-fixtures'
import { I18nProvider, createNamespaceTranslator } from '../../lib/i18n'
import type { IntelligenceRuntimeSnapshot } from '../../lib/types'
import { DerivedRuntimeReview } from './derived-runtime-review'

const commonT = createNamespaceTranslator('en', 'common')
const settingsT = createNamespaceTranslator('en', 'settings')

describe('DerivedRuntimeReview', () => {
  test('renders deterministic runtime notes and wires module toggles', async () => {
    const user = userEvent.setup()
    const onDeterministicModuleToggle = vi.fn().mockResolvedValue(undefined)

    render(
      <MemoryRouter>
        <I18nProvider>
          <DerivedRuntimeReview
            action={null}
            clearReport={null}
            dashboardRecentRun={null}
            intelligenceRuntime={runtimeFixture()}
            intelligenceRuntimeError={null}
            rebuildQueueReport={null}
            snapshot={snapshotFixture()}
            onCancelRuntimeJob={vi.fn()}
            onDeterministicModuleToggle={onDeterministicModuleToggle}
            onEnrichmentPluginToggle={vi.fn()}
            onRetryRuntimeJob={vi.fn()}
          />
        </I18nProvider>
      </MemoryRouter>,
    )

    expect(
      screen.getByText(
        'No visible visits remained for chrome:Default; cleared visit-derived facts.',
      ),
    ).toBeVisible()
    expect(
      screen.getByText('Manual full rebuild requested for daily rollups.'),
    ).toBeVisible()
    expect(
      screen.getByText(
        'Archive visibility regressed or source counters moved backwards for daily rollups.',
      ),
    ).toBeVisible()

    await user.click(screen.getAllByRole('button', { name: 'Disable' })[0])
    expect(onDeterministicModuleToggle).toHaveBeenCalledWith('sessions')
  })

  test('renders runtime-error, disabled module, extra module, and plugin fallbacks', () => {
    render(
      <MemoryRouter>
        <I18nProvider>
          <DerivedRuntimeReview
            action="Saving runtime preference"
            clearReport={null}
            dashboardRecentRun={null}
            intelligenceRuntime={{
              ...runtimeFixture(),
              queue: {
                queued: 0,
                running: 0,
                succeeded: 1,
                failed: 1,
                cancelled: 0,
                lastActivityAt: null,
              },
              modules: [
                {
                  moduleId: 'custom-module',
                  enabled: true,
                  version: 'diagnostic',
                  status: 'idle',
                  dependsOn: [],
                  derivedTables: [],
                  lastRunId: null,
                  lastBuiltAt: null,
                  lastInvalidatedAt: null,
                  staleReason: null,
                  notes: [],
                },
              ],
              plugins: [
                {
                  pluginId: 'custom-plugin',
                  sourceKind: 'external',
                  enabled: true,
                  storedRecords: 7,
                  queuedJobs: 0,
                  runningJobs: 0,
                  failedJobs: 0,
                  lastCompletedAt: null,
                  lastError: null,
                },
              ],
            }}
            intelligenceRuntimeError="Runtime bridge unavailable"
            rebuildQueueReport={null}
            snapshot={snapshotFixture({
              deterministicModules: [
                { id: 'sessions', enabled: false, version: 'ci-v1' },
              ],
              enrichmentPlugins: [
                { id: 'custom-plugin', enabled: true, version: 'diagnostic' },
              ],
            })}
            onCancelRuntimeJob={vi.fn()}
            onDeterministicModuleToggle={vi.fn()}
            onEnrichmentPluginToggle={vi.fn()}
            onRetryRuntimeJob={vi.fn()}
          />
        </I18nProvider>
      </MemoryRouter>,
    )

    expect(screen.getByText(settingsT('runtimeUnavailableTitle'))).toBeVisible()
    expect(screen.getByText('Runtime bridge unavailable')).toBeVisible()
    expect(
      screen.getByText(settingsT('deterministicModuleDisabled')),
    ).toBeVisible()
    expect(
      screen.getAllByRole('button', { name: settingsT('enablePlugin') })[0],
    ).toBeDisabled()
    expect(screen.getByText('custom-module')).toBeVisible()
    expect(
      screen.getByText(settingsT('deterministicModuleFallbackDescription')),
    ).toBeVisible()
    expect(screen.getByText('custom-plugin')).toBeVisible()
    expect(screen.getAllByText(commonT('notAvailable')).length).toBeGreaterThan(
      0,
    )
    expect(screen.getByText('Saving runtime preference')).toBeVisible()
  })

  test('renders raw runtime timestamps when date formatting cannot parse them', () => {
    render(
      <MemoryRouter>
        <I18nProvider>
          <DerivedRuntimeReview
            action={null}
            clearReport={null}
            dashboardRecentRun={null}
            intelligenceRuntime={{
              ...runtimeFixture(),
              modules: [
                {
                  ...runtimeFixture().modules[0],
                  lastBuiltAt: 'not-a-module-date',
                  staleReason: null,
                  notes: [],
                },
              ],
              plugins: [
                {
                  pluginId: 'readable-content-refetch',
                  sourceKind: 'network',
                  enabled: true,
                  storedRecords: 3,
                  queuedJobs: 0,
                  runningJobs: 0,
                  failedJobs: 0,
                  lastCompletedAt: 'not-a-plugin-date',
                  lastError: null,
                },
              ],
            }}
            intelligenceRuntimeError={null}
            rebuildQueueReport={null}
            snapshot={snapshotFixture({
              enrichmentPlugins: [
                {
                  id: 'readable-content-refetch',
                  enabled: true,
                  version: 'ci-v1',
                },
              ],
            })}
            onCancelRuntimeJob={vi.fn()}
            onDeterministicModuleToggle={vi.fn()}
            onEnrichmentPluginToggle={vi.fn()}
            onRetryRuntimeJob={vi.fn()}
          />
        </I18nProvider>
      </MemoryRouter>,
    )

    expect(screen.getByText('not-a-module-date')).toBeVisible()
    expect(screen.getByText('not-a-plugin-date')).toBeVisible()
  })

  test('wires readable-content controls when webpage body fetch is release-enabled', async () => {
    const user = userEvent.setup()
    const onEnrichmentPluginToggle = vi.fn()

    const { rerender } = render(
      <MemoryRouter>
        <I18nProvider>
          <DerivedRuntimeReview
            action={null}
            clearReport={null}
            dashboardRecentRun={null}
            intelligenceRuntime={{
              ...runtimeFixture(),
              plugins: [
                {
                  pluginId: 'readable-content-refetch',
                  sourceKind: 'network',
                  enabled: true,
                  storedRecords: 3,
                  queuedJobs: 0,
                  runningJobs: 0,
                  failedJobs: 0,
                  lastCompletedAt: null,
                  lastError: null,
                },
              ],
            }}
            intelligenceRuntimeError={null}
            readableContentAvailable
            rebuildQueueReport={null}
            snapshot={snapshotFixture({
              enrichmentPlugins: [
                {
                  id: 'readable-content-refetch',
                  enabled: true,
                  version: 'ci-v1',
                },
              ],
            })}
            onCancelRuntimeJob={vi.fn()}
            onDeterministicModuleToggle={vi.fn()}
            onEnrichmentPluginToggle={onEnrichmentPluginToggle}
            onRetryRuntimeJob={vi.fn()}
          />
        </I18nProvider>
      </MemoryRouter>,
    )

    const readableContentRow = screen
      .getByText('Readable content fetcher')
      .closest('.result-row')
    expect(readableContentRow).toBeInstanceOf(HTMLElement)
    if (!(readableContentRow instanceof HTMLElement)) {
      throw new Error('expected readable content row')
    }
    expect(
      within(readableContentRow).getByText(settingsT('enabled')),
    ).toBeVisible()

    await user.click(
      within(readableContentRow).getByRole('button', { name: 'Disable' }),
    )

    expect(onEnrichmentPluginToggle).toHaveBeenCalledWith(
      'readable-content-refetch',
    )

    rerender(
      <MemoryRouter>
        <I18nProvider>
          <DerivedRuntimeReview
            action={null}
            clearReport={null}
            dashboardRecentRun={null}
            intelligenceRuntime={{
              ...runtimeFixture(),
              plugins: [
                {
                  pluginId: 'readable-content-refetch',
                  sourceKind: 'network',
                  enabled: false,
                  storedRecords: 0,
                  queuedJobs: 0,
                  runningJobs: 0,
                  failedJobs: 0,
                  lastCompletedAt: null,
                  lastError: null,
                },
              ],
            }}
            intelligenceRuntimeError={null}
            readableContentAvailable
            rebuildQueueReport={null}
            snapshot={snapshotFixture({
              enrichmentPlugins: [
                {
                  id: 'readable-content-refetch',
                  enabled: false,
                  version: 'ci-v1',
                },
              ],
            })}
            onCancelRuntimeJob={vi.fn()}
            onDeterministicModuleToggle={vi.fn()}
            onEnrichmentPluginToggle={onEnrichmentPluginToggle}
            onRetryRuntimeJob={vi.fn()}
          />
        </I18nProvider>
      </MemoryRouter>,
    )

    const disabledReadableContentRow = screen
      .getByText('Readable content fetcher')
      .closest('.result-row')
    expect(disabledReadableContentRow).toBeInstanceOf(HTMLElement)
    if (!(disabledReadableContentRow instanceof HTMLElement)) {
      throw new Error('expected disabled readable content row')
    }
    expect(
      within(disabledReadableContentRow).getByText(settingsT('disabled')),
    ).toBeVisible()
    expect(
      within(disabledReadableContentRow).getByRole('button', {
        name: settingsT('enablePlugin'),
      }),
    ).toBeVisible()
  })

  test('shows the deferred placeholder when readable content is not release-available', () => {
    render(
      <MemoryRouter>
        <I18nProvider>
          <DerivedRuntimeReview
            action={null}
            clearReport={null}
            dashboardRecentRun={null}
            intelligenceRuntime={{
              ...runtimeFixture(),
              plugins: [
                {
                  pluginId: 'readable-content-refetch',
                  sourceKind: 'network',
                  enabled: true,
                  storedRecords: 3,
                  queuedJobs: 0,
                  runningJobs: 0,
                  failedJobs: 0,
                  lastCompletedAt: null,
                  lastError: null,
                },
              ],
            }}
            intelligenceRuntimeError={null}
            readableContentAvailable={false}
            rebuildQueueReport={null}
            snapshot={snapshotFixture({
              enrichmentPlugins: [
                {
                  id: 'readable-content-refetch',
                  enabled: false,
                  version: 'ci-v1',
                },
              ],
            })}
            onCancelRuntimeJob={vi.fn()}
            onDeterministicModuleToggle={vi.fn()}
            onEnrichmentPluginToggle={vi.fn()}
            onRetryRuntimeJob={vi.fn()}
          />
        </I18nProvider>
      </MemoryRouter>,
    )

    const deferredRow = screen
      .getByText('Readable content fetcher')
      .closest('.result-row')
    expect(deferredRow).toBeInstanceOf(HTMLElement)
    if (!(deferredRow instanceof HTMLElement)) {
      throw new Error('expected deferred readable content row')
    }
    // Deferred branch: placeholder badge + a disabled enable button.
    expect(
      within(deferredRow).getAllByText(
        settingsT('readableContentDeferredBadge'),
      ).length,
    ).toBeGreaterThan(0)
    expect(
      within(deferredRow).getByRole('button', {
        name: settingsT('enablePlugin'),
      }),
    ).toBeDisabled()
  })
})

function snapshotFixture({
  deterministicModules = [{ id: 'sessions', enabled: true, version: 'ci-v1' }],
  enrichmentPlugins,
}: {
  deterministicModules?: Array<{
    id: string
    enabled: boolean
    version: string
  }>
  enrichmentPlugins?: Array<{ id: string; enabled: boolean; version: string }>
} = {}) {
  const snapshot = structuredClone(mockSnapshot)
  return {
    ...snapshot,
    config: {
      ...snapshot.config,
      deterministic: {
        modules: deterministicModules,
      },
      enrichment: enrichmentPlugins
        ? {
            plugins: enrichmentPlugins,
          }
        : snapshot.config.enrichment,
    },
  }
}

function runtimeFixture(): IntelligenceRuntimeSnapshot {
  return {
    queue: {
      queued: 0,
      running: 0,
      succeeded: 1,
      failed: 0,
      cancelled: 0,
      lastActivityAt: '2026-04-25T12:00:00Z',
    },
    plugins: [],
    modules: [
      {
        moduleId: 'sessions',
        enabled: true,
        version: 'ci-v1',
        status: 'stale',
        dependsOn: ['visit-derived-facts'],
        derivedTables: ['sessions'],
        lastRunId: 10,
        lastBuiltAt: '2026-04-25T11:00:00Z',
        lastInvalidatedAt: null,
        staleReason:
          'No visible visits remained for chrome:Default; cleared visit-derived facts.',
        notes: [
          'Manual full rebuild requested for daily rollups.',
          'Archive visibility regressed or source counters moved backwards for daily rollups.',
        ],
      },
    ],
    recentJobs: [],
    notes: [],
  }
}
