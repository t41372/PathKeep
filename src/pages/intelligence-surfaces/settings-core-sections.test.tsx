/**
 * @file settings-core-sections.test.tsx
 * @description Protects the shipped Settings core-section route behavior after splitting the intelligence surface mega-suite.
 * @module pages/intelligence-surfaces
 *
 * ## Responsibilities
 * - Preserve the unlock recovery state assertions for the Settings route.
 * - Keep the localized Settings group dividers under regression coverage.
 * - Verify the extracted general and analytics sections still stay wired to the route owner.
 *
 * ## Non-Responsibilities
 * - Does not own runtime review, external outputs, or search-rule flows.
 * - Does not redefine the shared route render harness or archive seeding helpers.
 *
 * ## Dependencies
 * - Depends on the shared intelligence surface harness for seeded archive state and route rendering.
 * - Uses the shipped Settings page plus backend/runtime mocks to assert user-visible behavior.
 *
 * ## Performance Notes
 * - Reuses the shared seeded archive helpers so this split suite does not multiply setup work.
 */

import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { CONFIGURED_ANALYTICS_ENDPOINT } from '../../lib/analytics'
import { backend } from '../../lib/backend-client'
import { createNamespaceTranslator } from '../../lib/i18n'
import { SettingsPage } from '../settings'
import {
  createEmptyRuntimeSnapshot,
  createShellValue,
  renderSurface,
  resetIntelligenceSurfaceHarness,
  seedArchiveState,
} from './test-helpers'

