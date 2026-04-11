/**
 * This test file protects the shared Topbar component contract.
 *
 * Why this file exists:
 * - Reusable shell components can create subtle regressions everywhere at once, so the tests here act as a front-end safety net.
 * - If the design or accessibility contract changes, these tests should tell the next reader exactly which promise moved.
 *
 * Main declarations:
 * - This file is mostly internal implementation detail.
 *
 * Source-of-truth notes:
 * - Shared shell components must stay aligned with `docs/design/screens-and-nav.md`, `docs/design/ux-principles.md`, and `docs/design/design-tokens.md`.
 * - Avoid locking tests to decorative markup when the actual contract is state visibility, routing, or accessible labeling.
 */

import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test, vi } from 'vitest'
import { onboardingScreen } from '../../app/router'
import { ShellDataProvider } from '../../app/shell-data'
import {
  ShellDataContext,
  type ShellDataContextValue,
} from '../../app/shell-data-context'
import { I18nProvider } from '../../lib/i18n'
import { ProfileScopeProvider } from '../../lib/profile-scope'
import { Topbar } from './index'

describe('Topbar', () => {
  test('renders the active screen metadata and shell actions', async () => {
    render(
      <I18nProvider>
        <ProfileScopeProvider>
          <ShellDataProvider>
            <MemoryRouter>
              <Topbar
                screen={{
                  ...onboardingScreen,
                  labelKey: 'navigation.dashboardLabel',
                  titleKey: 'navigation.dashboardTitle',
                  subtitleKey: 'navigation.dashboardSubtitle',
                }}
              />
            </MemoryRouter>
          </ShellDataProvider>
        </ProfileScopeProvider>
      </I18nProvider>,
    )

    expect(screen.getByRole('heading', { name: 'Dashboard' })).toBeVisible()
    expect(
      screen.getByRole('searchbox', { name: 'Search history' }),
    ).toBeVisible()
    expect(
      screen.getByRole('button', {
        name: 'Switch profile scope. Current: All profiles',
      }),
    ).toBeVisible()
    expect(screen.getByText('All profiles')).toBeVisible()
    expect(
      await screen.findByRole('button', { name: /Initialize first/ }),
    ).toBeVisible()
  })

  test('routes the primary shell action to security when the archive needs unlocking', () => {
    const shellValue: ShellDataContextValue = {
      buildInfo: null,
      appLockStatus: null,
      snapshot: null,
      dashboard: null,
      loading: false,
      busyAction: null,
      busyOverlay: null,
      error: 'database key is required for encrypted archives',
      notice: null,
      refreshKey: 0,
      refreshAppData: vi.fn().mockResolvedValue(undefined),
      saveConfig: vi.fn().mockRejectedValue(new Error('not implemented')),
      initializeArchive: vi
        .fn()
        .mockRejectedValue(new Error('not implemented')),
      runBackup: vi.fn().mockRejectedValue(new Error('not implemented')),
      setAppLockPasscode: vi
        .fn()
        .mockRejectedValue(new Error('not implemented')),
      clearAppLockPasscode: vi
        .fn()
        .mockRejectedValue(new Error('not implemented')),
      lockAppSession: vi.fn().mockRejectedValue(new Error('not implemented')),
      unlockAppSession: vi.fn().mockRejectedValue(new Error('not implemented')),
      clearNotice: vi.fn(),
    }

    render(
      <I18nProvider>
        <ProfileScopeProvider>
          <ShellDataContext.Provider value={shellValue}>
            <MemoryRouter>
              <Topbar
                screen={{
                  ...onboardingScreen,
                  labelKey: 'navigation.dashboardLabel',
                  titleKey: 'navigation.dashboardTitle',
                  subtitleKey: 'navigation.dashboardSubtitle',
                }}
              />
            </MemoryRouter>
          </ShellDataContext.Provider>
        </ProfileScopeProvider>
      </I18nProvider>,
    )

    expect(screen.getByRole('button', { name: 'Check security' })).toBeEnabled()
  })
})
