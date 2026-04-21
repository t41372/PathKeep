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

import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import { createTranslator } from '../../lib/i18n'
import type { AppSnapshot } from '../../lib/types'
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
      remoteBackup: null,
    })

    renderShellProbe({ setLanguagePreference: languageSpy })

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    expect(languageSpy).toHaveBeenCalled()

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
    expect(screen.getByTestId('error')).toHaveTextContent(
      'initial refresh failed',
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
})
