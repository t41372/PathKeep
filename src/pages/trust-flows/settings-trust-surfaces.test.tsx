/**
 * @file settings-trust-surfaces.test.tsx
 * @description Protects Maintenance and Integrations trust-flow panels that gate cleanup and AI review copy.
 * @module pages/trust-flows
 *
 * ## Responsibilities
 * - Keep the retention prune review panel wired to the shipped preview and execute flow.
 * - Verify that non-English Integrations surfaces keep AI review copy localized instead of falling back to raw English.
 * - Reuse the shared trust-flow harness while preserving suite-local module boundaries.
 *
 * ## Non-Responsibilities
 * - Does not own the broader Settings route regression surface outside these two trust-flow cases.
 * - Does not redefine route-specific fixtures that already live in the shared trust-flow harness.
 * - Does not rewrite the original mega-suite; final cutover stays with the integrating owner.
 *
 * ## Dependencies
 * - Depends on `test-helpers.tsx` for the canonical trust-flow render/reset contract.
 * - Depends on the real Maintenance and Integrations routes so assertions keep protecting the shipped UI grammar.
 *
 * ## Performance Notes
 * - Reuses the initialized snapshot harness instead of rebuilding bespoke archive fixtures per assertion.
 */

import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { ShellDataContext } from '../../app/shell-data-context'
import { backend } from '../../lib/backend-client'
import { createNamespaceTranslator } from '../../lib/i18n'
import { I18nContext } from '../../lib/i18n/context'
import { ProfileScopeProvider } from '../../lib/profile-scope'
import { IntegrationsPage } from '../integrations'
import { MaintenancePage } from '../maintenance'
import {
  createI18nValue,
  createShellValue,
  renderTrustPage,
  resetTrustFlowHarness,
  seedInitializedSnapshot,
} from './test-helpers'

const { invoke, isTauri, subscribeToImportProgress } = vi.hoisted(() => ({
  invoke: vi.fn(),
  isTauri: vi.fn(() => false),
  subscribeToImportProgress: vi.fn(() => Promise.resolve(vi.fn())),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke,
  isTauri,
}))

vi.mock('../../lib/ipc/import-progress', () => ({
  subscribeToImportProgress,
}))

describe('Settings trust surfaces', () => {
  beforeEach(() => {
    resetTrustFlowHarness({
      invoke,
      isTauri,
      subscribeToImportProgress,
    })
  })

  test('renders the retention prune panel in maintenance and executes the selected cleanup', async () => {
    const user = userEvent.setup()
    const { snapshot, dashboard } = await seedInitializedSnapshot()
    const previewSpy = vi
      .spyOn(backend, 'previewRetentionPrune')
      .mockResolvedValue({
        buckets: [
          {
            id: 'snapshots',
            bytes: 2048,
            itemCount: 2,
            paths: [snapshot.directories.rawSnapshotsDir],
          },
          {
            id: 'exports',
            bytes: 0,
            itemCount: 0,
            paths: [snapshot.directories.exportsDir],
          },
        ],
        warnings: ['Snapshots stay local until you explicitly prune them.'],
      })
    const runSpy = vi.spyOn(backend, 'runRetentionPrune').mockResolvedValue({
      runId: 44,
      deletedBytes: 2048,
      deletedFiles: 2,
      buckets: [
        {
          id: 'snapshots',
          bytes: 2048,
          itemCount: 2,
          paths: [snapshot.directories.rawSnapshotsDir],
        },
      ],
      warnings: [],
    })
    const refreshSpy = vi.fn().mockResolvedValue(undefined)

    render(
      <MemoryRouter initialEntries={['/maintenance']}>
        <I18nContext.Provider value={createI18nValue('en')}>
          <ProfileScopeProvider>
            <ShellDataContext.Provider
              value={{
                ...createShellValue(snapshot, dashboard),
                refreshAppData: refreshSpy,
              }}
            >
              <MaintenancePage />
            </ShellDataContext.Provider>
          </ProfileScopeProvider>
        </I18nContext.Provider>
      </MemoryRouter>,
    )

    expect(document.getElementById('settings-retention')).toBeInstanceOf(
      HTMLElement,
    )
    expect(await screen.findByText(/Snapshots stay local/)).toBeVisible()

    await user.click(screen.getByRole('button', { name: 'Prune selected' }))

    await waitFor(() =>
      expect(runSpy).toHaveBeenCalledWith({ bucketIds: ['snapshots'] }),
    )
    expect(
      await screen.findByRole('link', { name: 'Open prune review' }),
    ).toHaveAttribute('href', '/audit?run=44')

    previewSpy.mockRestore()
    runSpy.mockRestore()
  })

  test('localizes AI integration review copy in non-English integrations surfaces', async () => {
    const { snapshot, dashboard } = await seedInitializedSnapshot()
    const settingsT = createNamespaceTranslator('zh-TW', 'settings')

    renderTrustPage(<IntegrationsPage />, {
      dashboard,
      language: 'zh-TW',
      route: '/integrations',
      snapshot,
    })

    expect(
      await screen.findByText(settingsT('aiIntegrationReview')),
    ).toBeVisible()
    expect(
      screen.getByText(settingsT('aiIntegrationManualEnable')),
    ).toBeVisible()
    expect(
      screen.getAllByText(settingsT('aiIntegrationGeneratedFileMcpPurpose'))
        .length,
    ).toBeGreaterThan(0)
    expect(
      screen.queryByText(
        'Enable MCP or Skill integration in Settings first. Both are off by default.',
      ),
    ).not.toBeInTheDocument()
  })
})
