/**
 * @file settings-shell-a.test.tsx
 * @description Settings-route slice of the original `src/app/index.test.tsx` shell suite.
 *
 * ## Responsibilities
 * - Preserve the original `/settings` shell assertions while extracting one reviewable slice out of the mega-suite.
 * - Cover crash diagnostics, remote backup PME, derived-state controls, and AI integration review boundaries on the settings route.
 * - Reuse the shared shell-test helpers so split suites stay aligned with the canonical app-shell harness.
 *
 * ## Not responsible for
 * - Changing settings route contracts, test titles, or assertion semantics inherited from `src/app/index.test.tsx`.
 * - Introducing new helper abstractions beyond the existing shared `test-helpers` surface.
 * - Covering non-settings shell behavior; those assertions stay with other slices of the app-shell suite.
 *
 * ## Dependencies
 * - Depends on `App`, `appRoutes`, backend harness mutations, and `src/app/index-tests/test-helpers.tsx`.
 * - Uses Testing Library, Vitest, and the same memory-router setup as the source suite.
 *
 * ## Performance notes
 * - Reuses shared archive and AI-provider seed helpers so the split suite keeps the original bootstrap cost profile.
 */

import { beforeEach, describe, expect, test, vi } from 'vitest'
import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter } from 'react-router-dom'
import App from '../index'
import { appRoutes } from '../router'
import { backend } from '../../lib/backend-client'
import { backendTestHarness } from '../../lib/backend'
import {
  commonT,
  expectHtmlElement,
  resetAppShellHarness,
  seedAiProviders,
  seedArchiveRun,
  settingsT,
} from './test-helpers'

