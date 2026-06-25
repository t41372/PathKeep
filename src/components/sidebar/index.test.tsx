/**
 * This test file protects the shared Sidebar component contract.
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
import { render, screen } from '@testing-library/react'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { ShellDataProvider } from '../../app/shell-data'
import {
  ShellDataContext,
  type ShellDataContextValue,
} from '../../app/shell-data-context'
import { backend } from '../../lib/backend-client'
import { backendTestHarness } from '../../lib/backend'
import { I18nProvider } from '../../lib/i18n'
import { ProfileScopeProvider } from '../../lib/profile-scope'
import { ProfileScopeContext } from '../../lib/profile-scope-context'
import type { AppScreen } from '../../app/router'
import { Sidebar } from './index'
import { SidebarNavItem } from './nav-item'

describe('Sidebar', () => {
  beforeEach(() => {
    backendTestHarness.reset()
    vi.restoreAllMocks()
  })

  test('renders the product name, sections, and archive status', async () => {
    const user = userEvent.setup()
    const router = createMemoryRouter(
      [
        {
          path: '/',
          element: <Sidebar collapsed={false} onToggle={() => {}} />,
        },
      ],
      { initialEntries: ['/'] },
    )

    const { container } = render(
      <I18nProvider>
        <ProfileScopeProvider>
          <ShellDataProvider>
            <RouterProvider router={router} />
          </ShellDataProvider>
        </ProfileScopeProvider>
      </I18nProvider>,
    )

    expect(screen.getByText('PATHKEEP')).toBeVisible()
    expect(screen.getByText('CORE')).toBeVisible()
    expect(screen.getByRole('link', { name: 'Dashboard' })).toHaveClass(
      'nav-item',
    )
    expect(screen.getByRole('link', { name: 'Dashboard' })).toHaveClass(
      'nav-item--active',
    )
    expect(container.querySelector('.nav-icon .glyph')).not.toBeNull()
    expect(await screen.findByText('Archive not initialized')).toBeVisible()
    expect(await screen.findByText('Encrypted archive')).toBeVisible()
    expect(screen.getByText('0 B')).toBeVisible()
    expect(screen.getByText('Profile scope: All profiles')).toBeVisible()

    document.documentElement.setAttribute('data-theme', 'light')
    await user.click(screen.getByRole('button', { name: 'Toggle theme' }))
    expect(document.documentElement).toHaveAttribute('data-theme', 'dark')
    expect(window.localStorage.getItem('pathkeep.theme')).toBe('dark')

    await user.click(screen.getByRole('button', { name: 'Toggle theme' }))
    expect(document.documentElement).toHaveAttribute('data-theme', 'light')
    expect(window.localStorage.getItem('pathkeep.theme')).toBe('light')
  })

  test('renders the assistant nav item without a roadmap badge', () => {
    const router = createMemoryRouter(
      [
        {
          path: '/',
          element: <Sidebar collapsed={false} onToggle={() => {}} />,
        },
      ],
      { initialEntries: ['/'] },
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

    // AI is now a shipped (consent-gated) feature, so the assistant nav entry no
    // longer advertises a "v0.3" roadmap badge.
    expect(screen.getByRole('link', { name: 'AI Assistant' })).toBeVisible()
    expect(screen.queryByText('v0.3')).toBeNull()
    expect(document.querySelector('.nav-badge')).toBeNull()
  })

  test('renders a nav badge when a screen still carries a badgeKey', () => {
    const screenWithBadge: AppScreen = {
      id: 'assistant',
      labelKey: 'navigation.assistantLabel',
      titleKey: 'navigation.assistantTitle',
      subtitleKey: 'navigation.assistantSubtitle',
      icon: 'smart_toy',
      href: '/assistant',
      badgeKey: 'navigation.assistantLabel',
      section: 'CORE',
    }
    const router = createMemoryRouter(
      [
        {
          path: '/',
          element: (
            <SidebarNavItem collapsed={false} screen={screenWithBadge} />
          ),
        },
      ],
      { initialEntries: ['/'] },
    )

    render(
      <I18nProvider>
        <RouterProvider router={router} />
      </I18nProvider>,
    )

    // The badge-rendering branch stays exercised even though no shipped screen
    // currently uses it: the badge resolves its i18n key and is hidden when
    // the rail is collapsed.
    const badge = document.querySelector('.nav-badge')
    expect(badge).not.toBeNull()
    expect(badge?.textContent).toBe('AI Assistant')
  })

  test('keeps the root link inactive when another route is selected', () => {
    const router = createMemoryRouter(
      [
        {
          path: '*',
          element: <Sidebar collapsed={false} onToggle={() => {}} />,
        },
      ],
      { initialEntries: ['/explorer'] },
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

    expect(screen.getByRole('link', { name: 'Dashboard' })).not.toHaveClass(
      'nav-item--active',
    )
    expect(screen.getByRole('link', { name: 'Explorer' })).toHaveClass(
      'nav-item--active',
    )
  })

  test('keeps navigation accessible when the sidebar is collapsed', async () => {
    const router = createMemoryRouter(
      [
        {
          path: '/',
          element: <Sidebar collapsed onToggle={() => {}} />,
        },
      ],
      { initialEntries: ['/'] },
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

    expect(screen.getByRole('link', { name: 'Dashboard' })).toBeVisible()
    expect(await screen.findByLabelText('Expand navigation')).toBeVisible()
    expect(screen.getByText('PATHKEEP')).toHaveClass('logo-name')
    expect(screen.getByText('Dashboard')).toHaveAttribute('aria-hidden', 'true')
  })

  test('shows a locked archive status instead of pretending the archive is uninitialized', () => {
    const router = createMemoryRouter(
      [
        {
          path: '/',
          element: <Sidebar collapsed={false} onToggle={() => {}} />,
        },
      ],
      { initialEntries: ['/'] },
    )
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
      errorKind: null,
      clearError: vi.fn(),
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

    expect(screen.getByText('Archive attention needed')).toBeVisible()
    expect(screen.getByText('Encrypted / Locked')).toBeVisible()
    expect(
      screen.queryByText('Archive not initialized'),
    ).not.toBeInTheDocument()
  })

  test('keeps archive warnings and active profile names visible in the footer', async () => {
    const snapshot = await backend.getAppSnapshot()
    snapshot.config.initialized = true
    snapshot.archiveStatus.initialized = true
    snapshot.archiveStatus.warning = 'low disk headroom'
    snapshot.archiveStatus.encrypted = false
    snapshot.browserProfiles = [
      {
        profileId: 'chrome:Default',
        profileName: 'Personal research',
      },
    ] as typeof snapshot.browserProfiles
    const shellValue: ShellDataContextValue = {
      buildInfo: null,
      appLockStatus: snapshot.appLockStatus,
      snapshot,
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
      errorKind: null,
      clearError: vi.fn(),
    }
    const router = createMemoryRouter(
      [
        {
          path: '/',
          element: <Sidebar collapsed={false} onToggle={() => {}} />,
        },
      ],
      { initialEntries: ['/'] },
    )

    render(
      <I18nProvider>
        <ProfileScopeContext.Provider
          value={{
            activeProfileId: 'chrome:Default',
            setActiveProfileId: vi.fn(),
          }}
        >
          <ShellDataContext.Provider value={shellValue}>
            <RouterProvider router={router} />
          </ShellDataContext.Provider>
        </ProfileScopeContext.Provider>
      </I18nProvider>,
    )

    expect(screen.getByText('Archive attention needed')).toBeVisible()
    expect(screen.getByText('Plaintext archive')).toBeVisible()
    expect(screen.getByText('Profile scope: Personal research')).toBeVisible()
  })

  test('falls back to readable profile id labels when active metadata is stale', async () => {
    const snapshot = await backend.getAppSnapshot()
    snapshot.config.initialized = true
    snapshot.archiveStatus.initialized = true
    snapshot.browserProfiles = []
    const shellValue: ShellDataContextValue = {
      buildInfo: null,
      appLockStatus: snapshot.appLockStatus,
      snapshot,
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
      errorKind: null,
      clearError: vi.fn(),
    }
    const router = createMemoryRouter(
      [
        {
          path: '/',
          element: <Sidebar collapsed={false} onToggle={() => {}} />,
        },
      ],
      { initialEntries: ['/'] },
    )

    render(
      <I18nProvider>
        <ProfileScopeContext.Provider
          value={{
            activeProfileId: 'safari:Archived',
            setActiveProfileId: vi.fn(),
          }}
        >
          <ShellDataContext.Provider value={shellValue}>
            <RouterProvider router={router} />
          </ShellDataContext.Provider>
        </ProfileScopeContext.Provider>
      </I18nProvider>,
    )

    expect(screen.getByText('Profile scope: Archived')).toBeVisible()
  })

  test('shows the compact build revision and routes background work toward Security while locked', async () => {
    const loadAiQueueStatusSpy = vi.spyOn(backend, 'loadAiQueueStatus')
    const loadIntelligenceRuntimeSpy = vi.spyOn(
      backend,
      'loadIntelligenceRuntime',
    )
    const snapshot = await backend.getAppSnapshot()
    snapshot.config.initialized = true
    snapshot.archiveStatus.initialized = true
    snapshot.archiveStatus.encrypted = true
    snapshot.archiveStatus.unlocked = false
    snapshot.archiveStatus.warning =
      'database key is required for encrypted archives'
    const shellValue: ShellDataContextValue = {
      buildInfo: {
        productName: 'PathKeep',
        version: '0.1.0',
        gitCommitShort: 'test123',
        gitCommitFull: 'test1234567890',
        gitDirty: true,
      },
      appLockStatus: snapshot.appLockStatus,
      snapshot,
      dashboard: null,
      loading: false,
      busyAction: null,
      busyOverlay: null,
      error: 'database key is required for encrypted archives',
      notice: null,
      refreshKey: 1,
      refreshAppData: vi.fn().mockResolvedValue(undefined),
      refreshRuntimeStatus: vi.fn().mockResolvedValue({
        aiQueue: null,
        intelligence: null,
        loading: false,
        error: null,
      }),
      saveConfig: vi.fn().mockResolvedValue(snapshot),
      initializeArchive: vi.fn().mockResolvedValue(snapshot),
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
      errorKind: null,
      clearError: vi.fn(),
    }

    const router = createMemoryRouter(
      [
        {
          path: '/',
          element: <Sidebar collapsed={false} onToggle={() => {}} />,
        },
      ],
      { initialEntries: ['/'] },
    )

    render(
      <I18nProvider>
        <ProfileScopeProvider>
          <ShellDataContext.Provider value={shellValue}>
            <RouterProvider router={router} />
          </ShellDataContext.Provider>
        </ProfileScopeProvider>
      </I18nProvider>,
    )

    expect(screen.getByText('v0.1.0 · test123+')).toBeVisible()
    expect(screen.getByText('Unlock the archive first')).toBeVisible()
    expect(
      screen.getByText('Open Security before reviewing queued work.'),
    ).toBeVisible()
    expect(
      screen.getAllByRole('link', { name: 'Security' })[1],
    ).toHaveAttribute('href', '/security#unlock-archive')
    expect(loadAiQueueStatusSpy).not.toHaveBeenCalled()
    expect(loadIntelligenceRuntimeSpy).not.toHaveBeenCalled()
  })

  test('shows compact background work status in the footer', async () => {
    const snapshot = await backend.getAppSnapshot()
    snapshot.config.initialized = true
    snapshot.archiveStatus.initialized = true
    snapshot.archiveStatus.unlocked = true
    snapshot.archiveStatus.warning = null
    const shellValue: ShellDataContextValue = {
      buildInfo: {
        productName: 'PathKeep',
        version: '0.1.0',
        gitCommitShort: 'test123',
        gitCommitFull: 'test1234567890',
        gitDirty: false,
      },
      appLockStatus: snapshot.appLockStatus,
      snapshot,
      dashboard: null,
      dashboardLoading: false,
      runtimeStatus: {
        aiQueue: {
          paused: false,
          concurrency: 2,
          queued: 2,
          running: 1,
          failed: 0,
          indexQueued: 2,
          indexRunning: 1,
          recentJobs: [],
        },
        intelligence: {
          queue: {
            queued: 1,
            running: 1,
            failed: 0,
            succeeded: 0,
            cancelled: 0,
            lastActivityAt: '2026-04-11T08:00:00.000Z',
          },
          plugins: [],
          modules: [],
          recentJobs: [
            {
              id: 900,
              jobType: 'deterministic-rebuild',
              pluginId: null,
              state: 'running',
              historyId: null,
              profileId: 'chrome:Default',
              url: null,
              title: 'chrome:Default · 30 days',
              attempt: 1,
              createdAt: '2026-04-11T07:59:00.000Z',
              startedAt: '2026-04-11T08:00:00.000Z',
              finishedAt: null,
              updatedAt: '2026-04-11T08:01:00.000Z',
              heartbeatAt: '2026-04-11T08:01:00.000Z',
              progressLabel: 'Scoring visits',
              progressDetail: '24,000 / 64,781 visits',
              progressCurrent: 24000,
              progressTotal: 64781,
              progressPercent: 46.8,
              lastError: null,
              retryable: false,
              cancellable: true,
            },
          ],
          notes: [],
        },
        loading: false,
        error: null,
      },
      loading: false,
      busyAction: null,
      busyOverlay: null,
      error: null,
      notice: null,
      refreshKey: 1,
      refreshAppData: vi.fn().mockResolvedValue(undefined),
      refreshRuntimeStatus: vi.fn().mockResolvedValue({
        aiQueue: null,
        intelligence: null,
        loading: false,
        error: null,
      }),
      saveConfig: vi.fn().mockResolvedValue(snapshot),
      initializeArchive: vi.fn().mockResolvedValue(snapshot),
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
      errorKind: null,
      clearError: vi.fn(),
    }

    const router = createMemoryRouter(
      [
        {
          path: '/',
          element: <Sidebar collapsed={false} onToggle={() => {}} />,
        },
      ],
      { initialEntries: ['/'] },
    )

    render(
      <I18nProvider>
        <ProfileScopeProvider>
          <ShellDataContext.Provider value={shellValue}>
            <RouterProvider router={router} />
          </ShellDataContext.Provider>
        </ProfileScopeProvider>
      </I18nProvider>,
    )

    expect(await screen.findByText('Background work')).toBeVisible()
    expect(await screen.findByText('2 running · 3 queued')).toBeVisible()
    expect(screen.getByText('24,000 / 64,781 visits')).toBeVisible()
    expect(screen.getAllByRole('link', { name: 'Jobs' })[1]).toHaveAttribute(
      'href',
      '/jobs',
    )
  })
})
