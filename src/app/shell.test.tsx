/**
 * @file shell.test.tsx
 * @description Focused coverage for the persistent app shell chrome owner.
 * @module app
 *
 * ## Responsibilities
 * - Verify the shell responds to viewport collapse changes and manual sidebar toggles.
 * - Keep AppShell's route-wrapper behavior covered without mounting every real chrome child.
 *
 * ## Not responsible for
 * - Re-testing Sidebar, Topbar, or route content internals.
 * - Re-testing shell-data bootstrap behavior.
 *
 * ## Dependencies
 * - Uses a data router because AppShell reads route matches and renders an Outlet.
 *
 * ## Performance notes
 * - Child chrome is mocked so this test stays scoped to AppShell state transitions.
 */

import { act } from 'react'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { afterEach, describe, expect, test, vi } from 'vitest'
import {
  ShellDataContext,
  type ShellDataContextValue,
} from './shell-data-context'
import { AppShell } from './shell'
import { appScreens } from './router'

vi.mock('../components/sidebar', () => ({
  Sidebar: ({
    collapsed,
    onToggle,
  }: {
    collapsed: boolean
    onToggle: () => void
  }) => (
    <button data-collapsed={String(collapsed)} onClick={onToggle} type="button">
      sidebar
    </button>
  ),
}))

vi.mock('../components/topbar', () => ({
  Topbar: ({ screen }: { screen: { id: string } }) => (
    <div data-testid="topbar-screen">{screen.id}</div>
  ),
}))

vi.mock('../components/primitives/busy-overlay', () => ({
  BusyOverlay: ({ label }: { label: string }) => <div>{label}</div>,
}))

describe('AppShell', () => {
  afterEach(() => {
    vi.restoreAllMocks()
  })

  test('tracks responsive sidebar collapse and manual toggles', async () => {
    const user = userEvent.setup()
    let changeHandler: ((event: MediaQueryListEvent) => void) | null = null
    const addEventListener = vi.fn(
      (_event: 'change', handler: (event: MediaQueryListEvent) => void) => {
        changeHandler = handler
      },
    )
    const removeEventListener = vi.fn()
    const matchMedia = vi.fn().mockReturnValue({
      matches: true,
      addEventListener,
      removeEventListener,
    })
    Object.defineProperty(window, 'matchMedia', {
      configurable: true,
      value: matchMedia,
    })

    const view = renderShell()

    const shell = screen.getByTestId('app-shell')
    const sidebar = screen.getByRole('button', { name: 'sidebar' })
    expect(shell).toHaveAttribute('data-sidebar-collapsed', 'true')
    expect(sidebar).toHaveAttribute('data-collapsed', 'true')
    expect(matchMedia.mock.calls).toEqual([
      ['(max-width: 1200px)'],
      ['(max-width: 1200px)'],
    ])

    await user.click(sidebar)
    expect(shell).toHaveAttribute('data-sidebar-collapsed', 'false')

    act(() => {
      changeHandler?.({ matches: true } as MediaQueryListEvent)
    })
    expect(shell).toHaveAttribute('data-sidebar-collapsed', 'true')

    view.unmount()
    expect(removeEventListener).toHaveBeenCalledWith('change', changeHandler)
  })

  test('renders without matchMedia and shows shell busy overlay fallbacks', () => {
    Object.defineProperty(window, 'matchMedia', {
      configurable: true,
      value: undefined,
    })

    renderShell({
      busyAction: 'Running backup',
      busyOverlay: null,
    })

    expect(screen.getByTestId('app-shell')).toHaveAttribute(
      'data-sidebar-collapsed',
      'false',
    )
    expect(screen.getByText('Running backup')).toBeVisible()
    expect(screen.getByTestId('topbar-screen')).toHaveTextContent('dashboard')
  })

  test('uses the deepest matched route handle as the active screen', () => {
    Object.defineProperty(window, 'matchMedia', {
      configurable: true,
      value: undefined,
    })

    renderShell({}, '/jobs')

    expect(screen.getByTestId('topbar-screen')).toHaveTextContent('jobs')
  })

  test('rewires the responsive listener if the matchMedia implementation changes', async () => {
    const user = userEvent.setup()
    const firstRemoveEventListener = vi.fn()
    const secondAddEventListener = vi.fn()
    const firstMatchMedia = vi.fn().mockReturnValue({
      matches: false,
      addEventListener: vi.fn(),
      removeEventListener: firstRemoveEventListener,
    })
    const secondMatchMedia = vi.fn().mockReturnValue({
      matches: false,
      addEventListener: secondAddEventListener,
      removeEventListener: vi.fn(),
    })
    Object.defineProperty(window, 'matchMedia', {
      configurable: true,
      value: firstMatchMedia,
    })

    renderShell()
    Object.defineProperty(window, 'matchMedia', {
      configurable: true,
      value: secondMatchMedia,
    })

    await user.click(screen.getByRole('button', { name: 'sidebar' }))

    await waitFor(() =>
      expect(secondMatchMedia).toHaveBeenCalledWith('(max-width: 1200px)'),
    )
    expect(firstRemoveEventListener).toHaveBeenCalledWith(
      'change',
      expect.any(Function),
    )
    expect(secondAddEventListener).toHaveBeenCalledWith(
      'change',
      expect.any(Function),
    )
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
          <ShellDataContext.Provider value={shellValue(overrides)}>
            <AppShell />
          </ShellDataContext.Provider>
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

  return render(<RouterProvider router={router} />)
}

function shellValue(
  overrides: Partial<ShellDataContextValue> = {},
): ShellDataContextValue {
  return {
    busyAction: null,
    busyOverlay: null,
    ...overrides,
  } as ShellDataContextValue
}
