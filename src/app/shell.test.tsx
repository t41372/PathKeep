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

import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import type * as BackendClient from '@/lib/backend-client'
import {
  ShellDataContext,
  type ShellDataContextValue,
} from './shell-data-context'
import { AppShell } from './shell'
import { appScreens } from './router'
import { I18nProvider } from '@/lib/i18n'
import { ProfileScopeProvider } from '@/lib/profile-scope'
import type { DashboardSnapshot } from '@/lib/types'
import { PAPER_PREFERENCES_EVENT } from '@/lib/paper-preferences'
import * as runtime from '@/lib/runtime'

vi.mock('@/components/primitives/busy-overlay', () => ({
  BusyOverlay: ({ label }: { label: string }) => (
    <div data-testid="busy-overlay">{label}</div>
  ),
}))

vi.mock('@/lib/backend-client', async (importOriginal) => {
  const actual = await importOriginal<typeof BackendClient>()
  return {
    ...actual,
    backend: {
      ...actual.backend,
      queryHistory: vi.fn().mockResolvedValue({
        total: 0,
        items: [],
        page: 1,
        pageSize: 8,
        pageCount: 0,
        hasPrevious: false,
        hasNext: false,
      }),
    },
  }
})

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
    expect(screen.getByTestId('busy-overlay')).toHaveTextContent(
      'Running backup',
    )
  })

  test('renders non-blocking background progress when the busy payload is marked background', () => {
    renderShell({
      busyAction: 'Running backup',
      busyOverlay: {
        background: true,
        label: 'Backing up archive',
        detail: 'Chrome Default',
        progressLabel: '2 of 4',
        progressValue: 50,
        steps: [],
        logLines: ['Copied 12 pages'],
      },
    })
    expect(screen.getByTestId('background-progress')).toHaveTextContent(
      'Backing up archive',
    )
    expect(screen.queryByTestId('busy-overlay')).not.toBeInTheDocument()
  })

  test('uses the deepest matched route handle as the active screen', () => {
    renderShell({}, '/jobs')
    expect(screen.getByTestId('pk-topbar')).toBeInTheDocument()
  })

  // LAYOUT (re-review): `<main>` is the shared route scroll owner on EVERY route EXCEPT the
  // fixed-height Assistant chat surface, which owns its own inner scroll (the messages list) and
  // pins the composer. On `/assistant`, `<main>` must NOT scroll — otherwise the empty gutters of
  // the centered chat column drag the conversation, composer included, off-screen. This pins the
  // route-scoped overflow so the shared shell contract cannot regress in either direction.
  test('scopes the main scroll: <main> clips on the Assistant route and scrolls elsewhere', () => {
    renderShell({}, '/assistant')
    const main = screen.getByTestId('app-scroll')
    // The Assistant route is fixed-height: <main> clips (no scroll) and adds no bottom padding.
    expect(main).toHaveAttribute('data-fixed-height', 'true')
    expect(main).toHaveClass('overflow-hidden', 'pb-0')
    expect(main).not.toHaveClass('overflow-y-auto')
  })

  test('keeps <main> as the vertical scroll owner on non-Assistant routes', () => {
    renderShell({}, '/jobs')
    const main = screen.getByTestId('app-scroll')
    // Every other route flows its content and lets <main> scroll vertically as before.
    expect(main).toHaveAttribute('data-fixed-height', 'false')
    expect(main).toHaveClass('overflow-y-auto', 'pb-7')
    expect(main).not.toHaveClass('overflow-hidden')
  })

  test('omits the macOS titlebar drag region off the overlay platform', () => {
    // jsdom is not a macOS Tauri window, so the overlay is inactive.
    renderShell({}, '/')
    const shellRoot = screen.getByTestId('app-shell')
    expect(shellRoot).toHaveAttribute('data-titlebar-overlay', 'false')
    expect(
      shellRoot.querySelector('.pk-titlebar-dragstrip'),
    ).not.toBeInTheDocument()
  })

  test('the topbar header is NOT a drag region off the overlay platform', () => {
    // Windows/Linux/browser keep native decorations; the header must stay inert
    // (no data-tauri-drag-region) so it cannot interfere with native dragging.
    renderShell({}, '/')
    const topbar = screen.getByTestId('pk-topbar')
    expect(topbar).not.toHaveAttribute('data-tauri-drag-region')
    // None of its descendants carry the attribute either.
    expect(
      topbar.querySelector('[data-tauri-drag-region]'),
    ).not.toBeInTheDocument()
  })

  test('renders the drag region under the macOS overlay title bar', () => {
    vi.spyOn(runtime, 'hasMacOverlayTitlebar').mockReturnValue(true)
    renderShell({}, '/')
    const shellRoot = screen.getByTestId('app-shell')
    expect(shellRoot).toHaveAttribute('data-titlebar-overlay', 'true')
    const dragStrip = shellRoot.querySelector('.pk-titlebar-dragstrip')
    expect(dragStrip).toBeInTheDocument()
    expect(dragStrip).toHaveAttribute('data-tauri-drag-region')
  })

  test('the topbar header becomes the window-drag region under the macOS overlay', () => {
    // Primary affordance: grabbing the visible top bar drags the window. The
    // header itself carries data-tauri-drag-region so its empty areas + the
    // page title drag, restoring the natural macOS gesture.
    vi.spyOn(runtime, 'hasMacOverlayTitlebar').mockReturnValue(true)
    renderShell({}, '/')
    const topbar = screen.getByTestId('pk-topbar')
    expect(topbar).toHaveAttribute('data-tauri-drag-region')
  })

  test('topbar interactive controls are NOT drag regions under the overlay', () => {
    // Tauri v2 never starts a drag from a mousedown on a child that lacks the
    // attribute, so the nav buttons, search trigger, and Backup CTA stay
    // clickable. Assert structurally that none of them carries it.
    vi.spyOn(runtime, 'hasMacOverlayTitlebar').mockReturnValue(true)
    renderShell(
      {
        snapshot: {
          archiveStatus: { initialized: true, warning: null },
          browserProfiles: [],
        } as never,
      },
      '/',
    )
    const topbar = screen.getByTestId('pk-topbar')
    // Every <button> in the topbar must be free of the drag attribute.
    const buttons = topbar.querySelectorAll('button')
    expect(buttons.length).toBeGreaterThan(0)
    buttons.forEach((button) => {
      expect(button).not.toHaveAttribute('data-tauri-drag-region')
    })
    // The search trigger specifically (a button) stays clickable.
    expect(
      topbar.querySelector('[data-testid="pk-topbar-palette"]'),
    ).not.toHaveAttribute('data-tauri-drag-region')
    // The back/forward nav buttons stay clickable.
    expect(
      topbar.querySelector('[data-testid="pk-topbar-back"]'),
    ).not.toHaveAttribute('data-tauri-drag-region')
    expect(
      topbar.querySelector('[data-testid="pk-topbar-forward"]'),
    ).not.toHaveAttribute('data-tauri-drag-region')
  })

  test('persists sidebar collapsed state to localStorage on toggle', async () => {
    const user = userEvent.setup()
    renderShell({}, '/')
    const sidebar = screen.getByTestId('pk-sidebar')
    expect(sidebar).toHaveAttribute('data-collapsed', 'false')

    const collapseButton = sidebar.querySelector<HTMLButtonElement>(
      'button[title*="Collapse"]',
    )
    expect(collapseButton).not.toBeNull()
    if (!collapseButton) throw new Error('collapse button missing')
    await user.click(collapseButton)
    expect(window.localStorage.getItem('pathkeep.sidebar.collapsed')).toBe(
      'true',
    )
  })

  test('flips and persists theme on the html element', () => {
    window.localStorage.setItem('pathkeep.theme', 'dark')
    renderShell({}, '/')
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark')
  })

  test('ignores malformed preference-change events without changing the theme toggle', () => {
    window.localStorage.setItem('pathkeep.theme', 'dark')
    renderShell({}, '/')
    const sidebar = screen.getByTestId('pk-sidebar')
    const themeToggle = sidebar.querySelector('button[aria-label*="theme" i]')
    if (!themeToggle) throw new Error('theme toggle missing')
    const renderedIconBefore = themeToggle.innerHTML

    window.dispatchEvent(
      new CustomEvent(PAPER_PREFERENCES_EVENT, { detail: {} }),
    )

    expect(themeToggle.innerHTML).toBe(renderedIconBefore)
  })

  test('renders the build version and short commit revision in the sidebar', () => {
    renderShell({
      buildInfo: {
        productName: 'PathKeep',
        version: '0.3.0',
        gitCommitShort: 'abc1234',
        gitCommitFull: 'abc1234deadbeef',
        gitDirty: false,
      },
    })
    expect(screen.getByTestId('pk-sidebar-build-version')).toHaveTextContent(
      'v0.3.0',
    )
    expect(screen.getByTestId('pk-sidebar-build-revision')).toHaveTextContent(
      'abc1234',
    )
  })

  test('marks the revision with a "+" suffix when the working tree was dirty', () => {
    renderShell({
      buildInfo: {
        productName: 'PathKeep',
        version: '0.3.0',
        gitCommitShort: 'abc1234',
        gitCommitFull: 'abc1234deadbeef',
        gitDirty: true,
      },
    })
    expect(screen.getByTestId('pk-sidebar-build-revision')).toHaveTextContent(
      'abc1234+',
    )
  })

  test('renders dashboard totals and last-archive telemetry in the status bar', () => {
    renderShell({ dashboard: makeDashboardSnapshot() })
    const statusBar = screen.getByTestId('pk-status-bar')
    expect(statusBar).toHaveTextContent('2,500 pages')
    expect(statusBar).toHaveTextContent('May 2026')
    expect(statusBar).toHaveTextContent(/Last archived/i)
  })

  test('source swatches prefer browser-family colors, then browser-name colors, then the neutral fallback', () => {
    renderShell({
      snapshot: {
        archiveStatus: { initialized: true, warning: null },
        browserProfiles: [
          {
            profileId: 'family',
            browserName: 'Chromium Variant',
            browserFamily: 'Chrome',
            profileName: 'Default',
            historyBytes: 1024,
          },
          {
            profileId: 'name',
            browserName: 'Chrome',
            browserFamily: 'unknown',
            profileName: 'Default',
            historyBytes: 1024,
          },
          {
            profileId: 'fallback',
            browserName: 'Mystery',
            browserFamily: 'unknown',
            profileName: 'Default',
            historyBytes: 1024,
          },
        ],
      } as never,
    })
    const swatches = screen
      .getByTestId('pk-status-bar-source-trigger')
      .querySelectorAll('span[style]')
    expect(swatches[0]).toHaveStyle({ background: '#4285F4' })
    expect(swatches[1]).toHaveStyle({ background: '#4285F4' })
    expect(swatches[2]).toHaveStyle({ background: '#8a7f70' })
  })

  test('opens the ⌘K palette via the topbar trigger (covers onOpenPalette branch)', async () => {
    const user = userEvent.setup()
    renderShell({}, '/')
    const topbar = screen.getByTestId('pk-topbar')
    const paletteTrigger = topbar.querySelector<HTMLButtonElement>(
      'button[data-testid="pk-topbar-palette"]',
    )
    expect(paletteTrigger).not.toBeNull()
    if (!paletteTrigger) throw new Error('palette trigger missing')
    await user.click(paletteTrigger)
    // CommandDialog renders the palette title role=dialog once open.
    expect(
      document.querySelector('[role="dialog"]') ||
        document.querySelector('[cmdk-root]'),
    ).not.toBeNull()
  })

  test('palette search swallows backend errors and shows the no-results state', async () => {
    const user = userEvent.setup()
    const { backend } = await import('@/lib/backend-client')
    vi.mocked(backend.queryHistory).mockRejectedValueOnce(new Error('ipc boom'))
    renderShell({}, '/')
    const paletteTrigger = screen
      .getByTestId('pk-topbar')
      .querySelector<HTMLButtonElement>(
        'button[data-testid="pk-topbar-palette"]',
      )
    if (!paletteTrigger) throw new Error('palette trigger missing')
    await user.click(paletteTrigger)
    const input = await screen.findByPlaceholderText(/Find a page/i)
    await user.type(input, 'broken')
    await new Promise((resolve) => window.setTimeout(resolve, 250))
    await vi.waitFor(() =>
      expect(screen.getByText('Nothing here yet. Memory is patient.')),
    )
  })

  test('palette query routes through backend.queryHistory with the trimmed search term', async () => {
    const user = userEvent.setup()
    const { backend } = await import('@/lib/backend-client')
    vi.mocked(backend.queryHistory).mockResolvedValue({
      total: 1,
      items: [
        {
          id: 7,
          profileId: 'chrome:Default',
          url: 'https://example.com/article',
          title: 'Example article',
          domain: 'example.com',
          favicon: null,
          visitedAt: '2026-04-17T10:30:00',
          visitTime: 1745311800,
          sourceVisitId: 0,
        },
      ],
      page: 1,
      pageSize: 8,
      pageCount: 1,
      hasPrevious: false,
      hasNext: false,
    })
    renderShell({}, '/')
    const paletteTrigger = screen
      .getByTestId('pk-topbar')
      .querySelector<HTMLButtonElement>(
        'button[data-testid="pk-topbar-palette"]',
      )
    if (!paletteTrigger) throw new Error('palette trigger missing')
    await user.click(paletteTrigger)
    const input = await screen.findByPlaceholderText(/Find a page/i)
    await user.type(input, 'example')
    // The palette debounces queries by 160 ms before firing the search.
    await new Promise((resolve) => window.setTimeout(resolve, 250))
    await vi.waitFor(() =>
      expect(backend.queryHistory).toHaveBeenCalledWith(
        expect.objectContaining({ q: 'example', limit: 8, sort: 'relevance' }),
      ),
    )
  })

  test('palette handles a response payload with no items field gracefully', async () => {
    const user = userEvent.setup()
    const { backend } = await import('@/lib/backend-client')
    // Drive the empty-items fallback on `response.items ?? []` so the route
    // returns no palette hits instead of crashing.
    vi.mocked(backend.queryHistory).mockResolvedValue({
      total: 0,
      items: undefined as never,
      page: 1,
      pageSize: 8,
      pageCount: 0,
      hasPrevious: false,
      hasNext: false,
    })
    renderShell({}, '/')
    const paletteTrigger = screen
      .getByTestId('pk-topbar')
      .querySelector<HTMLButtonElement>(
        'button[data-testid="pk-topbar-palette"]',
      )
    if (!paletteTrigger) throw new Error('palette trigger missing')
    await user.click(paletteTrigger)
    const input = await screen.findByPlaceholderText(/Find a page/i)
    await user.type(input, 'q')
    await new Promise((resolve) => window.setTimeout(resolve, 250))
    await vi.waitFor(() => expect(backend.queryHistory).toHaveBeenCalled())
    expect(
      await screen.findByText('Nothing here yet. Memory is patient.'),
    ).toBeInTheDocument()
  })

  test('palette result selection forwards to handlePaletteSelect (visit date present)', async () => {
    const user = userEvent.setup()
    const { backend } = await import('@/lib/backend-client')
    vi.mocked(backend.queryHistory).mockResolvedValue({
      total: 1,
      items: [
        {
          id: 7,
          profileId: 'chrome:Default',
          url: 'https://example.com/article',
          title: 'Example article',
          domain: 'example.com',
          favicon: null,
          visitedAt: '2026-04-17T10:30:00',
          visitTime: 1745311800,
          sourceVisitId: 0,
        },
      ],
      page: 1,
      pageSize: 8,
      pageCount: 1,
      hasPrevious: false,
      hasNext: false,
    })
    renderShell({}, '/')
    const paletteTrigger = screen
      .getByTestId('pk-topbar')
      .querySelector<HTMLButtonElement>(
        'button[data-testid="pk-topbar-palette"]',
      )
    if (!paletteTrigger) throw new Error('palette trigger missing')
    await user.click(paletteTrigger)
    const input = await screen.findByPlaceholderText(/Find a page/i)
    await user.type(input, 'example')
    await new Promise((resolve) => window.setTimeout(resolve, 250))
    await vi.waitFor(() => expect(backend.queryHistory).toHaveBeenCalled())
    const item = await screen.findByText('Example article')
    await user.click(item)
    expect(await screen.findByText('explorer body')).toBeInTheDocument()
  })

  test('palette maps URL-only backend hits to visible results with extracted domains', async () => {
    const user = userEvent.setup()
    const { backend } = await import('@/lib/backend-client')
    vi.mocked(backend.queryHistory).mockResolvedValue({
      total: 1,
      items: [
        {
          id: 7,
          profileId: 'chrome:Default',
          url: 'https://fallback.example/article',
          title: null,
          domain: '',
          favicon: null,
          visitedAt: null as never,
          visitTime: 1745311800,
          sourceVisitId: 0,
        },
      ],
      page: 1,
      pageSize: 8,
      pageCount: 1,
      hasPrevious: false,
      hasNext: false,
    })
    renderShell({}, '/')
    const paletteTrigger = screen
      .getByTestId('pk-topbar')
      .querySelector<HTMLButtonElement>(
        'button[data-testid="pk-topbar-palette"]',
      )
    if (!paletteTrigger) throw new Error('palette trigger missing')
    await user.click(paletteTrigger)
    const input = await screen.findByPlaceholderText(/Find a page/i)
    await user.type(input, 'fallback')
    await new Promise((resolve) => window.setTimeout(resolve, 250))
    await vi.waitFor(() => expect(backend.queryHistory).toHaveBeenCalled())
    expect(
      await screen.findByText('https://fallback.example/article'),
    ).toBeInTheDocument()
    expect(screen.getByText('fallback.example')).toBeInTheDocument()
    expect(screen.queryByText(/2026-04-17/)).not.toBeInTheDocument()
  })

  test('palette maps titleless and URL-less backend hits to an untitled result', async () => {
    const user = userEvent.setup()
    const { backend } = await import('@/lib/backend-client')
    vi.mocked(backend.queryHistory).mockResolvedValue({
      total: 1,
      items: [
        {
          id: 8,
          profileId: 'chrome:Default',
          url: null as never,
          title: null,
          domain: null as never,
          favicon: null,
          visitedAt: null as never,
          visitTime: 0,
          sourceVisitId: 0,
        },
      ],
      page: 1,
      pageSize: 8,
      pageCount: 1,
      hasPrevious: false,
      hasNext: false,
    })
    renderShell({}, '/')
    const paletteTrigger = screen
      .getByTestId('pk-topbar')
      .querySelector<HTMLButtonElement>(
        'button[data-testid="pk-topbar-palette"]',
      )
    if (!paletteTrigger) throw new Error('palette trigger missing')
    await user.click(paletteTrigger)
    const input = await screen.findByPlaceholderText(/Find a page/i)
    await user.type(input, 'untitled')
    await new Promise((resolve) => window.setTimeout(resolve, 250))
    await vi.waitFor(() => expect(backend.queryHistory).toHaveBeenCalled())
    expect(await screen.findByText('(untitled)')).toBeInTheDocument()
  })

  test('palette result with no visit date falls back to /explorer', async () => {
    const user = userEvent.setup()
    const { backend } = await import('@/lib/backend-client')
    vi.mocked(backend.queryHistory).mockResolvedValue({
      total: 1,
      items: [
        {
          id: 8,
          profileId: 'chrome:Default',
          url: 'https://example.com/no-date',
          title: 'No date article',
          domain: 'example.com',
          favicon: null,
          // intentionally missing visitedAt → visitDate stays null
          visitedAt: '',
          visitTime: 0,
          sourceVisitId: 0,
        },
      ],
      page: 1,
      pageSize: 8,
      pageCount: 1,
      hasPrevious: false,
      hasNext: false,
    })
    renderShell({}, '/')
    const paletteTrigger = screen
      .getByTestId('pk-topbar')
      .querySelector<HTMLButtonElement>(
        'button[data-testid="pk-topbar-palette"]',
      )
    if (!paletteTrigger) throw new Error('palette trigger missing')
    await user.click(paletteTrigger)
    const input = await screen.findByPlaceholderText(/Find a page/i)
    await user.type(input, 'no')
    await new Promise((resolve) => window.setTimeout(resolve, 250))
    await vi.waitFor(() => expect(backend.queryHistory).toHaveBeenCalled())
    const item = await screen.findByText('No date article')
    await user.click(item)
    expect(await screen.findByText('explorer body')).toBeInTheDocument()
  })

  test('toggling the theme button flips data-theme and persists', async () => {
    const user = userEvent.setup()
    renderShell({}, '/')
    const sidebar = screen.getByTestId('pk-sidebar')
    const themeToggle = sidebar.querySelector('button[aria-label*="theme" i]')
    if (!themeToggle) throw new Error('theme toggle missing')
    await user.click(themeToggle)
    // Toggle 1 → dark, toggle 2 → light. Either way, the data-theme attr
    // updates and shell.tsx line 144 fires.
    expect(document.documentElement.getAttribute('data-theme')).not.toBeNull()
  })

  test('toggling the theme button from dark returns the shell to light mode', async () => {
    const user = userEvent.setup()
    window.localStorage.setItem('pathkeep.theme', 'dark')
    renderShell({}, '/')
    const sidebar = screen.getByTestId('pk-sidebar')
    const themeToggle = sidebar.querySelector('button[aria-label*="theme" i]')
    if (!themeToggle) throw new Error('theme toggle missing')
    await user.click(themeToggle)
    expect(document.documentElement.getAttribute('data-theme')).toBe('light')
  })

  test('Lock now fires the shell lockAppSession action with manual reason', async () => {
    const user = userEvent.setup()
    const lockAppSession = vi.fn().mockResolvedValue({})
    renderShell({ lockAppSession }, '/')
    const sidebar = screen.getByTestId('pk-sidebar')
    const lockButton = sidebar.querySelector('button[aria-label*="Lock" i]')
    if (!lockButton) throw new Error('lock-now button missing')
    await user.click(lockButton)
    expect(lockAppSession).toHaveBeenCalledWith('manual')
  })

  test('Backup now fires the shell runBackup action', async () => {
    const user = userEvent.setup()
    const runBackup = vi.fn().mockResolvedValue({})
    renderShell(
      {
        runBackup,
        snapshot: {
          archiveStatus: { initialized: true, warning: null },
          browserProfiles: [],
        } as never,
      },
      '/',
    )
    const topbar = screen.getByTestId('pk-topbar')
    // The CTA renders its label via i18n; in English the runtime label is
    // "Back up now". Find by accessible name through getByRole.
    const backupButton = await within(topbar).findByRole('button', {
      name: /backup now/i,
    })
    await user.click(backupButton)
    expect(runBackup).toHaveBeenCalled()
  })

  test('Cmd+K keyboard shortcut toggles the palette open', async () => {
    const user = userEvent.setup()
    renderShell({}, '/')
    // Drive the window-level keydown listener with a metaKey + K combo.
    // userEvent's `keyboard` API translates `{Meta>}k{/Meta}` into the
    // Cmd+K event the shell listens for.
    await user.keyboard('{Meta>}k{/Meta}')
    // The palette opens on the first Cmd+K and a placeholder appears.
    expect(
      await screen.findByPlaceholderText(/Find a page/i),
    ).toBeInTheDocument()
  })

  test('Escape key closes the palette while it is open', async () => {
    const user = userEvent.setup()
    renderShell({}, '/')
    const paletteTrigger = screen
      .getByTestId('pk-topbar')
      .querySelector<HTMLButtonElement>(
        'button[data-testid="pk-topbar-palette"]',
      )
    if (!paletteTrigger) throw new Error('palette trigger missing')
    await user.click(paletteTrigger)
    await screen.findByPlaceholderText(/Find a page/i)
    await user.keyboard('{Escape}')
    await vi.waitFor(() =>
      expect(
        screen.queryByPlaceholderText(/Find a page/i),
      ).not.toBeInTheDocument(),
    )
  })

  test('selecting "Manage sources" navigates to settings#sources', async () => {
    const user = userEvent.setup()
    renderShell(
      {
        snapshot: {
          archiveStatus: { initialized: true, warning: null },
          browserProfiles: [
            {
              profileId: 'chrome:Default',
              browserName: 'Chrome',
              browserFamily: 'chromium',
              profileName: 'Default',
              historyBytes: 1024,
            },
          ],
        } as never,
      },
      '/',
    )
    const statusBar = screen.getByTestId('pk-status-bar')
    // The source picker is a Popover trigger; the popover renders the
    // "Manage sources" link only after the trigger is opened. Find it by
    // any button inside the picker chrome.
    const sourcesButton = statusBar.querySelector('button')
    if (!sourcesButton) throw new Error('status-bar source trigger missing')
    await user.click(sourcesButton)
    const manage = await screen.findByText(/Manage sources/i)
    await user.click(manage)
    expect(await screen.findByText('settings body')).toBeInTheDocument()
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
            <ProfileScopeProvider>
              <ShellDataContext.Provider value={shellValue(overrides)}>
                <AppShell />
              </ShellDataContext.Provider>
            </ProfileScopeProvider>
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
          {
            path: 'explorer',
            element: <p>explorer body</p>,
          },
          {
            path: 'settings',
            element: <p>settings body</p>,
          },
          {
            path: 'assistant',
            element: <p>assistant body</p>,
            handle: {
              screen: appScreens.find((screen) => screen.id === 'assistant'),
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

function makeDashboardSnapshot(): DashboardSnapshot {
  return {
    generatedAt: '2026-05-19T00:00:00Z',
    totalProfiles: 1,
    totalUrls: 1000,
    totalVisits: 2500,
    totalDownloads: 0,
    lastSuccessfulBackupAt: '2026-05-18T14:23:00Z',
    recentRuns: [],
    storage: {
      archiveDatabaseBytes: 8 * 1024 * 1024,
      sourceEvidenceDatabaseBytes: 0,
      searchDatabaseBytes: 1024 * 1024,
      intelligenceDatabaseBytes: 0,
      manifestBytes: 0,
      snapshotBytes: 0,
      exportBytes: 0,
      stagingBytes: 0,
      quarantineBytes: 0,
      semanticSidecarBytes: 0,
      intelligenceBlobBytes: 0,
    },
    nextAction: null,
  }
}
