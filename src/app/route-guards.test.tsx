/**
 * @file route-guards.test.tsx
 * @description Focused coverage for shell app-lock redirect boundaries.
 * @module app
 */

import { render, screen } from '@testing-library/react'
import {
  createMemoryRouter,
  RouterProvider,
  useLocation,
} from 'react-router-dom'
import { describe, expect, test, vi } from 'vitest'
import type { AppLockStatus, AppSnapshot } from '../lib/types'
import { createNamespaceTranslator, I18nProvider } from '../lib/i18n'
import {
  ShellDataContext,
  type ShellDataContextValue,
} from './shell-data-context'
import { RequireLockScreen, RequireUnlockedShell } from './route-guards'

vi.mock('../pages/lock', () => ({
  LockPage: () => <p>lock page</p>,
}))

const commonT = createNamespaceTranslator('en', 'common')

describe('route guards', () => {
  test.each([
    { initialized: false, target: 'onboarding target' },
    { initialized: true, target: 'home target' },
  ])(
    'redirects away from the lock screen when the app is unlocked (initialized=$initialized)',
    async ({ initialized, target }) => {
      renderGuard({ initialized })

      expect(await screen.findByText(target)).toBeVisible()
    },
  )

  test('redirects locked shell routes to lock with the full encoded return target', async () => {
    const router = createMemoryRouter(
      [
        {
          path: '/explorer',
          element: (
            <ShellDataContext.Provider
              value={
                shellValue({
                  initialized: true,
                  appLockStatus: {
                    enabled: true,
                    locked: true,
                  } as AppLockStatus,
                }) as ShellDataContextValue
              }
            >
              <RequireUnlockedShell>
                <p>protected target</p>
              </RequireUnlockedShell>
            </ShellDataContext.Provider>
          ),
        },
        {
          path: '/lock',
          element: <LocationEcho />,
        },
      ],
      {
        initialEntries: ['/explorer?q=path#panel'],
      },
    )

    render(
      <I18nProvider>
        <RouterProvider router={router} />
      </I18nProvider>,
    )

    expect(await screen.findByTestId('location')).toHaveTextContent(
      '?next=%2Fexplorer%3Fq%3Dpath%23panel',
    )
    expect(screen.queryByText('protected target')).not.toBeInTheDocument()
  })

  test('shows a neutral loading gate while lock state is still unknown', async () => {
    const router = createMemoryRouter(
      [
        {
          path: '/explorer',
          element: (
            <ShellDataContext.Provider
              value={
                shellValue({
                  initialized: true,
                  appLockStatus: null,
                  loading: true,
                }) as ShellDataContextValue
              }
            >
              <RequireUnlockedShell>
                <p>protected target</p>
              </RequireUnlockedShell>
            </ShellDataContext.Provider>
          ),
        },
      ],
      {
        initialEntries: ['/explorer'],
      },
    )

    render(
      <I18nProvider>
        <RouterProvider router={router} />
      </I18nProvider>,
    )

    expect(await screen.findByText(commonT('loading'))).toBeVisible()
    expect(screen.queryByText('protected target')).not.toBeInTheDocument()
  })

  test('keeps unlocked shell content visible when loading has a known unlocked status', async () => {
    renderUnlockedGuard({
      appLockStatus: {
        enabled: true,
        locked: false,
      } as AppLockStatus,
      loading: true,
    })

    expect(await screen.findByText('protected target')).toBeVisible()
    expect(screen.queryByText(commonT('loading'))).not.toBeInTheDocument()
  })

  test('keeps unlocked shell content visible for missing status once loading is false', async () => {
    renderUnlockedGuard({
      appLockStatus: null,
      loading: false,
    })

    expect(await screen.findByText('protected target')).toBeVisible()
    expect(screen.queryByText(commonT('loading'))).not.toBeInTheDocument()
  })

  test('honors an explicit next target when leaving the lock screen unlocked', async () => {
    renderGuard({
      initialized: true,
      initialEntry: '/lock?next=%2Fjobs%3Ftab%3Dfailed',
    })

    expect(await screen.findByText('jobs target')).toBeVisible()
  })

  test('renders the lock page when the session is explicitly locked', async () => {
    renderGuard({
      appLockStatus: {
        enabled: true,
        locked: true,
      } as AppLockStatus,
      initialized: true,
    })

    expect(await screen.findByText('lock page')).toBeVisible()
    expect(screen.queryByText('home target')).not.toBeInTheDocument()
  })

  test('uses the lock-screen loading gate only while status is unknown', async () => {
    renderGuard({
      appLockStatus: null,
      initialized: true,
      loading: true,
    })

    expect(await screen.findByText(commonT('loading'))).toBeVisible()
    expect(screen.queryByText('home target')).not.toBeInTheDocument()
  })

  test('redirects from lock screen when status is known unlocked even during loading', async () => {
    renderGuard({
      initialized: true,
      loading: true,
    })

    expect(await screen.findByText('home target')).toBeVisible()
    expect(screen.queryByText(commonT('loading'))).not.toBeInTheDocument()
  })

  test('redirects from lock screen when loading is false and status is unavailable', async () => {
    renderGuard({
      appLockStatus: null,
      initialized: true,
      loading: false,
    })

    expect(await screen.findByText('home target')).toBeVisible()
  })
})