describe('App shell', () => {
  beforeEach(() => {
    resetAppShellHarness()
  })

  test('shows crash diagnostics paths on the settings route', async () => {
    await seedArchiveRun()
    backendTestHarness.mutateState((state) => {
      state.snapshot.runtimeDiagnostics.latestCrashReport = {
        source: 'rust-panic',
        recordedAt: '2026-04-10T12:34:00Z',
        fatal: true,
        message: 'panic in worker bridge',
        location: 'src-tauri/src/lib.rs:42',
        path: '/tmp/pathkeep-crash/rust-panic-latest.json',
      }
    })
    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/settings'],
    })

    render(<App router={router} />)

    const page = await screen.findByTestId('settings-page')
    expect(
      await within(page).findByText(settingsT('logsDirectory')),
    ).toBeVisible()
    expect(within(page).getByText(settingsT('crashReports'))).toBeVisible()
    expect(within(page).getByText(settingsT('latestCrashTitle'))).toBeVisible()
    expect(
      within(page).getByRole('button', {
        name: settingsT('openCrashReport'),
      }),
    ).toBeVisible()
  })

  test('walks the settings remote backup PME and derived-state controls', async () => {
    await seedArchiveRun()
    const user = userEvent.setup()
    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/settings'],
    })

    render(<App router={router} />)

    const settingsPage = await screen.findByTestId('settings-page')
    const remotePanel = expectHtmlElement(
      within(settingsPage)
        .getByText(settingsT('remoteBackup'))
        .closest('.panel'),
    )
    expect(
      within(settingsPage).getByText(settingsT('enrichmentDerivedState')),
    ).toBeVisible()
    expect(
      within(settingsPage).getByText(settingsT('externalOutputsTitle')),
    ).toBeVisible()
    expect(
      within(settingsPage).getByText(settingsT('archiveDatabase')),
    ).toBeVisible()
    expect(
      within(settingsPage).getByText(settingsT('auditRepository')),
    ).toBeVisible()
    expect(within(settingsPage).getByText(settingsT('gitCommit'))).toBeVisible()
    expect(
      within(remotePanel).getByText(commonT('common.previewTab')),
    ).toBeVisible()
    expect(
      within(settingsPage).getByRole('tab', {
        name: settingsT('externalOutputsTabPublic'),
      }),
    ).toBeVisible()

    await user.clear(
      within(remotePanel).getByLabelText(settingsT('bucketLabel')),
    )
    await user.type(
      within(remotePanel).getByLabelText(settingsT('bucketLabel')),
      'example-bucket',
    )
    await user.click(
      within(remotePanel).getByRole('button', {
        name: settingsT('saveRemoteSettings'),
      }),
    )

    await user.type(
      within(remotePanel).getByLabelText(settingsT('accessKeyId')),
      'preview-key',
    )
    await user.type(
      within(remotePanel).getByLabelText(settingsT('secretAccessKey')),
      'preview-secret',
    )
    await user.click(
      within(remotePanel).getByRole('button', {
        name: settingsT('storeRemoteCredentials'),
      }),
    )

    await waitFor(() => {
      expect(
        within(remotePanel).getByText(settingsT('credentialsSaved')),
      ).toBeVisible()
    })

    await user.click(
      within(remotePanel).getByRole('button', {
        name: settingsT('previewRemoteBackup'),
      }),
    )

    await waitFor(() => {
      expect(
        within(remotePanel).getByText(settingsT('bundlePath')),
      ).toBeVisible()
      expect(
        within(remotePanel).getAllByText(/pathkeep-remote-.*\.zip/).length,
      ).toBeGreaterThan(0)
    })

    await user.click(
      within(remotePanel).getByRole('button', {
        name: settingsT('executeRemoteBackup'),
      }),
    )

    await waitFor(() => {
      expect(
        within(remotePanel).getByText(
          'Browser preview mode simulated the upload and produced a local bundle for verification.',
        ),
      ).toBeVisible()
    })

    await waitFor(() => {
      expect(
        within(remotePanel).getByRole('button', {
          name: settingsT('verifyRemoteBackup'),
        }),
      ).toBeEnabled()
    })

    await user.click(
      within(remotePanel).getByRole('button', {
        name: settingsT('verifyRemoteBackup'),
      }),
    )

    await waitFor(() => {
      expect(
        within(remotePanel).getByText(settingsT('bundleVersion')),
      ).toBeVisible()
      expect(
        within(remotePanel).getByText('pathkeep.remote-backup.v1'),
      ).toBeVisible()
    })

    const readableContentCard = screen
      .getAllByText(settingsT('readableContentPlugin'))[0]
      .closest('.result-row')
    if (!(readableContentCard instanceof HTMLElement)) {
      throw new Error('Expected readable content plugin card to be present')
    }

    await user.click(
      within(readableContentCard).getByRole('button', {
        name: settingsT('disablePlugin'),
      }),
    )
    await waitFor(() => {
      expect(
        within(settingsPage).getByRole('button', {
          name: settingsT('enablePlugin'),
        }),
      ).toBeVisible()
    })

    await user.click(
      within(settingsPage).getByRole('button', {
        name: settingsT('clearDerivedState'),
      }),
    )
    await waitFor(() => {
      expect(
        within(settingsPage).getByText(settingsT('clearCompletedTitle')),
      ).toBeVisible()
    })

    await user.click(
      within(settingsPage).getByRole('button', {
        name: settingsT('rebuildDerivedState'),
      }),
    )
    await waitFor(() => {
      expect(
        within(settingsPage).getByText(settingsT('rebuildQueuedTitle')),
      ).toBeVisible()
    })
    expect(
      within(settingsPage).getByRole('link', {
        name: settingsT('runtimeQueueTitle'),
      }),
    ).toHaveAttribute('href', '/jobs')
  })

  test('keeps AI provider field edits local until save is confirmed', async () => {
    await seedArchiveRun()
    await seedAiProviders()
    const user = userEvent.setup()
    const saveConfigSpy = vi.spyOn(backend, 'saveConfig')
    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/settings'],
    })

    render(<App router={router} />)

    const settingsPage = await screen.findByTestId('settings-page')
    const aiPanel = expectHtmlElement(
      within(settingsPage).getByText(settingsT('aiProvider')).closest('.panel'),
    )
    const providerNameInput =
      within(settingsPage).getByDisplayValue('Local LLM')

    await user.clear(providerNameInput)
    await user.type(providerNameInput, 'Local LLM Draft')

    expect(saveConfigSpy).not.toHaveBeenCalled()
    expect(
      within(aiPanel).getByText(settingsT('aiUnsavedChanges')),
    ).toBeVisible()

    await user.click(
      within(aiPanel).getByRole('button', {
        name: settingsT('aiSaveConfig'),
      }),
    )

    await waitFor(() => {
      expect(saveConfigSpy).toHaveBeenCalledTimes(1)
    })
  })

  test('shows AI integration preview artifacts and consent boundaries in settings', async () => {
    await seedArchiveRun()
    await seedAiProviders()
    const user = userEvent.setup()
    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/settings'],
    })

    render(<App router={router} />)

    const settingsPage = await screen.findByTestId('settings-page')
    const aiPanel = expectHtmlElement(
      within(settingsPage).getByText(settingsT('aiProvider')).closest('.panel'),
    )

    expect(
      await within(aiPanel).findByText(settingsT('aiIntegrationReview')),
    ).toBeVisible()
    expect(
      within(aiPanel).getByText(settingsT('aiCapabilityNotes')),
    ).toBeVisible()
    expect(
      within(aiPanel).getByText(settingsT('aiGeneratedFiles')),
    ).toBeVisible()
    expect(
      within(aiPanel).getByRole('button', {
        name: 'integrations/pathkeep-mcp.json',
      }),
    ).toBeVisible()
    expect(within(aiPanel).getByText(/"mcpServers"/)).toBeVisible()

    await user.click(
      within(aiPanel).getByRole('button', {
        name: 'integrations/codex-pathkeep-skill/SKILL.md',
      }),
    )

    expect(within(aiPanel).getByText(/# PathKeep Search/)).toBeVisible()
  })
})
