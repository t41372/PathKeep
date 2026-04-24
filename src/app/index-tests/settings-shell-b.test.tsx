/**
 * @file settings-shell-b.test.tsx
 * @description Split app-shell settings tests for analytics consent and updater recovery flows.
 * @module app/index-tests
 *
 * ## Responsibilities
 * - Preserve the original app-shell route contract for the settings analytics consent save flow.
 * - Preserve the original updater review and recovery assertions from `src/app/index.test.tsx`.
 * - Reuse the shared shell-test helpers instead of introducing parallel harness logic.
 *
 * ## Not responsible for
 * - Does not redefine shared seeding, DOM narrowing, or reset helpers.
 * - Does not expand the settings route coverage beyond the two owned tests in this slice.
 * - Does not change application code, route wiring, or source test behavior outside this file.
 *
 * ## Dependencies
 * - Depends on `App`, `appRoutes`, and the shipped settings route contract.
 * - Reuses `src/app/index-tests/test-helpers.tsx` for canonical shell-test setup.
 * - Mocks updater helpers through `src/lib/update` exactly like the original mega-suite.
 *
 * ## Performance notes
 * - Seeds only the archive state needed by these settings flows so the split suite stays lightweight.
 */

import { beforeEach, describe, expect, test, vi } from 'vitest'
import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter } from 'react-router-dom'
import App from '../index'
import { appRoutes } from '../router'
import { backend } from '../../lib/backend-client'
import * as updateLib from '../../lib/update'
import {
  expectHtmlElement,
  resetAppShellHarness,
  seedArchiveRun,
  settingsT,
} from './test-helpers'

describe('App shell', () => {
  beforeEach(() => {
    resetAppShellHarness()
  })

  test('saves analytics consent in settings and runs updater review from maintenance', async () => {
    await seedArchiveRun()
    const user = userEvent.setup()
    const saveConfigSpy = vi.spyOn(backend, 'saveConfig')
    const checkForAppUpdateSpy = vi
      .spyOn(updateLib, 'checkForAppUpdate')
      .mockResolvedValue({
        availability: {
          supported: true,
          checkedAt: '2026-04-10T00:00:00Z',
          available: true,
          currentVersion: '0.1.0',
          version: '0.2.0',
          notes: 'Updater wiring is ready.',
          publishedAt: '2026-04-10T00:00:00Z',
          error: null,
          downloadUrl: updateLib.RELEASES_PAGE_URL,
        },
        pendingUpdate: {
          currentVersion: '0.1.0',
          version: '0.2.0',
          notes: 'Updater wiring is ready.',
          publishedAt: '2026-04-10T00:00:00Z',
          downloadUrl: updateLib.RELEASES_PAGE_URL,
        },
      })
    const downloadAndInstallSpy = vi
      .spyOn(updateLib, 'downloadAndInstallAppUpdate')
      .mockResolvedValue({
        phase: 'installed',
        downloadedBytes: 128,
        contentLength: null,
        message: 'Installed',
      } as Awaited<ReturnType<typeof updateLib.downloadAndInstallAppUpdate>>)

    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/settings'],
    })

    render(<App router={router} />)

    const settingsPage = await screen.findByTestId('settings-page')
    const analyticsPanel = expectHtmlElement(
      document.getElementById('settings-analytics'),
    )
    await user.click(
      within(analyticsPanel).getByRole('checkbox', {
        name: settingsT('analyticsEnabled'),
      }),
    )
    await user.click(
      within(analyticsPanel).getByRole('button', {
        name: settingsT('analyticsSave'),
      }),
    )

    const maintenanceLink = expectHtmlElement(
      within(settingsPage)
        .getAllByRole('link', {
          name: new RegExp(settingsT('openMaintenance')),
        })
        .find((link) => link.getAttribute('href') === '/maintenance') ?? null,
    )
    await user.click(maintenanceLink)

    await screen.findByTestId('maintenance-page')
    const updatePanel = expectHtmlElement(
      document.getElementById('settings-updater'),
    )

    await waitFor(() => {
      expect(saveConfigSpy).toHaveBeenCalled()
    })
    expect(saveConfigSpy).toHaveBeenLastCalledWith(
      expect.objectContaining({
        analytics: expect.objectContaining({
          enabled: true,
          consentGrantedAt: expect.any(String),
        }),
      }),
    )

    await user.click(
      within(updatePanel).getByRole('button', {
        name: settingsT('updateCheckNow'),
      }),
    )

    await waitFor(() => {
      expect(checkForAppUpdateSpy).toHaveBeenCalledWith('0.1.0')
    })
    expect(
      within(updatePanel).getByText(settingsT('updateReleaseNotes')),
    ).toBeVisible()
    expect(
      within(updatePanel).getByText('Updater wiring is ready.'),
    ).toBeVisible()

    await user.click(
      within(updatePanel).getByRole('button', {
        name: settingsT('updateDownloadAndInstall'),
      }),
    )

    await waitFor(() => {
      expect(downloadAndInstallSpy).toHaveBeenCalledTimes(1)
    })
  })

  test('recovers the updater panel when check now fails', async () => {
    await seedArchiveRun()
    const user = userEvent.setup()
    vi.spyOn(updateLib, 'checkForAppUpdate').mockRejectedValue(
      new Error('Bridge disconnected'),
    )

    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/maintenance'],
    })

    render(<App router={router} />)

    await screen.findByTestId('maintenance-page')
    const updatePanel = expectHtmlElement(
      document.getElementById('settings-updater'),
    )
    const checkButton = within(updatePanel).getByRole('button', {
      name: settingsT('updateCheckNow'),
    })

    await user.click(checkButton)

    expect(
      await within(updatePanel).findByText('Bridge disconnected'),
    ).toBeVisible()
    await waitFor(() => expect(checkButton).toBeEnabled())
  })
})