function renderGuard({
  appLockStatus,
  initialized,
  initialEntry = '/lock?next=%20',
  loading,
}: {
  appLockStatus?: AppLockStatus | null
  initialized: boolean
  initialEntry?: string
  loading?: boolean
}) {
  const router = createMemoryRouter(
    [
      {
        path: '/lock',
        element: (
          <ShellDataContext.Provider
            value={
              shellValue({
                appLockStatus,
                initialized,
                loading,
              }) as ShellDataContextValue
            }
          >
            <RequireLockScreen />
          </ShellDataContext.Provider>
        ),
      },
      {
        path: '/',
        element: <p>home target</p>,
      },
      {
        path: '/onboarding',
        element: <p>onboarding target</p>,
      },
      {
        path: '/jobs',
        element: <p>jobs target</p>,
      },
    ],
    {
      initialEntries: [initialEntry],
    },
  )

  return render(
    <I18nProvider>
      <RouterProvider router={router} />
    </I18nProvider>,
  )
}

function renderUnlockedGuard({
  appLockStatus,
  loading,
}: {
  appLockStatus: AppLockStatus | null
  loading: boolean
}) {
  const router = createMemoryRouter(
    [
      {
        path: '/explorer',
        element: (
          <ShellDataContext.Provider
            value={
              shellValue({
                appLockStatus,
                initialized: true,
                loading,
              }) as ShellDataContextValue
            }
          >
            <RequireUnlockedShell>
              <p>protected target</p>
            </RequireUnlockedShell>
          </ShellDataContext.Provider>
        ),
      },
      {
        path: '/lock',
        element: <LocationEcho />,
      },
    ],
    {
      initialEntries: ['/explorer'],
    },
  )

  return render(
    <I18nProvider>
      <RouterProvider router={router} />
    </I18nProvider>,
  )
}

function shellValue({
  appLockStatus = {
    enabled: false,
    locked: false,
  } as AppLockStatus,
  initialized,
  loading = false,
}: {
  appLockStatus?: AppLockStatus | null
  initialized: boolean
  loading?: boolean
}): Partial<ShellDataContextValue> {
  return {
    appLockStatus,
    loading,
    snapshot: {
      config: {
        initialized,
      },
    } as AppSnapshot,
  }
}

function LocationEcho() {
  const location = useLocation()

  return (
    <p data-testid="location">
      {location.pathname}
      {location.search}
      {location.hash}
    </p>
  )
}
