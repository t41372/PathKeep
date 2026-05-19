/**
 * Coverage for the v0.3 paper-redesign app shell composition.
 *
 * What this test owns:
 * - Verifies the shell renders sidebar, topbar, status bar, route outlet, and
 *   the busy overlay path.
 * - Verifies sidebar collapsed state persists through localStorage.
 * - Verifies theme toggle flips data-theme and persists.
 *
 * What this test does NOT own:
 * - Individual shell component rendering (covered in their own tests).
 * - Backend wiring details (covered by shell-data tests).
 */

import { act } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import {
  createMemoryRouter,
  RouterProvider,
} from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import {
  ShellDataContext,
  type ShellDataContextValue,
} from './shell-data-context'
import { AppShell } from './shell'
import { appScreens } from './router'
import { I18nProvider } from '@/lib/i18n'

vi.mock('@/components/primitives/busy-overlay', () => ({
  BusyOverlay: ({ label }: { label: string }) => (
    <div data-testid="busy-overlay">{label}</div>
  ),
}))

vi.mock('@/lib/ipc/bridge', () => ({
  invokeCommand: vi.fn().mockResolvedValue({ rows: [] }),
}))

describe('AppShell (paper redesign)', () => {
  beforeEach(() => {
    window.localStorage.clear()
    document.documentElement.removeAttribute('data-theme')
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  test('renders the sidebar, topbar, status bar, and outlet content', () => {
    renderShell({}, '/')
    expect(screen.getByTestId('app-shell')).toBeInTheDocument()
    expect(screen.getByTestId('pk-sidebar')).toBeInTheDocument()
    expect(screen.getByTestId('pk-topbar')).toBeInTheDocument()
    expect(screen.getByTestId('pk-status-bar')).toBeInTheDocument()
    expect(screen.getByText('route body')).toBeInTheDocument()
  })

  test('renders the busy overlay when shell context advertises busyAction', () => {
    renderShell({ busyAction: 'Running backup', busyOverlay: null })
    expect(screen.getByTestId('busy-overlay')).toHaveTextContent('Running backup')
  })

  test('uses the deepest matched route handle as the active screen', () => {
    renderShell({}, '/jobs')
    expect(screen.getByTestId('pk-topbar')).toBeInTheDocument()
  })

  test('persists sidebar collapsed state to localStorage on toggle', async () => {
    const user = userEvent.setup()
    renderShell({}, '/')
    const sidebar = screen.getByTestId('pk-sidebar')
    expect(sidebar).toHaveAttribute('data-collapsed', 'false')

    const collapseButton = sidebar.querySelector(
      'button[title*="Collapse"]',
    ) as HTMLButtonElement | null
    expect(collapseButton).not.toBeNull()
    await user.click(collapseButton!)
    expect(window.localStorage.getItem('pathkeep.sidebar.collapsed')).toBe('true')
  })

  test('flips and persists theme on the html element', () => {
    window.localStorage.setItem('pathkeep.theme', 'dark')
    renderShell({}, '/')
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark')
  })
})

function renderShell(
  overrides: Partial<ShellDataContextValue> = {},
  initialEntry = '/',
) {
  const router = createMemoryRouter(
    [
      {
        path: '/',
        element: (
          <I18nProvider>
            <ShellDataContext.Provider value={shellValue(overrides)}>
              <AppShell />
            </ShellDataContext.Provider>
          </I18nProvider>
        ),
        children: [
          {
            index: true,
            element: <p>route body</p>,
          },
          {
            path: 'jobs',
            element: <p>jobs body</p>,
            handle: {
              screen: appScreens.find((screen) => screen.id === 'jobs'),
            },
          },
        ],
      },
    ],
    {
      initialEntries: [initialEntry],
    },
  )

  return act(() => render(<RouterProvider router={router} />))
}

function shellValue(
  overrides: Partial<ShellDataContextValue> = {},
): ShellDataContextValue {
  return {
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
    refreshRuntimeStatus: vi.fn(),
    saveConfig: vi.fn(),
    initializeArchive: vi.fn(),
    runBackup: vi.fn().mockResolvedValue({}),
    setAppLockPasscode: vi.fn(),
    clearAppLockPasscode: vi.fn(),
    lockAppSession: vi.fn().mockResolvedValue({}),
    unlockAppSession: vi.fn(),
    clearNotice: vi.fn(),
    ...overrides,
  } as ShellDataContextValue
}
