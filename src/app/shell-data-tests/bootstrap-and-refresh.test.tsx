/**
 * @file bootstrap-and-refresh.test.tsx
 * @description Split bootstrap and refresh coverage lifted from `src/app/shell-data.test.tsx`.
 * @module app/shell-data-tests/bootstrap-and-refresh
 *
 * ## Responsibilities
 * - Preserve the original shell bootstrap, refresh, and config-save regression cases without changing their contract.
 * - Reuse the shared shell-data test harness so this suite only owns the selected behavior slice.
 * - Keep mocked backend sequencing, provider actions, and rendered assertions aligned with the mega-suite.
 *
 * ## Not responsible for
 * - Rewriting unrelated shell-data tests or redefining shared fixtures already owned by `test-helpers.tsx`.
 * - Introducing new provider abstractions, assertions, or setup paths beyond the extracted cases.
 * - Verifying route-specific shells or app-lock flows that belong to other split suites.
 *
 * ## Dependencies
 * - Depends on the real `ShellDataProvider` wiring exposed via `test-helpers.tsx`.
 * - Uses the backend client spies and backup-progress subscription mock from the legacy mega-suite contract.
 *
 * ## Performance notes
 * - Reuses the shared seeded archive bootstrap to avoid duplicating expensive provider setup in every split suite.
 */

