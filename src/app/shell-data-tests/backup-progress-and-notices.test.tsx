/**
 * @file backup-progress-and-notices.test.tsx
 * @description Split shell-data suite for backup busy-overlay progress and manual-backup notice contracts.
 * @module app/shell-data-tests
 *
 * ## Responsibilities
 * - Preserve the original shell-data tests covering initialize/backup busy states before long-running work settles.
 * - Preserve the original manual-backup notice coverage for due-window and generic completion outcomes.
 * - Preserve the shared busy-overlay progress assertions for streamed backup phases.
 *
 * ## Not responsible for
 * - Rewriting `ShellDataProvider` test helpers or changing the shared provider contract.
 * - Expanding coverage beyond the three backup-progress and notice cases owned by this split.
 * - Mutating neighboring shell-data suites while other workers are splitting adjacent slices.
 *
 * ## Dependencies
 * - Depends on `src/app/shell-data-tests/test-helpers.tsx` for the canonical shell-data harness.
 * - Uses the real backend client mocks plus the shared backup-progress subscription mock surface.
 *
 * ## Performance notes
 * - Reuses the centralized shell-data harness so this split does not duplicate archive bootstrap work.
 */

import { act, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import { createTranslator } from '../../lib/i18n'
import type {
  AppSnapshot,
  BackupProgressEvent,
  BackupReport,
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

  test('surfaces initialize and backup busy states before long-running work settles', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()
    const initializedSnapshot: AppSnapshot = {
      ...snapshot,
      config: {
        ...snapshot.config,
        initialized: true,
      },
    }
    const backupReport: BackupReport = {
      dueSkipped: false,
      run: {
        id: 77,
        startedAt: '2026-04-20T08:00:00Z',
        finishedAt: '2026-04-20T08:05:00Z',
        status: 'success',
        manifestHash: 'manifest-77',
        profileScope: ['chrome:Default'],
        profilesProcessed: 1,
        newVisits: 3,
        newUrls: 1,
        newDownloads: 0,
        runType: 'backup',
      },
      profiles: [],
      warnings: [],
      remoteBackup: null,
    }
    const unsubscribe = vi.fn()
    let resolveInitialize: ((value: AppSnapshot) => void) | null = null
    let resolveBackup: ((value: BackupReport) => void) | null = null

    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    vi.spyOn(backend, 'initializeArchive').mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveInitialize = resolve
        }),
    )
    getBackupProgressMock().mockResolvedValue(unsubscribe)
    vi.spyOn(backend, 'runBackupNow').mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveBackup = resolve
        }),
    )

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )

    await user.click(screen.getByRole('button', { name: 'initialize' }))
    await waitFor(() =>
      expect(screen.getByTestId('busy-label')).toHaveTextContent(
        createTranslator('en')('shell.preparingArchive'),
      ),
    )
    await waitFor(() => expect(backend.initializeArchive).toHaveBeenCalled())
    expect(screen.getByTestId('notice')).toHaveTextContent('none')

    await act(async () => {
      resolveInitialize?.(initializedSnapshot)
      await Promise.resolve()
    })

    await waitFor(() =>
      expect(screen.getByTestId('notice')).not.toHaveTextContent('none'),
    )

    await user.click(screen.getByRole('button', { name: 'backup' }))
    await waitFor(() =>
      expect(screen.getByTestId('busy-label')).toHaveTextContent(
        createTranslator('en')('shell.backupWritingArchive'),
      ),
    )
    await waitFor(() => expect(backend.runBackupNow).toHaveBeenCalled())

    await act(async () => {
      resolveBackup?.(backupReport)
      await Promise.resolve()
    })

    await waitFor(() =>
      expect(screen.getByTestId('notice')).toHaveTextContent(/run #77/i),
    )
    expect(unsubscribe).toHaveBeenCalledTimes(1)
  })

  test('surfaces due-window and generic completion notices for manual backups', async () => {
    const user = userEvent.setup()
    const translator = createTranslator('en')
    const { dashboard, snapshot } = await seedSnapshot()

    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    vi.spyOn(backend, 'runBackupNow')
      .mockResolvedValueOnce({
        dueSkipped: true,
        reason: 'Backup is still within the due window.',
        run: null,
        profiles: [],
        warnings: [],
        remoteBackup: null,
      })
      .mockResolvedValueOnce({
        dueSkipped: true,
        reason: null,
        run: null,
        profiles: [],
        warnings: [],
        remoteBackup: null,
      })
      .mockResolvedValueOnce({
        dueSkipped: false,
        run: null,
        profiles: [],
        warnings: [],
        remoteBackup: null,
      })

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )

    await user.click(screen.getByRole('button', { name: 'backup' }))
    await waitFor(() =>
      expect(screen.getByTestId('notice')).toHaveTextContent(
        'Backup is still within the due window.',
      ),
    )

    await user.click(screen.getByRole('button', { name: 'backup' }))
    await waitFor(() =>
      expect(screen.getByTestId('notice')).toHaveTextContent(
        translator('shell.manualBackupDueWindow'),
      ),
    )

    await user.click(screen.getByRole('button', { name: 'backup' }))
    await waitFor(() =>
      expect(screen.getByTestId('notice')).toHaveTextContent(
        translator('common.complete'),
      ),
    )
  })

  test('tracks backup progress phases through the shared busy overlay state', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()
    const translator = createTranslator('en')
    const unsubscribe = vi.fn()
    let listener: ((event: BackupProgressEvent) => void) | null = null
    let resolveBackup: ((value: BackupReport) => void) | null = null

    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    getBackupProgressMock().mockImplementation((nextListener) => {
      listener = nextListener
      return Promise.resolve(unsubscribe)
    })
    vi.spyOn(backend, 'runBackupNow').mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveBackup = resolve
        }),
    )

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )

    await user.click(screen.getByRole('button', { name: 'backup' }))
    await waitFor(() =>
      expect(screen.getByTestId('busy-label')).toHaveTextContent(
        translator('shell.backupWritingArchive'),
      ),
    )
    expect(screen.getByTestId('busy-progress-label')).toHaveTextContent('2 / 3')
    expect(
      Number(screen.getByTestId('busy-progress-value').textContent),
    ).toBeCloseTo(67, 0)

    act(() => {
      listener?.({
        phase: 'prepare',
        label: 'Inspect selected browser profiles',
        detail: 'Queued 3 readable profile(s) for the canonical backup run.',
        step: 0,
        totalSteps: 3,
        completedProfiles: 0,
        totalProfiles: 3,
        profileId: null,
      })
    })
    await waitFor(() =>
      expect(screen.getByTestId('busy-progress-label')).toHaveTextContent(
        '0 / 3',
      ),
    )

    act(() => {
      listener?.({
        phase: 'stage-profile',
        label: 'Stage source profile',
        detail: 'Copying chrome:Default into the staging area (1/3).',
        step: 1,
        totalSteps: 3,
        completedProfiles: 0,
        totalProfiles: 3,
        profileId: 'chrome:Default',
      })
    })
    await waitFor(() =>
      expect(screen.getByTestId('busy-detail')).toHaveTextContent(
        'chrome:Default (1/3)',
      ),
    )
    expect(
      Number(screen.getByTestId('busy-progress-value').textContent),
    ).toBeCloseTo(33, 0)

    act(() => {
      listener?.({
        phase: 'ingest-profile',
        label: 'Write canonical archive facts',
        detail: 'Processing chrome:Default and writing archive rows (2/3).',
        step: 1,
        totalSteps: 3,
        completedProfiles: 1,
        totalProfiles: 3,
        profileId: null,
      })
    })
    await waitFor(() =>
      expect(screen.getByTestId('busy-label')).toHaveTextContent(
        translator('shell.backupWritingArchive'),
      ),
    )

    act(() => {
      listener?.({
        phase: 'finalize',
        label: 'Finalize manifest and cached totals',
        detail: 'Committing run artifacts after 3 processed profile(s).',
        step: 2,
        totalSteps: 3,
        completedProfiles: 3,
        totalProfiles: 3,
        profileId: null,
      })
    })
    await waitFor(() =>
      expect(screen.getByTestId('busy-progress-label')).toHaveTextContent(
        '3 / 3',
      ),
    )
    expect(screen.getByTestId('busy-progress-value')).toHaveTextContent('100')

    act(() => {
      listener?.({
        phase: 'mystery',
        label: 'Unexpected phase',
        detail: 'Fallback branch should still stay honest.',
        step: 0,
        totalSteps: 3,
        completedProfiles: 0,
        totalProfiles: 0,
        profileId: null,
      })
    })
    await waitFor(() =>
      expect(screen.getByTestId('busy-label')).toHaveTextContent(
        translator('shell.runningManualBackup'),
      ),
    )
    expect(screen.getByTestId('busy-progress-label')).toHaveTextContent('1 / 3')
    expect(
      Number(screen.getByTestId('busy-progress-value').textContent),
    ).toBeCloseTo(33, 0)

    act(() => {
      listener?.({
        phase: 'stage-profile',
        label: 'Stage source profile',
        detail: 'Fallback branch without profile scope.',
        step: 0,
        totalSteps: 0,
        completedProfiles: 0,
        totalProfiles: 0,
        profileId: null,
      })
    })
    await waitFor(() =>
      expect(screen.getByTestId('busy-label')).toHaveTextContent(
        translator('shell.backupWritingArchive'),
      ),
    )
    expect(screen.getByTestId('busy-progress-label')).toHaveTextContent('0 / 0')
    expect(screen.getByTestId('busy-progress-value')).toHaveTextContent('none')

    act(() => {
      listener?.({
        phase: 'finalize',
        label: 'Finalize without profile counts',
        detail: 'Fallback branch without totals.',
        step: 0,
        totalSteps: 0,
        completedProfiles: 0,
        totalProfiles: 0,
        profileId: null,
      })
    })
    await waitFor(() =>
      expect(screen.getByTestId('busy-label')).toHaveTextContent(
        translator('shell.refreshingArchiveViews'),
      ),
    )
    expect(screen.getByTestId('busy-progress-label')).toHaveTextContent('0 / 0')
    expect(screen.getByTestId('busy-progress-value')).toHaveTextContent('none')

    act(() => {
      resolveBackup?.({
        dueSkipped: false,
        run: {
          id: 73,
          startedAt: '2026-04-08T00:00:00Z',
          finishedAt: '2026-04-08T00:05:00Z',
          status: 'success',
          manifestHash: 'manifest-73',
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
    })

    await waitFor(() =>
      expect(screen.getByTestId('notice')).toHaveTextContent(/run #73/i),
    )
    expect(unsubscribe).toHaveBeenCalledTimes(1)
    expect(screen.getByTestId('busy-label')).toHaveTextContent('none')
  })
})