describe('intelligence surfaces settings core sections', () => {
  beforeEach(() => {
    resetIntelligenceSurfaceHarness()
  })

  test('shows a security recovery empty state in settings when the archive needs unlocking', async () => {
    const { snapshot } = await seedArchiveState()
    const settingsT = createNamespaceTranslator('en', 'settings')
    const dashboardT = createNamespaceTranslator('en', 'dashboard')

    await backend.clearSessionDatabaseKey()

    renderSurface(<SettingsPage />, {
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
        name: settingsT('archiveUnlockTitle'),
      }),
    ).toBeVisible()
    expect(
      screen.getByRole('link', { name: dashboardT('reviewSecurity') }),
    ).toHaveAttribute('href', '/security')
  })

  test('localizes settings group dividers in zh-TW', async () => {
    const { snapshot, dashboard } = await seedArchiveState()
    const settingsT = createNamespaceTranslator('zh-TW', 'settings')

    renderSurface(<SettingsPage />, {
      dashboard,
      language: 'zh-TW',
      route: '/settings',
      snapshot,
    })

    expect(await screen.findByText(settingsT('groupCore'))).toBeVisible()
    expect(screen.getByText(settingsT('groupDataUpdates'))).toBeVisible()
    expect(screen.getByText(settingsT('groupSecurityAccess'))).toBeVisible()
    expect(screen.getByText(settingsT('groupIntelligence'))).toBeVisible()
    expect(screen.getByText(settingsT('groupBackupSync'))).toBeVisible()
    expect(screen.getByText(settingsT('groupPlatform'))).toBeVisible()
    expect(screen.queryByText(/^CORE$/)).not.toBeInTheDocument()
    expect(screen.queryByText(/^DATA & UPDATES$/)).not.toBeInTheDocument()
  })

  test('renders settings nav anchors and keeps extracted general section actions wired to the route owner', async () => {
    const user = userEvent.setup()
    const { snapshot, dashboard } = await seedArchiveState()
    const commonT = createNamespaceTranslator('en', 'common')
    const navigationT = createNamespaceTranslator('en', 'navigation')
    const settingsT = createNamespaceTranslator('en', 'settings')

    snapshot.runtimeDiagnostics.latestCrashReport = {
      source: 'frontend',
      recordedAt: '2026-04-18T10:15:00Z',
      fatal: false,
      message: 'Renderer stalled while collecting logs.',
      path: '/tmp/pathkeep/crash/frontend.log',
    }

    const openPathSpy = vi
      .spyOn(backend, 'openPathInFileManager')
      .mockResolvedValue('/tmp/pathkeep/crash/frontend.log')
    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
      createEmptyRuntimeSnapshot(),
    )
    const shellValue = createShellValue(snapshot, dashboard)
    shellValue.saveConfig = vi.fn().mockResolvedValue({
      ...snapshot,
      config: {
        ...snapshot.config,
        explorerBackgroundPrefetchPages: 0,
      },
    })
    shellValue.buildInfo = {
      productName: 'PathKeep',
      version: '0.9.0',
      gitCommitShort: 'abc1234',
      gitCommitFull: 'abc123456789',
      gitDirty: false,
    }

    renderSurface(<SettingsPage />, {
      dashboard,
      language: 'en',
      route: '/settings',
      shellValue,
      snapshot,
    })

    const nav = await screen.findByRole('navigation', {
      name: navigationT('settingsLabel'),
    })
    expect(
      within(nav).getByRole('link', { name: settingsT('general') }),
    ).toHaveAttribute('href', '#settings-general')
    expect(
      within(nav).getByRole('link', { name: settingsT('analyticsTitle') }),
    ).toHaveAttribute('href', '#settings-analytics')
    expect(
      within(nav).getByRole('link', {
        name: settingsT('platformTroubleshooting'),
      }),
    ).toHaveAttribute('href', '#settings-platform')

    const generalPanel = document.getElementById('settings-general')
    if (!(generalPanel instanceof HTMLElement)) {
      throw new Error('expected settings general panel')
    }

    expect(
      within(generalPanel).getByRole('combobox', {
        name: settingsT('interfaceLanguage'),
      }),
    ).toHaveValue(snapshot.config.preferredLanguage)
    expect(
      within(generalPanel).getByRole('combobox', {
        name: settingsT('explorerBackgroundPrefetchPages'),
      }),
    ).toHaveValue(String(snapshot.config.explorerBackgroundPrefetchPages))
    expect(within(generalPanel).getByText('0.9.0')).toBeVisible()
    expect(within(generalPanel).getByText('abc1234')).toBeVisible()
    expect(
      within(generalPanel).getByRole('button', {
        name: settingsT('openCrashReport'),
      }),
    ).toBeVisible()

    await user.click(
      within(generalPanel).getAllByRole('button', {
        name: commonT('copyAction'),
      })[0],
    )
    expect(
      await within(generalPanel).findByText(commonT('copiedNotice')),
    ).toBeVisible()

    await user.click(
      within(generalPanel).getByRole('button', {
        name: settingsT('openCrashReport'),
      }),
    )
    expect(openPathSpy).toHaveBeenLastCalledWith(
      '/tmp/pathkeep/crash/frontend.log',
    )

    await user.selectOptions(
      within(generalPanel).getByRole('combobox', {
        name: settingsT('explorerBackgroundPrefetchPages'),
      }),
      '0',
    )
    await waitFor(() =>
      expect(shellValue.saveConfig).toHaveBeenCalledWith(
        expect.objectContaining({
          explorerBackgroundPrefetchPages: 0,
        }),
      ),
    )
  })

  test('keeps extracted analytics section dirty-state and save behavior truthful', async () => {
    const user = userEvent.setup()
    const { snapshot, dashboard } = await seedArchiveState()
    const settingsT = createNamespaceTranslator('en', 'settings')

    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
      createEmptyRuntimeSnapshot(),
    )
    const shellValue = createShellValue(snapshot, dashboard)
    shellValue.saveConfig = vi.fn().mockResolvedValue({
      ...snapshot,
      config: {
        ...snapshot.config,
        analytics: {
          enabled: true,
          consentGrantedAt: '2026-04-20T18:20:00.000Z',
        },
      },
    })

    renderSurface(<SettingsPage />, {
      dashboard,
      language: 'en',
      route: '/settings',
      shellValue,
      snapshot,
    })

    const analyticsPanel = document.getElementById('settings-analytics')
    if (!(analyticsPanel instanceof HTMLElement)) {
      throw new Error('expected settings analytics panel')
    }

    const saveButton = within(analyticsPanel).getByRole('button', {
      name: settingsT('analyticsSave'),
    })
    expect(saveButton).toBeDisabled()
    if (CONFIGURED_ANALYTICS_ENDPOINT) {
      expect(
        within(analyticsPanel).queryByText(
          settingsT('analyticsEndpointMissingTitle'),
        ),
      ).not.toBeInTheDocument()
    } else {
      expect(
        within(analyticsPanel).getByText(
          settingsT('analyticsEndpointMissingTitle'),
        ),
      ).toBeVisible()
    }

    await user.click(
      within(analyticsPanel).getByRole('checkbox', {
        name: settingsT('analyticsEnabled'),
      }),
    )
    expect(saveButton).toBeEnabled()

    await user.click(saveButton)
    await waitFor(() => expect(shellValue.saveConfig).toHaveBeenCalledTimes(1))
    expect(shellValue.saveConfig).toHaveBeenCalledWith(
      expect.objectContaining({
        analytics: expect.objectContaining({
          enabled: true,
          consentGrantedAt: expect.any(String),
        }),
      }),
    )
  })
})
