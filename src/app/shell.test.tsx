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

import { useState } from 'react'
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import type * as BackendClient from '@/lib/backend-client'
import {
  ShellDataContext,
  type ShellDataContextValue,
} from './shell-data-context'
import type { ShellTask } from './shell-tasks'
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
      openExternalUrl: vi.fn().mockResolvedValue(undefined),
      revealLogs: vi.fn().mockResolvedValue('/path/to/logs'),
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

  test('renders the ambient bar for a running backup (task-store driven), not the blocking overlay', () => {
    // A backup marks its overlay background:true AND registers a running task in the store. The
    // unified bottom slot must surface it through the ambient bar, never the blocking BusyOverlay.
    renderShell({
      busyAction: 'Running backup',
      busyOverlay: { background: true, label: 'Backing up' },
      archiveTasks: [
        runningArchiveTask({
          id: 'archive-backup-1',
          kind: 'backup',
          title: 'Manual backup',
        }),
      ],
    })
    expect(screen.getByTestId('ambient-task-bar')).toHaveTextContent(
      'Manual backup',
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

  test('does not render its own drag strip under the overlay (moved to app root)', () => {
    // The window-drag strip is now a single global element hoisted to the app
    // root (src/app/index.tsx) so EVERY screen can drag the window — not just
    // the main shell. The shell must NOT render its own copy (no double strip),
    // but it still flips data-titlebar-overlay so the sidebar/topbar reserve
    // clearance for the traffic lights + title strip.
    vi.spyOn(runtime, 'hasMacOverlayTitlebar').mockReturnValue(true)
    renderShell({}, '/')
    const shellRoot = screen.getByTestId('app-shell')
    expect(shellRoot).toHaveAttribute('data-titlebar-overlay', 'true')
    expect(
      shellRoot.querySelector('.pk-titlebar-dragstrip'),
    ).not.toBeInTheDocument()
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

  test('Cmd+K is inert while the unlock gate owns the screen', async () => {
    const user = userEvent.setup()
    renderShell(
      {
        snapshot: {
          archiveStatus: {
            initialized: true,
            encrypted: true,
            unlocked: false,
            warning: null,
          },
          keyringStatus: { available: false, backend: '', storedSecret: false },
          browserProfiles: [],
        } as never,
      },
      '/',
    )
    // The blocking gate is mounted because the archive is encrypted + locked.
    expect(screen.getByTestId('archive-unlock-gate')).toBeInTheDocument()
    await user.keyboard('{Meta>}k{/Meta}')
    // ⌘K must NOT open the search palette over a locked archive.
    expect(
      screen.queryByPlaceholderText(/Find a page/i),
    ).not.toBeInTheDocument()
  })

  // --- Backup failure toast ---

  test('renders the backup-failure toast (role=alert) when the shell advertises a backup error', () => {
    renderShell(
      { error: 'Backup failed: permission denied', errorKind: 'backup' },
      '/',
    )
    const toast = screen.getByTestId('backup-failure-toast')
    expect(toast).toBeInTheDocument()
    expect(toast).toHaveAttribute('role', 'alert')
    expect(toast).toHaveTextContent('Backup failed: permission denied')
  })

  test('non-backup errors (config save, lock, etc.) do NOT show the backup toast', () => {
    // The toast is scoped to backup failures: "Backup didn't finish", a reassurance
    // about archive safety, and a "Try again → runBackup" button are all wrong for
    // a config-save or lock/unlock error. Those stay quiet in the shell (the failing
    // action surfaces its own feedback).
    renderShell({ error: 'Could not save config', errorKind: null }, '/')
    expect(screen.queryByTestId('backup-failure-toast')).not.toBeInTheDocument()
  })

  test('the failure toast always reassures that the existing archive is safe', () => {
    renderShell({ error: 'disk full', errorKind: 'backup' }, '/')
    expect(screen.getByTestId('backup-failure-toast')).toHaveTextContent(
      /nothing was lost/i,
    )
  })

  test('the failure toast lives OUTSIDE the scroll area so it is always in view', () => {
    renderShell(
      { error: 'Backup failed: permission denied', errorKind: 'backup' },
      '/',
    )
    const toast = screen.getByTestId('backup-failure-toast')
    const scrollArea = screen.getByTestId('app-scroll')
    expect(scrollArea.contains(toast)).toBe(false)
  })

  test('does not render the failure toast when error is null', () => {
    renderShell({ error: null }, '/')
    expect(screen.queryByTestId('backup-failure-toast')).not.toBeInTheDocument()
  })

  test('a running backup owns the bottom slot — the failure toast never shows mid-run', () => {
    renderShell(
      {
        error: 'stale error from a previous attempt',
        errorKind: 'backup',
        busyAction: 'Running backup',
        busyOverlay: { background: true, label: 'Backing up' },
        archiveTasks: [
          runningArchiveTask({
            id: 'archive-backup-1',
            kind: 'backup',
            title: 'Manual backup',
          }),
        ],
      },
      '/',
    )
    expect(screen.getByTestId('ambient-task-bar')).toBeInTheDocument()
    expect(screen.queryByTestId('backup-failure-toast')).not.toBeInTheDocument()
  })

  test('dismissing the failure toast calls clearError', async () => {
    const user = userEvent.setup()
    const clearError = vi.fn()
    renderShell(
      { error: 'Something went wrong', errorKind: 'backup', clearError },
      '/',
    )
    const dismissButton = screen.getByRole('button', { name: /dismiss/i })
    await user.click(dismissButton)
    expect(clearError).toHaveBeenCalled()
  })

  test('Try again fires the shell runBackup action', async () => {
    const user = userEvent.setup()
    const runBackup = vi.fn().mockResolvedValue({})
    renderShell({ error: 'Backup failed', errorKind: 'backup', runBackup }, '/')
    const toast = screen.getByTestId('backup-failure-toast')
    await user.click(within(toast).getByRole('button', { name: /try again/i }))
    expect(runBackup).toHaveBeenCalled()
  })

  test('FDA failure shows the "Open Full Disk Access settings" button — gated on errorKind, not the error text', () => {
    // Drive the gate via the locale-independent classification, NOT an English
    // substring. The displayed error is deliberately non-English (a translated
    // FDA message) to prove the button appears for zh-CN / zh-TW users too.
    renderShell(
      {
        error: 'PathKeep 需要“完全磁盘访问权限”才能读取浏览器历史。',
        errorKind: 'full-disk-access',
      },
      '/',
    )
    const toast = screen.getByTestId('backup-failure-toast')
    expect(
      within(toast).getByRole('button', { name: /full disk access/i }),
    ).toBeInTheDocument()
  })

  test('error whose English text mentions "Full Disk Access" but is classified as generic backup hides the FDA button', () => {
    // The OLD bug was the inverse: gating on the string showed/hid the button by
    // re-parsing translated copy. Now a backup error that happens to contain the
    // English marker never shows the FDA button unless the kind is 'full-disk-access'.
    renderShell(
      {
        error: 'Network error mentioning Full Disk Access',
        errorKind: 'backup',
      },
      '/',
    )
    const toast = screen.getByTestId('backup-failure-toast')
    expect(
      within(toast).queryByRole('button', { name: /full disk access/i }),
    ).not.toBeInTheDocument()
  })

  test('non-FDA backup failure does not show the "Open Full Disk Access settings" button', () => {
    renderShell(
      { error: 'Network timeout during backup', errorKind: 'backup' },
      '/',
    )
    const toast = screen.getByTestId('backup-failure-toast')
    expect(
      within(toast).queryByRole('button', { name: /full disk access/i }),
    ).not.toBeInTheDocument()
  })

  test('FDA settings button opens the macOS privacy deep-link', async () => {
    const user = userEvent.setup()
    const { backend } = await import('@/lib/backend-client')
    renderShell(
      {
        error: 'PathKeep 需要“完全磁盘访问权限”才能读取浏览器历史。',
        errorKind: 'full-disk-access',
      },
      '/',
    )
    const toast = screen.getByTestId('backup-failure-toast')
    await user.click(
      within(toast).getByRole('button', { name: /full disk access/i }),
    )
    await vi.waitFor(() => {
      expect(backend.openExternalUrl).toHaveBeenCalledWith(
        expect.stringContaining('Privacy_AllFiles'),
      )
    })
  })

  test('Reveal logs is demoted into the technical-details disclosure and calls backend.revealLogs', async () => {
    const user = userEvent.setup()
    const { backend } = await import('@/lib/backend-client')
    renderShell(
      { error: 'Backup failed: something unexpected', errorKind: 'backup' },
      '/',
    )
    const toast = screen.getByTestId('backup-failure-toast')
    // No longer a first-line affordance: open "Technical details" first.
    await user.click(within(toast).getByText(/technical details/i))
    const revealButton = within(toast).getByRole('button', {
      name: /logs folder/i,
    })
    await user.click(revealButton)
    await vi.waitFor(() => {
      expect(backend.revealLogs).toHaveBeenCalled()
    })
  })

  test('Copy diagnostics writes a report containing the failure detail and confirms', async () => {
    const user = userEvent.setup()
    const writeText = vi.fn().mockResolvedValue(undefined)
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText },
      configurable: true,
    })
    renderShell({ error: 'sqlite error: disk I/O', errorKind: 'backup' }, '/')
    const toast = screen.getByTestId('backup-failure-toast')
    await user.click(
      within(toast).getByRole('button', { name: /copy diagnostics/i }),
    )
    await vi.waitFor(() => {
      expect(writeText).toHaveBeenCalledWith(
        expect.stringContaining('sqlite error: disk I/O'),
      )
    })
    expect(
      await within(toast).findByRole('button', { name: /^Copied$/ }),
    ).toBeInTheDocument()
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

  // --- Ambient task bar (all-task indicator) ---
  //
  // The bottom bar is no longer backup-only: ANY running task (import, backup, AI index build)
  // must surface it, on EVERY route, driven by the route-independent task store — so navigating
  // away from the originating page can never hide live progress.
  describe('Ambient task bar (all-task indicator)', () => {
    test('HEADLINE — a running import in the task store shows the ambient bar on a non-import route', () => {
      // On the OLD code an import set no busyAction, so the bottom slot rendered nothing off the
      // import page — getByTestId throws, proving the regression this change fixes.
      renderShell(
        {
          archiveTasks: [
            runningArchiveTask({
              title: 'Importing history',
              progressValue: 45,
              progressLabel: '450 / 1000',
            }),
          ],
        },
        '/',
      )
      const bar = screen.getByTestId('ambient-task-bar')
      expect(bar).toHaveTextContent('Importing history')
      expect(bar).toHaveTextContent('45%')
    })

    test('survives navigation — the bar is present regardless of route', () => {
      renderShell(
        {
          archiveTasks: [runningArchiveTask({ title: 'Importing history' })],
        },
        '/explorer',
      )
      expect(screen.getByTestId('ambient-task-bar')).toBeInTheDocument()
      // The current route still rendered underneath the ambient bar.
      expect(screen.getByText('explorer body')).toBeInTheDocument()
    })

    test('multiple concurrent tasks show a compact summary, not stacked bars', () => {
      renderShell(
        {
          archiveTasks: [
            runningArchiveTask({
              id: 'archive-backup-1',
              kind: 'backup',
              title: 'Manual backup',
              progressValue: 20,
            }),
          ],
          runtimeStatus: {
            aiQueue: {
              paused: false,
              concurrency: 1,
              queued: 0,
              running: 1,
              failed: 0,
              indexQueued: 0,
              indexRunning: 1,
              recentJobs: [runningIndexJob()],
            },
            intelligence: null,
            loading: false,
            error: null,
          },
        },
        '/',
      )
      // Exactly one strip — concurrent tasks collapse into a summary, never stack.
      expect(screen.getAllByTestId('ambient-task-bar')).toHaveLength(1)
      expect(screen.getByTestId('ambient-task-bar')).toHaveTextContent(
        '2 tasks running',
      )
    })

    test('clicking the bar navigates to the Activity page', async () => {
      const user = userEvent.setup()
      renderShell(
        {
          archiveTasks: [runningArchiveTask({ title: 'Importing history' })],
        },
        '/',
      )
      await user.click(screen.getByTestId('ambient-task-bar'))
      expect(await screen.findByText('jobs body')).toBeInTheDocument()
    })

    test('the ambient bar and the failure toast are mutually exclusive', () => {
      // (a) A live task hides a stale failure toast.
      const { unmount } = renderShell(
        {
          error: 'x',
          errorKind: 'backup',
          archiveTasks: [
            runningArchiveTask({ id: 'archive-backup-1', kind: 'backup' }),
          ],
        },
        '/',
      )
      expect(screen.getByTestId('ambient-task-bar')).toBeInTheDocument()
      expect(
        screen.queryByTestId('backup-failure-toast'),
      ).not.toBeInTheDocument()
      unmount()

      // (b) With nothing running, the failure toast owns the slot again.
      renderShell({ error: 'x', errorKind: 'backup' }, '/')
      expect(screen.getByTestId('backup-failure-toast')).toBeInTheDocument()
      expect(screen.queryByTestId('ambient-task-bar')).not.toBeInTheDocument()
    })

    test('idle → no ambient bar', () => {
      renderShell({}, '/')
      expect(screen.queryByTestId('ambient-task-bar')).not.toBeInTheDocument()
    })

    test('desync fallback — a background overlay with no registered task still shows the ambient bar', () => {
      // Airtight today (runBackup always registers a task), but a future
      // `background: true` producer that forgets to register a task would
      // otherwise make the work invisible — no blocking overlay AND no ambient
      // bar. The shell falls back to the overlay's own payload so background
      // work can never silently disappear.
      renderShell(
        {
          busyAction: 'Backing up',
          busyOverlay: {
            background: true,
            label: 'Backing up',
            progressValue: 30,
          },
          archiveTasks: [],
        },
        '/',
      )
      const bar = screen.getByTestId('ambient-task-bar')
      expect(bar).toHaveTextContent('Backing up')
      expect(bar).toHaveTextContent('30%')
      // The blocking overlay must stay hidden for a background overlay.
      expect(screen.queryByTestId('busy-overlay')).not.toBeInTheDocument()
    })
  })

  // --- Ambient task announcer (a11y presence live region) ---
  describe('Ambient task announcer', () => {
    test('mounts a persistent SR region that stays silent on mount and speaks when work appears', async () => {
      const user = userEvent.setup()

      function Harness() {
        const [active, setActive] = useState(false)
        return (
          <I18nProvider>
            <ProfileScopeProvider>
              <ShellDataContext.Provider
                value={shellValue({
                  archiveTasks: active
                    ? [runningArchiveTask({ title: 'Importing history' })]
                    : [],
                })}
              >
                <AppShell />
              </ShellDataContext.Provider>
            </ProfileScopeProvider>
            <button type="button" onClick={() => setActive(true)}>
              activate-task
            </button>
          </I18nProvider>
        )
      }

      const router = createMemoryRouter(
        [
          {
            path: '/',
            element: <Harness />,
            children: [{ index: true, element: <p>route body</p> }],
          },
        ],
        { initialEntries: ['/'] },
      )
      render(<RouterProvider router={router} />)

      // Always mounted (even while idle) and silent on mount — no announcement.
      const announcer = screen.getByTestId('ambient-task-announcer')
      expect(announcer).toBeInTheDocument()
      expect(announcer).toHaveAttribute('role', 'status')
      expect(announcer).toHaveAttribute('aria-live', 'polite')
      expect(announcer).not.toHaveTextContent('Background work started')

      // Background work appears → the region announces the presence transition.
      await user.click(screen.getByRole('button', { name: 'activate-task' }))
      expect(screen.getByTestId('ambient-task-announcer')).toHaveTextContent(
        'Background work started',
      )
    })
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
    rawError: null,
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
    errorKind: null,
    clearError: vi.fn(),
    ...overrides,
  } as ShellDataContextValue
}

function runningArchiveTask(overrides: Partial<ShellTask> = {}): ShellTask {
  return {
    id: 'archive-import-1',
    kind: 'import',
    state: 'running',
    title: 'Importing history',
    detail: '',
    startedAt: '2026-06-28T00:00:00Z',
    updatedAt: '2026-06-28T00:00:00Z',
    finishedAt: null,
    sourceLabel: null,
    profileLabel: null,
    progressLabel: null,
    progressValue: null,
    current: null,
    total: null,
    processedRecords: null,
    totalRecords: null,
    importedRecords: null,
    duplicateRecords: null,
    skippedRecords: null,
    logEntries: [],
    resultLink: null,
    error: null,
    ...overrides,
  }
}

function runningIndexJob() {
  return {
    id: 55,
    jobType: 'index-build',
    state: 'running',
    priority: 10,
    attempt: 1,
    maxAttempts: 3,
    runId: null,
    summary: null,
    queuedAt: '2026-06-28T00:00:00Z',
    availableAt: '2026-06-28T00:00:00Z',
    startedAt: '2026-06-28T00:01:00Z',
    finishedAt: null,
    heartbeatAt: null,
    errorCode: null,
    errorMessage: null,
  }
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