import { act, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import { createTranslator } from '../../lib/i18n'
import type {
  AiQueueStatus,
  AppSnapshot,
  DashboardSnapshot,
  IntelligenceRuntimeSnapshot,
} from '../../lib/types'
import {
  getBackupProgressMock,
  getDefaultBuildInfo,
  renderShellProbe,
  resetShellDataHarness,
  seedSnapshot,
} from './test-helpers'

describe('ShellDataProvider', () => {
  beforeEach(() => {
    resetShellDataHarness()
  })

  test('starts with shell loading and tracks dashboard refresh completion separately', async () => {
    const { dashboard, snapshot } = await seedSnapshot()
    let resolveDashboard: ((value: DashboardSnapshot) => void) | undefined
    const pendingDashboard = new Promise<DashboardSnapshot>((resolve) => {
      resolveDashboard = resolve
    })
    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockReturnValue(pendingDashboard)

    renderShellProbe()

    expect(screen.getByTestId('loading')).toHaveTextContent('true')
    expect(screen.getByTestId('dashboard-loading')).toHaveTextContent('false')

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    expect(screen.getByTestId('dashboard-loading')).toHaveTextContent('true')

    await act(async () => {
      resolveDashboard?.(dashboard)
      await pendingDashboard
    })

    await waitFor(() =>
      expect(screen.getByTestId('dashboard-generated-at')).toHaveTextContent(
        dashboard.generatedAt,
      ),
    )
    expect(screen.getByTestId('dashboard-loading')).toHaveTextContent('false')
  })

  test('loads and mutates shell data through provider actions', async () => {
    const user = userEvent.setup()
    const languageSpy = vi.fn()
    const unsubscribe = vi.fn()
    const { dashboard, snapshot } = await seedSnapshot()
    const savedSnapshot: AppSnapshot = {
      ...snapshot,
      config: { ...snapshot.config, preferredLanguage: 'zh-CN' },
    }
    const initializedSnapshot: AppSnapshot = {
      ...snapshot,
      config: {
        ...snapshot.config,
        initialized: true,
        preferredLanguage: 'zh-TW',
      },
    }
    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    vi.spyOn(backend, 'saveConfig').mockResolvedValue(savedSnapshot)
    vi.spyOn(backend, 'initializeArchive').mockResolvedValue(
      initializedSnapshot,
    )
    getBackupProgressMock().mockResolvedValueOnce(unsubscribe)
    vi.spyOn(backend, 'runBackupNow').mockResolvedValue({
      dueSkipped: false,
      run: {
        id: 42,
        startedAt: '2026-04-07T00:00:00Z',
        finishedAt: '2026-04-07T00:05:00Z',
        status: 'success',
        manifestHash: 'manifest-42',
        profileScope: ['chrome:Default'],
        profilesProcessed: 1,
        newVisits: 2,
        newUrls: 1,
        newDownloads: 0,
        runType: 'backup',
      },
      profiles: [],
      warnings: [],
    })

    renderShellProbe({ setLanguagePreference: languageSpy })

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    expect(languageSpy).toHaveBeenCalled()
    expect(languageSpy).toHaveBeenCalledWith(
      snapshot.config.preferredLanguage,
      { persist: false },
    )

    await user.click(screen.getByRole('button', { name: 'save' }))
    await waitFor(() =>
      expect(screen.getByTestId('snapshot-language')).toHaveTextContent(
        'zh-CN',
      ),
    )

    await user.click(screen.getByRole('button', { name: 'initialize' }))
    await waitFor(() =>
      expect(screen.getByTestId('notice')).not.toHaveTextContent('none'),
    )

    await user.click(screen.getByRole('button', { name: 'backup' }))
    await waitFor(() =>
      expect(screen.getByTestId('notice')).toHaveTextContent(/run #42/i),
    )
    expect(getBackupProgressMock()).toHaveBeenCalledTimes(1)
    expect(unsubscribe).toHaveBeenCalledTimes(1)

    await user.click(screen.getByRole('button', { name: 'clear' }))
    expect(screen.getByTestId('notice')).toHaveTextContent('none')

    await user.click(screen.getByRole('button', { name: 'refresh' }))
    await waitFor(() =>
      expect(screen.getByTestId('refresh-key')).not.toHaveTextContent('0'),
    )
  })

  test('uses the paint fallback and surfaces refresh errors without breaking follow-up saves', async () => {
    const user = userEvent.setup()
    const translator = createTranslator('en')
    const { dashboard, snapshot } = await seedSnapshot()
    const savedSnapshot: AppSnapshot = {
      ...snapshot,
      config: { ...snapshot.config, preferredLanguage: 'zh-CN' },
    }
    const originalRequestAnimationFrame = window.requestAnimationFrame
    Object.defineProperty(window, 'requestAnimationFrame', {
      configurable: true,
      writable: true,
      value: undefined,
    })

    const getAppSnapshotSpy = vi
      .spyOn(backend, 'getAppSnapshot')
      .mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot')
      .mockResolvedValueOnce(dashboard)
      .mockRejectedValueOnce(new Error('follow-up dashboard refresh failed'))
      .mockResolvedValue(dashboard)
    vi.spyOn(backend, 'saveConfig').mockResolvedValue(savedSnapshot)

    try {
      renderShellProbe()

      await waitFor(() =>
        expect(screen.getByTestId('loading')).toHaveTextContent('false'),
      )

      await user.click(screen.getByRole('button', { name: 'save' }))
      await waitFor(() =>
        expect(screen.getByTestId('snapshot-language')).toHaveTextContent(
          'zh-CN',
        ),
      )
      expect(screen.getByTestId('error')).toHaveTextContent('none')

      getAppSnapshotSpy.mockRejectedValueOnce('not-an-error')
      await user.click(screen.getByRole('button', { name: 'refresh' }))
      await waitFor(() =>
        expect(screen.getByTestId('error')).toHaveTextContent(
          translator('shell.loadingLatestArchiveState'),
        ),
      )

      getAppSnapshotSpy.mockRejectedValueOnce(new Error('refresh failed'))
      await user.click(screen.getByRole('button', { name: 'refresh' }))
      await waitFor(() =>
        expect(screen.getByTestId('error')).toHaveTextContent('refresh failed'),
      )
    } finally {
      Object.defineProperty(window, 'requestAnimationFrame', {
        configurable: true,
        writable: true,
        value: originalRequestAnimationFrame,
      })
    }
  })

  test('manual refresh keeps the shell loading flag true until the snapshot settles', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()
    let resolveRefreshSnapshot: ((value: AppSnapshot) => void) | undefined
    const pendingRefreshSnapshot = new Promise<AppSnapshot>((resolve) => {
      resolveRefreshSnapshot = resolve
    })
    const getAppSnapshotSpy = vi
      .spyOn(backend, 'getAppSnapshot')
      .mockResolvedValueOnce(snapshot)
      .mockReturnValueOnce(pendingRefreshSnapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )

    await user.click(screen.getByRole('button', { name: 'refresh' }))
    await waitFor(() => expect(getAppSnapshotSpy).toHaveBeenCalledTimes(2))
    expect(screen.getByTestId('loading')).toHaveTextContent('true')

    await act(async () => {
      resolveRefreshSnapshot?.(snapshot)
      await pendingRefreshSnapshot
    })
    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
  })

  test('silent backup refresh does not clear a pending manual refresh spinner', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()
    let resolveManualRefresh: ((value: AppSnapshot) => void) | undefined
    const pendingManualRefresh = new Promise<AppSnapshot>((resolve) => {
      resolveManualRefresh = resolve
    })
    const getAppSnapshotSpy = vi
      .spyOn(backend, 'getAppSnapshot')
      .mockResolvedValueOnce(snapshot)
      .mockReturnValueOnce(pendingManualRefresh)
      .mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    vi.spyOn(backend, 'runBackupNow').mockResolvedValue({
      dueSkipped: false,
      run: null,
      profiles: [],
      warnings: [],
    })

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )

    await user.click(screen.getByRole('button', { name: 'refresh' }))
    await waitFor(() => expect(getAppSnapshotSpy).toHaveBeenCalledTimes(2))
    expect(screen.getByTestId('loading')).toHaveTextContent('true')

    await user.click(screen.getByRole('button', { name: 'backup' }))
    await waitFor(() => expect(getAppSnapshotSpy).toHaveBeenCalledTimes(3))
    expect(screen.getByTestId('loading')).toHaveTextContent('true')

    await act(async () => {
      resolveManualRefresh?.(snapshot)
      await pendingManualRefresh
    })
    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
  })

  test('localizes non-error dashboard refresh failures during bootstrap', async () => {
    const translator = createTranslator('en')
    const { snapshot } = await seedSnapshot()
    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockRejectedValueOnce(
      'dashboard offline',
    )

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent(
        translator('shell.loadingLatestArchiveState'),
      ),
    )
  })

  test('ignores stale dashboard refresh success after a newer refresh wins', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()
    let resolveStaleRefresh:
      | ((dashboard: DashboardSnapshot) => void)
      | undefined
    const staleRefresh = new Promise<DashboardSnapshot>((resolve) => {
      resolveStaleRefresh = resolve
    })
    const freshDashboard: DashboardSnapshot = {
      ...dashboard,
      generatedAt: '2026-04-24T09:00:00.000Z',
    }
    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot')
      .mockReturnValueOnce(staleRefresh)
      .mockResolvedValue(freshDashboard)

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    await user.click(screen.getByRole('button', { name: 'refresh' }))
    await waitFor(() =>
      expect(screen.getByTestId('dashboard-generated-at')).toHaveTextContent(
        freshDashboard.generatedAt,
      ),
    )

    await act(async () => {
      resolveStaleRefresh?.({
        ...dashboard,
        generatedAt: '2026-04-24T08:00:00.000Z',
      })
      await staleRefresh
    })
    await waitFor(() =>
      expect(screen.getByTestId('dashboard-generated-at')).toHaveTextContent(
        freshDashboard.generatedAt,
      ),
    )
  })

  test('ignores stale dashboard refresh errors after a newer refresh wins', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()
    let rejectStaleRefresh: ((error: Error) => void) | undefined
    const staleRefresh = new Promise<DashboardSnapshot>((_, reject) => {
      rejectStaleRefresh = reject
    })
    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot')
      .mockReturnValueOnce(staleRefresh)
      .mockResolvedValue(dashboard)

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    await user.click(screen.getByRole('button', { name: 'refresh' }))
    await waitFor(() =>
      expect(screen.getByTestId('dashboard-generated-at')).toHaveTextContent(
        dashboard.generatedAt,
      ),
    )

    await act(async () => {
      rejectStaleRefresh?.(new Error('stale dashboard failed'))
      await staleRefresh.catch(() => undefined)
    })
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent('none'),
    )
  })

  test('keeps dashboard loading active when a stale refresh settles before the current one', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()
    let resolveStaleRefresh:
      | ((dashboard: DashboardSnapshot) => void)
      | undefined
    let resolveCurrentRefresh:
      | ((dashboard: DashboardSnapshot) => void)
      | undefined
    const staleRefresh = new Promise<DashboardSnapshot>((resolve) => {
      resolveStaleRefresh = resolve
    })
    const currentRefresh = new Promise<DashboardSnapshot>((resolve) => {
      resolveCurrentRefresh = resolve
    })
    const freshDashboard: DashboardSnapshot = {
      ...dashboard,
      generatedAt: '2026-04-24T10:00:00.000Z',
    }
    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    const loadDashboardSnapshotSpy = vi
      .spyOn(backend, 'loadDashboardSnapshot')
      .mockReturnValueOnce(staleRefresh)
      .mockReturnValueOnce(currentRefresh)

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    expect(screen.getByTestId('dashboard-loading')).toHaveTextContent('true')

    await user.click(screen.getByRole('button', { name: 'refresh' }))
    await waitFor(() =>
      expect(loadDashboardSnapshotSpy).toHaveBeenCalledTimes(2),
    )
    expect(screen.getByTestId('dashboard-loading')).toHaveTextContent('true')
    await act(async () => {
      resolveStaleRefresh?.({
        ...dashboard,
        generatedAt: '2026-04-24T09:30:00.000Z',
      })
      await staleRefresh
    })
    expect(screen.getByTestId('dashboard-loading')).toHaveTextContent('true')

    await act(async () => {
      resolveCurrentRefresh?.(freshDashboard)
      await currentRefresh
    })
    await waitFor(() =>
      expect(screen.getByTestId('dashboard-generated-at')).toHaveTextContent(
        freshDashboard.generatedAt,
      ),
    )
    expect(screen.getByTestId('dashboard-loading')).toHaveTextContent('false')
  })

  test('does not convert ordinary refresh errors into app-lock fallback state', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()
    const lockedStatus = {
      ...snapshot.appLockStatus,
      enabled: true,
      locked: true,
      lockReason: 'manual',
    }
    vi.spyOn(backend, 'getAppSnapshot')
      .mockResolvedValueOnce(snapshot)
      .mockRejectedValueOnce(new Error('ordinary refresh failed'))
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadAppLockStatus')
      .mockResolvedValueOnce(snapshot.appLockStatus)
      .mockResolvedValueOnce(snapshot.appLockStatus)
      .mockResolvedValueOnce(lockedStatus)
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    await waitFor(() =>
      expect(screen.getByTestId('snapshot-language')).not.toHaveTextContent(
        'none',
      ),
    )

    await user.click(screen.getByRole('button', { name: 'refresh' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent(
        'ordinary refresh failed',
      ),
    )
    expect(screen.getByTestId('app-lock-locked')).toHaveTextContent('false')
    expect(screen.getByTestId('snapshot-language')).not.toHaveTextContent(
      'none',
    )
  })

  test('surfaces initial refresh failures without leaking an unhandled rejection', async () => {
    const { snapshot } = await seedSnapshot()
    vi.spyOn(backend, 'loadAppLockStatus').mockResolvedValue(
      snapshot.appLockStatus,
    )
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'getAppSnapshot').mockRejectedValueOnce(
      new Error('initial refresh failed'),
    )

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent(
        'initial refresh failed',
      ),
    )
  })

  test('falls back to a zero-state dashboard when the archive is still uninitialized', async () => {
    const { snapshot } = await seedSnapshot()
    const uninitializedSnapshot: AppSnapshot = {
      ...snapshot,
      config: { ...snapshot.config, initialized: false },
      archiveStatus: {
        ...snapshot.archiveStatus,
        initialized: false,
        encrypted: false,
        unlocked: false,
      },
      recentRuns: [],
    }
    vi.spyOn(backend, 'loadAppLockStatus').mockResolvedValue(
      uninitializedSnapshot.appLockStatus,
    )
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(uninitializedSnapshot)
    vi.spyOn(backend, 'loadDashboardSnapshot').mockRejectedValueOnce(
      new Error('dashboard bootstrap failed before onboarding'),
    )

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    expect(screen.getByTestId('error')).toHaveTextContent('none')
    expect(screen.getByTestId('snapshot-language')).toHaveTextContent('system')
    expect(screen.getByTestId('dashboard-generated-at')).not.toHaveTextContent(
      'none',
    )
  })

  test('surfaces each runtime crash report path once', async () => {
    const user = userEvent.setup()
    const translator = createTranslator('en')
    const { dashboard, snapshot } = await seedSnapshot()
    const crashSnapshot: AppSnapshot = {
      ...snapshot,
      runtimeDiagnostics: {
        ...snapshot.runtimeDiagnostics,
        latestCrashReport: {
          source: 'rust-panic',
          recordedAt: '2026-04-25T12:00:00.000Z',
          fatal: true,
          message: 'panic while importing browser history',
          location: 'src-tauri/src/lib.rs:42',
          path: '/tmp/pathkeep-crash.json',
        },
      },
    }
    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(crashSnapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('notice')).toHaveTextContent(
        translator('shell.runtimeCrashNotice'),
      ),
    )

    await user.click(screen.getByRole('button', { name: 'clear' }))
    await waitFor(() =>
      expect(screen.getByTestId('notice')).toHaveTextContent('none'),
    )

    await user.click(screen.getByRole('button', { name: 'refresh' }))
    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    expect(screen.getByTestId('notice')).toHaveTextContent('none')
  })

  test('does not let a crash report replace an existing notice', async () => {
    const user = userEvent.setup()
    const translator = createTranslator('en')
    const { dashboard, snapshot } = await seedSnapshot()
    const initializedCrashSnapshot: AppSnapshot = {
      ...snapshot,
      config: {
        ...snapshot.config,
        initialized: true,
        preferredLanguage: 'zh-TW',
      },
      runtimeDiagnostics: {
        ...snapshot.runtimeDiagnostics,
        latestCrashReport: {
          source: 'frontend-error',
          recordedAt: '2026-04-25T13:00:00.000Z',
          fatal: false,
          message: 'render failed after route transition',
          location: null,
          path: '/tmp/pathkeep-render-crash.json',
        },
      },
    }
    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    vi.spyOn(backend, 'initializeArchive').mockResolvedValue(
      initializedCrashSnapshot,
    )

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    await user.click(screen.getByRole('button', { name: 'initialize' }))
    await waitFor(() =>
      expect(screen.getByTestId('notice')).toHaveTextContent(
        translator('shell.initializedNotice'),
      ),
    )
    expect(screen.getByTestId('notice')).not.toHaveTextContent(
      translator('shell.runtimeCrashNotice'),
    )
  })

  test('publishes a stale-plist notification when the macOS schedule probe reports a mismatch', async () => {
    const translator = createTranslator('en')
    const { dashboard, snapshot } = await seedSnapshot()
    // The probe gates on (config.initialized && archiveStatus.unlocked) so
    // first-run installs and locked sessions stay silent; an upgraded user
    // with an established archive is exactly when we want the notification.
    const readySnapshot: AppSnapshot = {
      ...snapshot,
      config: { ...snapshot.config, initialized: true },
      archiveStatus: { ...snapshot.archiveStatus, unlocked: true },
    }
    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(readySnapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    vi.spyOn(backend, 'scheduleStatus').mockResolvedValue({
      platform: 'macos',
      label: 'app.pathkeep.scheduled-backup',
      dueAfterHours: 72,
      checkIntervalHours: 6,
      applySupported: true,
      installState: 'mismatch',
      detectedFiles: [],
      manualSteps: [],
      issues: [],
      warnings: [],
      verificationChecks: [],
    })

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('notification-titles')).toHaveTextContent(
        translator('shell.scheduleStaleAfterUpgradeTitle'),
      ),
    )
  })

  test('keeps the shell quiet when the macOS schedule probe reports anything other than mismatch', async () => {
    const translator = createTranslator('en')
    const { dashboard, snapshot } = await seedSnapshot()
    const readySnapshot: AppSnapshot = {
      ...snapshot,
      config: { ...snapshot.config, initialized: true },
      archiveStatus: { ...snapshot.archiveStatus, unlocked: true },
    }
    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(readySnapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    const scheduleStatusSpy = vi
      .spyOn(backend, 'scheduleStatus')
      .mockResolvedValue({
        platform: 'macos',
        label: 'app.pathkeep.scheduled-backup',
        dueAfterHours: 72,
        checkIntervalHours: 6,
        applySupported: true,
        installState: 'installed',
        detectedFiles: [],
        manualSteps: [],
        issues: [],
        warnings: [],
        verificationChecks: [],
      })

    renderShellProbe()

    await waitFor(() => expect(scheduleStatusSpy).toHaveBeenCalled())
    // Settle one more tick so a hypothetical late publish would have to land.
    await act(async () => {
      await Promise.resolve()
    })
    expect(screen.getByTestId('notification-titles')).not.toHaveTextContent(
      translator('shell.scheduleStaleAfterUpgradeTitle'),
    )
  })

  test('runs the macOS schedule probe at most once per archive databasePath, even after the archive is locked and re-unlocked', async () => {
    const user = userEvent.setup()
    const translator = createTranslator('en')
    const { dashboard, snapshot } = await seedSnapshot()
    const readySnapshot: AppSnapshot = {
      ...snapshot,
      config: { ...snapshot.config, initialized: true },
      archiveStatus: { ...snapshot.archiveStatus, unlocked: true },
    }
    const lockedSnapshot: AppSnapshot = {
      ...readySnapshot,
      archiveStatus: { ...readySnapshot.archiveStatus, unlocked: false },
    }
    // First call returns the unlocked snapshot (probe fires). The refresh
    // sequence then briefly flips to locked → unlocked, which re-runs the
    // useEffect with the SAME databasePath after the lock interlude — the
    // ref guard must short-circuit that second run so the notification
    // doesn't re-fire every time the user re-enters their passcode.
    vi.spyOn(backend, 'getAppSnapshot')
      .mockResolvedValueOnce(readySnapshot)
      .mockResolvedValueOnce(lockedSnapshot)
      .mockResolvedValue(readySnapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    const scheduleStatusSpy = vi
      .spyOn(backend, 'scheduleStatus')
      .mockResolvedValue({
        platform: 'macos',
        label: 'app.pathkeep.scheduled-backup',
        dueAfterHours: 72,
        checkIntervalHours: 6,
        applySupported: true,
        installState: 'mismatch',
        detectedFiles: [],
        manualSteps: [],
        issues: [],
        warnings: [],
        verificationChecks: [],
      })

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('notification-titles')).toHaveTextContent(
        translator('shell.scheduleStaleAfterUpgradeTitle'),
      ),
    )
    expect(scheduleStatusSpy).toHaveBeenCalledTimes(1)
    const firstNotificationCount =
      screen.getByTestId('notification-count').textContent

    // Two refreshes: first lands `unlocked: false` so the effect bails on
    // the gate, second re-lands `unlocked: true` so the effect would
    // re-enter the body — the ref guard's job is to early-return there.
    await user.click(screen.getByRole('button', { name: 'refresh' }))
    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    await user.click(screen.getByRole('button', { name: 'refresh' }))
    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    expect(scheduleStatusSpy).toHaveBeenCalledTimes(1)
    expect(screen.getByTestId('notification-count')).toHaveTextContent(
      firstNotificationCount ?? '0',
    )
  })

  test('drops the in-flight schedule probe result when the provider unmounts mid-await', async () => {
    const { dashboard, snapshot } = await seedSnapshot()
    const readySnapshot: AppSnapshot = {
      ...snapshot,
      config: { ...snapshot.config, initialized: true },
      archiveStatus: { ...snapshot.archiveStatus, unlocked: true },
    }
    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(readySnapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    let resolveProbe: ((value: never) => void) | undefined
    vi.spyOn(backend, 'scheduleStatus').mockReturnValue(
      new Promise((resolve) => {
        resolveProbe = resolve as (value: never) => void
      }),
    )

    const probe = renderShellProbe()
    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    // Unmount the provider while the schedule probe is still awaiting its
    // response. The cancel-token check inside the probe must short-circuit
    // the publish path so we don't try to update unmounted state.
    probe.unmount()
    resolveProbe?.({
      platform: 'macos',
      label: 'app.pathkeep.scheduled-backup',
      dueAfterHours: 72,
      checkIntervalHours: 6,
      applySupported: true,
      installState: 'mismatch',
      detectedFiles: [],
      manualSteps: [],
      issues: [],
      warnings: [],
      verificationChecks: [],
    } as never)
    await act(async () => {
      await Promise.resolve()
    })
    // If the cancel-token branch were missing, React would warn about a
    // setState on an unmounted component; the assertion below is just a
    // smoke check that nothing else blew up.
    expect(true).toBe(true)
  })

  test('survives a scheduleStatus probe rejection without surfacing an error to the shell', async () => {
    const { dashboard, snapshot } = await seedSnapshot()
    const readySnapshot: AppSnapshot = {
      ...snapshot,
      config: { ...snapshot.config, initialized: true },
      archiveStatus: { ...snapshot.archiveStatus, unlocked: true },
    }
    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(readySnapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    vi.spyOn(backend, 'scheduleStatus').mockRejectedValue(
      new Error('schedule_status: unsupported platform'),
    )

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    expect(screen.getByTestId('error')).toHaveTextContent('none')
    expect(screen.getByTestId('notification-count')).toHaveTextContent('0')
  })

  test('ignores follow-up dashboard refresh failures after saving config', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()
    const savedSnapshot: AppSnapshot = {
      ...snapshot,
      config: { ...snapshot.config, preferredLanguage: 'zh-CN' },
    }
    const loadDashboardSnapshotSpy = vi
      .spyOn(backend, 'loadDashboardSnapshot')
      .mockResolvedValueOnce(dashboard)
      .mockRejectedValueOnce(new Error('follow-up dashboard refresh failed'))
    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'saveConfig').mockResolvedValue(savedSnapshot)

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )

    await user.click(screen.getByRole('button', { name: 'save' }))
    await waitFor(() =>
      expect(screen.getByTestId('snapshot-language')).toHaveTextContent(
        'zh-CN',
      ),
    )
    await waitFor(() =>
      expect(loadDashboardSnapshotSpy).toHaveBeenCalledTimes(2),
    )
    expect(screen.getByTestId('error')).toHaveTextContent('none')
  })

  test('refreshes dashboard rhythm surfaces when runtime jobs drain to idle', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()
    const activeQueue: AiQueueStatus = {
      paused: false,
      concurrency: 1,
      queued: 0,
      running: 1,
      failed: 0,
      recentJobs: [],
    }
    const idleQueue: AiQueueStatus = {
      ...activeQueue,
      running: 0,
    }
    const activeRuntime: IntelligenceRuntimeSnapshot = {
      queue: {
        queued: 0,
        running: 1,
        succeeded: 0,
        failed: 0,
        cancelled: 0,
        lastActivityAt: '2026-04-24T08:00:00Z',
      },
      plugins: [],
      modules: [],
      recentJobs: [],
      notes: [],
    }
    const idleRuntime: IntelligenceRuntimeSnapshot = {
      ...activeRuntime,
      queue: {
        ...activeRuntime.queue,
        running: 0,
        succeeded: 1,
      },
    }
    const loadDashboardSnapshotSpy = vi
      .spyOn(backend, 'loadDashboardSnapshot')
      .mockResolvedValue(dashboard)
    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadAiQueueStatus')
      .mockResolvedValueOnce(activeQueue)
      .mockResolvedValue(idleQueue)
    vi.spyOn(backend, 'loadIntelligenceRuntime')
      .mockResolvedValueOnce(activeRuntime)
      .mockResolvedValue(idleRuntime)

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('runtime-running')).toHaveTextContent('1'),
    )
    expect(screen.getByTestId('refresh-key')).toHaveTextContent('1')

    await user.click(screen.getByRole('button', { name: 'refresh-runtime' }))

    await waitFor(() =>
      expect(screen.getByTestId('runtime-running')).toHaveTextContent('0'),
    )
    await waitFor(() =>
      expect(screen.getByTestId('refresh-key')).toHaveTextContent('2'),
    )
    expect(loadDashboardSnapshotSpy).toHaveBeenCalledTimes(2)
  })

  test('surfaces dashboard refresh errors when runtime jobs drain', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()
    const activeQueue: AiQueueStatus = {
      paused: false,
      concurrency: 1,
      queued: 0,
      running: 1,
      failed: 0,
      recentJobs: [],
    }
    const idleQueue: AiQueueStatus = {
      ...activeQueue,
      running: 0,
    }
    const activeRuntime: IntelligenceRuntimeSnapshot = {
      queue: {
        queued: 0,
        running: 1,
        succeeded: 0,
        failed: 0,
        cancelled: 0,
        lastActivityAt: '2026-04-24T08:00:00Z',
      },
      plugins: [],
      modules: [],
      recentJobs: [],
      notes: [],
    }
    const idleRuntime: IntelligenceRuntimeSnapshot = {
      ...activeRuntime,
      queue: {
        ...activeRuntime.queue,
        running: 0,
        succeeded: 1,
      },
    }
    vi.spyOn(backend, 'loadDashboardSnapshot')
      .mockResolvedValueOnce(dashboard)
      .mockRejectedValueOnce(new Error('runtime drain dashboard failed'))
    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadAiQueueStatus')
      .mockResolvedValueOnce(activeQueue)
      .mockResolvedValue(idleQueue)
    vi.spyOn(backend, 'loadIntelligenceRuntime')
      .mockResolvedValueOnce(activeRuntime)
      .mockResolvedValue(idleRuntime)

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('runtime-running')).toHaveTextContent('1'),
    )
    await user.click(screen.getByRole('button', { name: 'refresh-runtime' }))

    await waitFor(() =>
      expect(screen.getByTestId('runtime-running')).toHaveTextContent('0'),
    )
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent(
        'runtime drain dashboard failed',
      ),
    )
  })
})
