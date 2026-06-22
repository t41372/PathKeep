/**
 * @file settings-core-sections.test.tsx
 * @description Protects the shipped Settings core-section route behavior after splitting the intelligence surface mega-suite.
 * @module pages/intelligence-surfaces
 *
 * ## Responsibilities
 * - Preserve the unlock recovery state assertions for the Settings route.
 * - Keep the localized Settings group dividers under regression coverage.
 * - Verify the extracted general section still stays wired to the route owner.
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

  test('shows the generic unavailable state when settings support probes fail without an unlock signal', async () => {
    const { snapshot } = await seedArchiveState()
    const settingsT = createNamespaceTranslator('en', 'settings')
    vi.spyOn(backend, 'scheduleStatus').mockRejectedValue(
      new Error('schedule unavailable'),
    )
    vi.spyOn(backend, 'securityStatus').mockRejectedValue(
      new Error('security unavailable'),
    )

    renderSurface(<SettingsPage />, {
      snapshot,
      shellValue: {
        ...createShellValue(snapshot),
        snapshot: null,
      },
    })

    expect(
      await screen.findByRole('heading', {
        name: settingsT('unavailableTitle'),
      }),
    ).toBeVisible()
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
    expect(screen.getByText(settingsT('groupPrivacyAccess'))).toBeVisible()
    expect(screen.getByText(settingsT('groupIntelligence'))).toBeVisible()
    expect(screen.getByText(settingsT('groupBackupSync'))).toBeVisible()
    expect(screen.queryByText(/^CORE$/)).not.toBeInTheDocument()
    expect(screen.queryByText(/^DATA & UPDATES$/)).not.toBeInTheDocument()
  })

  test('keeps settings preference-only and links advanced workflows out to their owners', async () => {
    const { snapshot, dashboard } = await seedArchiveState()
    const settingsT = createNamespaceTranslator('en', 'settings')

    renderSurface(<SettingsPage />, {
      dashboard,
      language: 'en',
      route: '/settings',
      snapshot,
    })

    await screen.findByTestId('settings-page')
    expect(document.getElementById('settings-updater')).not.toBeInTheDocument()
    expect(
      document.getElementById('settings-retention'),
    ).not.toBeInTheDocument()
    expect(document.getElementById('settings-derived')).not.toBeInTheDocument()
    expect(
      document.getElementById('settings-external-outputs'),
    ).not.toBeInTheDocument()
    const migrationPanel = document.getElementById('settings-migration')
    if (!(migrationPanel instanceof HTMLElement)) {
      throw new Error('expected settings data migration panel')
    }
    expect(
      within(migrationPanel).getByTestId('settings-migration-export'),
    ).toBeVisible()
    expect(
      within(migrationPanel).getByTestId('settings-migration-import'),
    ).toBeVisible()
    expect(
      screen
        .getAllByRole('link', {
          name: new RegExp(settingsT('openMaintenance')),
        })
        .some((link) => link.getAttribute('href') === '/maintenance'),
    ).toBe(true)
    expect(
      screen
        .getAllByRole('link', {
          name: new RegExp(settingsT('openIntegrations')),
        })
        .some((link) => link.getAttribute('href') === '/integrations'),
    ).toBe(true)
  })

  test('renders settings nav anchors and keeps extracted general section actions wired to the route owner', async () => {
    const user = userEvent.setup()
    const { snapshot, dashboard } = await seedArchiveState()
    const commonT = createNamespaceTranslator('en', 'common')
    const navigationT = createNamespaceTranslator('en', 'navigation')
    const settingsT = createNamespaceTranslator('en', 'settings')

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
    const generalNavLink = within(nav).getByRole('link', {
      name: settingsT('general'),
    })
    const profilesNavLink = within(nav).getByRole('link', {
      name: settingsT('browserProfiles'),
    })
    const migrationNavLink = within(nav).getByRole('link', {
      name: settingsT('migrationTitle'),
    })
    expect(generalNavLink).toHaveAttribute(
      'href',
      '#/settings#settings-general',
    )
    expect(profilesNavLink).toHaveAttribute(
      'href',
      '#/settings#settings-profiles',
    )
    expect(migrationNavLink).toHaveAttribute(
      'href',
      '#/settings#settings-migration',
    )

    const migrationPanel = document.getElementById('settings-migration')
    if (!(migrationPanel instanceof HTMLElement)) {
      throw new Error('expected settings migration panel')
    }
    const scrollDoubles = installImmediateSectionScrollDoubles()
    try {
      await user.click(migrationNavLink)
      await waitFor(() =>
        expect(scrollDoubles.scrollIntoView).toHaveBeenCalledWith({
          behavior: 'smooth',
          block: 'start',
        }),
      )
      expect(migrationPanel).toHaveAttribute('tabindex', '-1')
      expect(scrollDoubles.focus).toHaveBeenCalled()
    } finally {
      scrollDoubles.restore()
    }

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
    expect(screen.queryByText(commonT('rescanAction'))).not.toBeInTheDocument()

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

  test('scrolls initial settings hash links after the target panel mounts', async () => {
    const { snapshot, dashboard } = await seedArchiveState()
    const scrollDoubles = installImmediateSectionScrollDoubles()

    try {
      renderSurface(<SettingsPage />, {
        dashboard,
        language: 'en',
        route: '/settings#settings-profiles',
        snapshot,
      })

      await screen.findByTestId('settings-page')
      await waitFor(() =>
        expect(scrollDoubles.scrollIntoView).toHaveBeenCalledWith({
          behavior: 'smooth',
          block: 'start',
        }),
      )
      expect(document.getElementById('settings-profiles')).toHaveAttribute(
        'tabindex',
        '-1',
      )
    } finally {
      scrollDoubles.restore()
    }
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
