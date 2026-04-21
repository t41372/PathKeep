/**
 * This module belongs to the application shell layer for Shell.test.tsx.
 *
 * Why this file exists:
 * - Files under `src/app/` explain how the desktop shell is stitched together before route-specific UI takes over.
 * - This is where shared profile scope, app-lock gating, route metadata, and shell-level loading grammar should stay readable.
 *
 * Main declarations:
 * - This file is mostly internal implementation detail.
 *
 * Source-of-truth notes:
 * - Keep this aligned with `docs/design/screens-and-nav.md` for information architecture and route semantics.
 * - Keep busy, locked, degraded, and loading behavior aligned with `docs/design/ux-principles.md`.
 */

import { render, screen } from '@testing-library/react'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { beforeEach, describe, expect, test } from 'vitest'
import { AppShell } from './shell'
import { ShellDataProvider } from './shell-data'
import {
  type ShellDataContextValue,
  ShellDataContext,
} from './shell-data-context'
import { I18nProvider } from '../lib/i18n'
import { backendTestHarness } from '../lib/backend'
import { ProfileScopeProvider } from '../lib/profile-scope'

describe('AppShell', () => {
  beforeEach(() => {
    backendTestHarness.reset()
  })

  test('falls back to the dashboard metadata when no route handle is present', async () => {
    const router = createMemoryRouter(
      [
        {
          path: '/',
          element: <AppShell />,
        },
      ],
      {
        initialEntries: ['/'],
      },
    )

    render(
      <I18nProvider>
        <ProfileScopeProvider>
          <ShellDataProvider>
            <RouterProvider router={router} />
          </ShellDataProvider>
        </ProfileScopeProvider>
      </I18nProvider>,
    )

    expect(
      await screen.findByRole('heading', { name: 'Dashboard' }),
    ).toBeVisible()
    expect(
      screen.getByRole('button', { name: /Initialize first/ }),
    ).toBeVisible()
  })

  test('shows the busy overlay details when a long-running shell action is active', async () => {
    const router = createMemoryRouter(
      [
        {
          path: '/',
          element: <AppShell />,
        },
      ],
      {
        initialEntries: ['/'],
      },
    )

    const shellValue: ShellDataContextValue = {
      buildInfo: null,
      appLockStatus: null,
      snapshot: null,
      dashboard: null,
      loading: false,
      busyAction: 'Writing archive facts',
      busyOverlay: {
        label: 'Writing archive facts',
        detail: 'Large real-world profiles can take a while here.',
        steps: [
          'Inspect selected browser profiles',
          'Write the canonical archive run',
          'Refresh dashboard and shell state',
        ],
        activeStep: 1,
      },
      error: null,
      notice: null,
      refreshKey: 0,
      refreshAppData: () => Promise.resolve(undefined),
      refreshRuntimeStatus: () =>
        Promise.resolve({
          aiQueue: null,
          intelligence: null,
          loading: false,
          error: null,
        }),
      saveConfig: () => Promise.reject(new Error('not implemented')),
      initializeArchive: () => Promise.reject(new Error('not implemented')),
      runBackup: () => Promise.reject(new Error('not implemented')),
      setAppLockPasscode: () => Promise.reject(new Error('not implemented')),
      clearAppLockPasscode: () => Promise.reject(new Error('not implemented')),
      lockAppSession: () => Promise.reject(new Error('not implemented')),
      unlockAppSession: () => Promise.reject(new Error('not implemented')),
      clearNotice: () => undefined,
    }

    render(
      <I18nProvider>
        <ProfileScopeProvider>
          <ShellDataContext.Provider value={shellValue}>
            <RouterProvider router={router} />
          </ShellDataContext.Provider>
        </ProfileScopeProvider>
      </I18nProvider>,
    )

    expect(await screen.findByRole('status')).toHaveTextContent(
      'Writing archive facts',
    )
    expect(
      screen.getByText('Large real-world profiles can take a while here.'),
    ).toBeVisible()
    expect(screen.getByText('Refresh dashboard and shell state')).toBeVisible()
  })
})
