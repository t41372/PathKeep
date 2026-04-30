/**
 * @file settings-shell-a.test.tsx
 * @description Settings-route slice of the original `src/app/index.test.tsx` shell suite.
 *
 * ## Responsibilities
 * - Preserve the app-shell Settings/Maintenance/Integrations assertions while extracting one reviewable slice out of the mega-suite.
 * - Cover crash diagnostics, remote backup PME, derived-state controls, and AI integration review boundaries on their canonical routes.
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
import { createNamespaceTranslator } from '../../lib/i18n'
import {
  commonT,
  expectHtmlElement,
  resetAppShellHarness,
  seedAiProviders,
  seedArchiveRun,
  settingsT,
} from './test-helpers'

const navigationT = createNamespaceTranslator('en', 'navigation')

describe('App shell', () => {
  beforeEach(() => {
    resetAppShellHarness()
  })

  test('shows crash diagnostics paths on the maintenance route', async () => {
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
      initialEntries: ['/maintenance'],
    })

    render(<App router={router} />)

    const page = await screen.findByTestId('maintenance-page')
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

  test('walks the maintenance remote backup PME and derived-state controls', async () => {
    await seedArchiveRun()
    const seededSnapshot = await backend.getAppSnapshot()
    await backend.saveConfig({
      ...seededSnapshot.config,
      remoteBackup: {
        ...seededSnapshot.config.remoteBackup,
        enabled: true,
        bucket: 'example-bucket',
      },
    })
    await backend.storeS3Credentials({
      accessKeyId: 'preview-key',
      secretAccessKey: 'preview-secret',
    })
    const user = userEvent.setup()
    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/maintenance'],
    })

    render(<App router={router} />)

    const maintenancePage = await screen.findByTestId('maintenance-page')
    const nav = within(maintenancePage).getByRole('navigation', {
      name: navigationT('maintenanceLabel'),
    })
    const remoteNavLink = within(nav).getByRole('link', {
      name: settingsT('remoteBackup'),
    })
    expect(remoteNavLink).toHaveAttribute(
      'href',
      '#/maintenance#settings-remote',
    )
    const remotePanel = expectHtmlElement(
      document.getElementById('settings-remote'),
    )
    const scrollDoubles = installImmediateSectionScrollDoubles()
    try {
      await user.click(remoteNavLink)
      await waitFor(() =>
        expect(scrollDoubles.scrollIntoView).toHaveBeenCalledWith({
          block: 'start',
        }),
      )
      expect(remotePanel).toHaveAttribute('tabindex', '-1')
      expect(scrollDoubles.focus).toHaveBeenCalled()
    } finally {
      scrollDoubles.restore()
    }
    const derivedPanel = expectHtmlElement(
      document.getElementById('settings-derived'),
    )
    expect(
      within(derivedPanel).getByText(settingsT('enrichmentDerivedState')),
    ).toBeVisible()
    expect(
      within(maintenancePage).getAllByText(settingsT('archiveDatabase')).length,
    ).toBeGreaterThan(0)
    expect(
      within(maintenancePage).getAllByText(settingsT('auditRepository')).length,
    ).toBeGreaterThan(0)
    expect(
      within(maintenancePage).getAllByText(settingsT('gitCommit')).length,
    ).toBeGreaterThan(0)
    expect(
      within(remotePanel).getByText(commonT('common.previewTab')),
    ).toBeVisible()
    expect(
      within(remotePanel).getByText(settingsT('remoteMaintenanceConfigTitle')),
    ).toBeVisible()
    expect(within(remotePanel).getByText('example-bucket')).toBeVisible()
    expect(
      within(remotePanel).getByRole('link', {
        name: settingsT('remoteMaintenanceEditSettings'),
      }),
    ).toHaveAttribute('href', '/settings#settings-remote')
    expect(
      within(remotePanel).queryByLabelText(settingsT('bucketLabel')),
    ).not.toBeInTheDocument()
    expect(
      within(remotePanel).queryByLabelText(settingsT('accessKeyId')),
    ).not.toBeInTheDocument()
    expect(
      within(remotePanel).queryByLabelText(settingsT('secretAccessKey')),
    ).not.toBeInTheDocument()
    expect(
      within(remotePanel).queryByRole('button', {
        name: settingsT('saveRemoteSettings'),
      }),
    ).not.toBeInTheDocument()

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

    expect(
      within(readableContentCard).getAllByText(
        settingsT('readableContentDeferredBadge'),
      ).length,
    ).toBeGreaterThan(0)
    expect(
      within(readableContentCard).getByRole('button', {
        name: settingsT('enablePlugin'),
      }),
    ).toBeDisabled()

    await user.click(
      within(maintenancePage).getByRole('button', {
        name: settingsT('clearDerivedState'),
      }),
    )
    await waitFor(() => {
      expect(
        within(maintenancePage).getByText(settingsT('clearCompletedTitle')),
      ).toBeVisible()
    })

    await user.click(
      within(maintenancePage).getByRole('button', {
        name: settingsT('rebuildDerivedState'),
      }),
    )
    await waitFor(() => {
      expect(
        within(maintenancePage).getByText(settingsT('rebuildQueuedTitle')),
      ).toBeVisible()
    })
    const runtimeQueueLinks = within(maintenancePage).getAllByRole('link', {
      name: settingsT('runtimeQueueTitle'),
    })
    expect(
      runtimeQueueLinks.some((link) => link.getAttribute('href') === '/jobs'),
    ).toBe(true)
  })

  test('scrolls initial maintenance hash links after panels mount', async () => {
    await seedArchiveRun()
    const scrollDoubles = installImmediateSectionScrollDoubles()
    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/maintenance#settings-derived'],
    })

    try {
      render(<App router={router} />)

      await screen.findByTestId('maintenance-page')
      await waitFor(() =>
        expect(scrollDoubles.scrollIntoView).toHaveBeenCalledWith({
          block: 'start',
        }),
      )
      expect(document.getElementById('settings-derived')).toHaveAttribute(
        'tabindex',
        '-1',
      )
    } finally {
      scrollDoubles.restore()
    }
  })

  test('keeps AI provider settings disabled for the v0.1 release', async () => {
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
      within(settingsPage)
        .getAllByText(settingsT('aiProvider'))
        .map((node) => node.closest('.panel'))
        .find((node): node is HTMLElement => node instanceof HTMLElement) ??
        null,
    )
    const enableAiButton = within(aiPanel).getByRole('button', {
      name: settingsT('aiMasterToggle'),
    })
    expect(enableAiButton).toBeDisabled()
    expect(
      within(aiPanel).getByText(settingsT('aiDeferredTitle')),
    ).toBeVisible()
    expect(within(aiPanel).queryByDisplayValue('Local LLM')).toBeNull()

    await user.click(enableAiButton)
    expect(saveConfigSpy).not.toHaveBeenCalled()
  })

  test('shows AI integration roadmap placeholders in integrations', async () => {
    await seedArchiveRun()
    await seedAiProviders()
    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/integrations'],
    })

    render(<App router={router} />)

    const integrationsPage = await screen.findByTestId('integrations-page')
    const aiPanel = expectHtmlElement(
      within(integrationsPage)
        .getByText(settingsT('aiIntegrationArtifactsTitle'))
        .closest('.panel'),
    )

    expect(
      await within(aiPanel).findByText(settingsT('aiIntegrationDeferredTitle')),
    ).toBeVisible()
    expect(
      within(aiPanel).getByText(settingsT('aiIntegrationDeferredBody')),
    ).toBeVisible()
    expect(
      within(aiPanel).getByText(settingsT('aiGeneratedFiles')),
    ).toBeVisible()
    expect(
      within(aiPanel).getByText(settingsT('aiIntegrationDeferredFilesBody')),
    ).toBeVisible()
    expect(within(aiPanel).queryByText(/"mcpServers"/)).toBeNull()
    expect(within(aiPanel).queryByText(/# PathKeep Search/)).toBeNull()
  })
})

function installImmediateSectionScrollDoubles() {
  const originalScrollIntoView = Reflect.get(
    Element.prototype,
    'scrollIntoView',
  )
  const originalRequestAnimationFrame = window.requestAnimationFrame
  const originalCancelAnimationFrame = window.cancelAnimationFrame
  const scrollIntoView = vi.fn()
  const focus = vi
    .spyOn(HTMLElement.prototype, 'focus')
    .mockImplementation(() => undefined)

  Element.prototype.scrollIntoView = scrollIntoView
  window.requestAnimationFrame = vi.fn((callback: FrameRequestCallback) => {
    callback(0)
    return 1
  })
  window.cancelAnimationFrame = vi.fn()

  return {
    focus,
    scrollIntoView,
    restore: () => {
      Element.prototype.scrollIntoView = originalScrollIntoView
      window.requestAnimationFrame = originalRequestAnimationFrame
      window.cancelAnimationFrame = originalCancelAnimationFrame
      focus.mockRestore()
    },
  }
}
