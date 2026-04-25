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

import userEvent from '@testing-library/user-event'
import { render, screen, waitFor } from '@testing-library/react'
import {
  createHashRouter,
  MemoryRouter,
  RouterProvider,
  useLocation,
  useNavigate,
} from 'react-router-dom'
import { describe, expect, test, vi } from 'vitest'
import { onboardingScreen } from '../../app/router'
import { ShellDataProvider } from '../../app/shell-data'
import {
  ShellDataContext,
  type ShellDataContextValue,
} from '../../app/shell-data-context'
import { I18nProvider } from '../../lib/i18n'
import { ProfileScopeProvider } from '../../lib/profile-scope'
import type { AppSnapshot } from '../../lib/types'
import { Topbar } from './index'

describe('Topbar', () => {
  function RouteDriver() {
    const location = useLocation()
    const navigate = useNavigate()

    return (
      <>
        <button type="button" onClick={() => navigate('/explorer')}>
          Go to explorer
        </button>
        <p>{location.pathname}</p>
      </>
    )
  }

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
      refreshRuntimeStatus: vi.fn().mockResolvedValue({
        aiQueue: null,
        intelligence: null,
        loading: false,
        error: null,
      }),
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

  test('starts manual backups without leaking rejected promises from chrome buttons', async () => {
    const user = userEvent.setup()
    const runBackup = vi.fn().mockRejectedValue(new Error('backup failed'))
    const unhandledRejection = vi.fn((event: PromiseRejectionEvent) => {
      event.preventDefault()
    })
    const shellValue: ShellDataContextValue = {
      buildInfo: null,
      appLockStatus: null,
      snapshot: {
        config: {
          initialized: true,
          selectedProfileIds: ['chrome:Default'],
        },
        browserProfiles: [],
      } as unknown as AppSnapshot,
      dashboard: null,
      loading: false,
      busyAction: null,
      busyOverlay: null,
      error: null,
      notice: null,
      refreshKey: 0,
      refreshAppData: vi.fn().mockResolvedValue(undefined),
      refreshRuntimeStatus: vi.fn().mockResolvedValue({
        aiQueue: null,
        intelligence: null,
        loading: false,
        error: null,
      }),
      saveConfig: vi.fn().mockRejectedValue(new Error('not implemented')),
      initializeArchive: vi
        .fn()
        .mockRejectedValue(new Error('not implemented')),
      runBackup,
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

    window.addEventListener('unhandledrejection', unhandledRejection)

    try {
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

      await user.click(screen.getByRole('button', { name: 'Backup now' }))
      await waitFor(() => expect(runBackup).toHaveBeenCalledTimes(1))
      await new Promise((resolve) => setTimeout(resolve, 0))

      expect(unhandledRejection).not.toHaveBeenCalled()
    } finally {
      window.removeEventListener('unhandledrejection', unhandledRejection)
    }
  })

  test('tracks in-app route history for the global back and forward buttons', async () => {
    const user = userEvent.setup()
    const shellValue: ShellDataContextValue = {
      buildInfo: null,
      appLockStatus: null,
      snapshot: null,
      dashboard: null,
      loading: false,
      busyAction: null,
      busyOverlay: null,
      error: null,
      notice: null,
      refreshKey: 0,
      refreshAppData: vi.fn().mockResolvedValue(undefined),
      refreshRuntimeStatus: vi.fn().mockResolvedValue({
        aiQueue: null,
        intelligence: null,
        loading: false,
        error: null,
      }),
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

    window.location.hash = '#/'
    const router = createHashRouter([
      {
        path: '/',
        element: (
          <>
            <Topbar
              screen={{
                ...onboardingScreen,
                labelKey: 'navigation.dashboardLabel',
                titleKey: 'navigation.dashboardTitle',
                subtitleKey: 'navigation.dashboardSubtitle',
              }}
            />
            <RouteDriver />
          </>
        ),
      },
      {
        path: '/explorer',
        element: (
          <>
            <Topbar
              screen={{
                ...onboardingScreen,
                labelKey: 'navigation.dashboardLabel',
                titleKey: 'navigation.dashboardTitle',
                subtitleKey: 'navigation.dashboardSubtitle',
              }}
            />
            <RouteDriver />
          </>
        ),
      },
    ])

    render(
      <I18nProvider>
        <ProfileScopeProvider>
          <ShellDataContext.Provider value={shellValue}>
            <RouterProvider router={router} />
          </ShellDataContext.Provider>
        </ProfileScopeProvider>
      </I18nProvider>,
    )

    const backButton = screen.getByRole('button', { name: 'Go back' })
    const forwardButton = screen.getByRole('button', { name: 'Go forward' })

    expect(backButton).toBeDisabled()
    expect(forwardButton).toBeDisabled()
    expect(screen.getByText('/')).toBeVisible()

    await user.click(screen.getByRole('button', { name: 'Go to explorer' }))

    expect(await screen.findByText('/explorer')).toBeVisible()
    expect(backButton).toBeEnabled()
    expect(forwardButton).toBeDisabled()

    await user.click(backButton)

    expect(await screen.findByText('/')).toBeVisible()
    expect(backButton).toBeDisabled()
    expect(forwardButton).toBeEnabled()

    await user.click(forwardButton)

    expect(await screen.findByText('/explorer')).toBeVisible()
    expect(backButton).toBeEnabled()
    expect(forwardButton).toBeDisabled()
  })
})
