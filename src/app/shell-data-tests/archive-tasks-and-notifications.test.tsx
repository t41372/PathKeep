/**
 * @file archive-tasks-and-notifications.test.tsx
 * @description Shell-data coverage for global archive tasks and notification persistence.
 * @module app/shell-data-tests
 */

import { act, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import type { AppSnapshot, TakeoutInspection } from '../../lib/types'
import {
  getDefaultBuildInfo,
  getImportProgressMock,
  renderShellProbe,
  resetShellDataHarness,
  seedSnapshot,
} from './test-helpers'

const notificationStorageKey = 'pathkeep.shellNotifications.v1'

describe('ShellDataProvider archive tasks and notifications', () => {
  beforeEach(() => {
    resetShellDataHarness()
    window.localStorage.removeItem(notificationStorageKey)
  })

  test('starts imports as shell-owned tasks and completes them with notifications', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()
    const importResult = takeoutInspection()
    const unsubscribe = vi.fn()

    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    vi.spyOn(backend, 'importTakeout').mockResolvedValue(importResult)
    getImportProgressMock().mockImplementationOnce((listener) => {
      listener({
        phase: 'import-file',
        label: 'Importing browser history',
        detail: 'Writing BrowserHistory.json',
        current: 1,
        total: 1,
        progressPercent: null,
        logLines: ['raw fallback'],
        sourcePath: '/tmp/Takeout/BrowserHistory.json',
        sourceLabel: 'Chrome Default',
        processedRecords: 3,
        totalRecords: 6,
        importedRecords: 2,
        duplicateRecords: 1,
        skippedRecords: 0,
        logEvents: [
          {
            level: 'success',
            code: 'import.records',
            message: '3 records processed.',
            sourceLabel: 'Chrome Default',
            processedRecords: 3,
            totalRecords: 6,
          },
        ],
      })
      return Promise.resolve(unsubscribe)
    })

    renderShellProbe()
    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )

    await user.click(screen.getByRole('button', { name: 'import-takeout' }))

    await waitFor(() => expect(backend.importTakeout).toHaveBeenCalled())
    await waitFor(() =>
      expect(screen.getByTestId('latest-archive-task')).toHaveTextContent(
        'Import Google Takeout',
      ),
    )
    await waitFor(() =>
      expect(screen.getByTestId('archive-task-count')).toHaveTextContent('1'),
    )
    await waitFor(() =>
      expect(screen.getByTestId('active-archive-task')).toHaveTextContent(
        'none',
      ),
    )
    await waitFor(() =>
      expect(screen.getByTestId('unread-notifications')).toHaveTextContent('2'),
    )
    expect(unsubscribe).toHaveBeenCalledTimes(1)
    expect(
      JSON.parse(window.localStorage.getItem(notificationStorageKey) ?? '[]'),
    ).toHaveLength(2)
  })

  test('starts browser imports with profile labels in the shell task', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()
    const importResult = takeoutInspection()
    let resolveImport: ((value: TakeoutInspection) => void) | null = null

    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    vi.spyOn(backend, 'importBrowserHistory').mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveImport = resolve
        }),
    )

    renderShellProbe()
    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )

    await user.click(screen.getByRole('button', { name: 'import-browser' }))

    await waitFor(() =>
      expect(screen.getByTestId('active-archive-task')).toHaveTextContent(
        'Import browser history',
      ),
    )
    await waitFor(() =>
      expect(backend.importBrowserHistory).toHaveBeenCalledWith(
        expect.objectContaining({
          browserName: 'Chrome',
          profileName: 'Default',
        }),
      ),
    )

    await act(async () => {
      resolveImport?.(importResult)
      await Promise.resolve()
    })

    await waitFor(() =>
      expect(screen.getByTestId('active-archive-task')).toHaveTextContent(
        'none',
      ),
    )
  })

  test('starts browser imports with profile-id fallback metadata', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()

    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    vi.spyOn(backend, 'importBrowserHistory').mockResolvedValue(
      takeoutInspection(),
    )

    renderShellProbe()
    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )

    await user.click(
      screen.getByRole('button', { name: 'import-browser-profile-id' }),
    )

    await waitFor(() =>
      expect(backend.importBrowserHistory).toHaveBeenCalledWith(
        expect.objectContaining({
          profileId: 'chrome:Profile 1',
          sourcePath: '/profiles/Profile 1/History',
        }),
      ),
    )
  })

  test('starts browser imports with no discovered profile metadata', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()

    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    vi.spyOn(backend, 'importBrowserHistory').mockResolvedValue(
      takeoutInspection(),
    )

    renderShellProbe()
    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )

    await user.click(
      screen.getByRole('button', { name: 'import-browser-no-profile' }),
    )

    await waitFor(() =>
      expect(backend.importBrowserHistory).toHaveBeenCalledWith(
        expect.objectContaining({
          profileId: null,
          profileName: null,
          sourcePath: '/manual/History',
        }),
      ),
    )
  })

  test('links second archive-write actions to the already running task', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()
    let resolveImport: ((value: TakeoutInspection) => void) | null = null

    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    vi.spyOn(backend, 'importTakeout').mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveImport = resolve
        }),
    )
    vi.spyOn(backend, 'importBrowserHistory').mockResolvedValue(
      takeoutInspection(),
    )
    vi.spyOn(backend, 'runBackupNow').mockResolvedValue({
      dueSkipped: false,
      reason: null,
      run: null,
      profiles: [],
      manifestPath: null,
      gitCommit: null,
      warnings: [],
      remoteBackup: null,
    })

    renderShellProbe()
    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )

    await user.click(screen.getByRole('button', { name: 'import-takeout' }))
    await waitFor(() =>
      expect(screen.getByTestId('active-archive-task')).toHaveTextContent(
        'Import Google Takeout',
      ),
    )
    await user.click(screen.getByRole('button', { name: 'backup' }))

    expect(backend.runBackupNow).not.toHaveBeenCalled()
    await user.click(screen.getByRole('button', { name: 'import-browser' }))

    expect(backend.importBrowserHistory).not.toHaveBeenCalled()
    await waitFor(() =>
      expect(screen.getByTestId('unread-notifications')).toHaveTextContent('3'),
    )

    await act(async () => {
      resolveImport?.(takeoutInspection())
      await Promise.resolve()
    })
    await waitFor(() =>
      expect(screen.getByTestId('active-archive-task')).toHaveTextContent(
        'none',
      ),
    )
  })

  test('fails shell-owned imports with a task notification when the backend rejects', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()

    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    vi.spyOn(backend, 'importTakeout').mockRejectedValue('string failure')

    renderShellProbe()
    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )

    await user.click(screen.getByRole('button', { name: 'import-takeout' }))

    await waitFor(() => expect(backend.importTakeout).toHaveBeenCalled())
    await waitFor(() =>
      expect(screen.getByTestId('active-archive-task')).toHaveTextContent(
        'none',
      ),
    )
    await waitFor(() =>
      expect(screen.getByTestId('unread-notifications')).toHaveTextContent('2'),
    )
  })

  test('uses backend error messages when shell-owned imports fail with Error objects', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()

    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    vi.spyOn(backend, 'importTakeout').mockRejectedValue(
      new Error('import failed'),
    )

    renderShellProbe()
    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )

    await user.click(screen.getByRole('button', { name: 'import-takeout' }))

    await waitFor(() => expect(backend.importTakeout).toHaveBeenCalled())
    await waitFor(() =>
      expect(screen.getByTestId('unread-notifications')).toHaveTextContent('2'),
    )
  })

  test('loads only valid stored notifications and tolerates broken localStorage payloads', async () => {
    const user = userEvent.setup()
    const bootstrap = await seedSnapshot()
    mockBootstrap(bootstrap)
    window.localStorage.setItem(notificationStorageKey, '{')
    const invalidJson = renderShellProbe()
    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    expect(screen.getByTestId('unread-notifications')).toHaveTextContent('0')
    invalidJson.unmount()

    mockBootstrap(bootstrap)
    window.localStorage.setItem(notificationStorageKey, '{}')
    const nonArray = renderShellProbe()
    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    expect(screen.getByTestId('unread-notifications')).toHaveTextContent('0')
    nonArray.unmount()

    mockBootstrap(bootstrap)
    window.localStorage.setItem(
      notificationStorageKey,
      JSON.stringify([
        {
          id: 'valid',
          timestamp: '2026-04-27T10:00:00.000Z',
          title: 'Stored notification',
          body: 'Still visible',
          tone: 'info',
          read: false,
        },
        null,
        'invalid primitive',
        { id: 'invalid' },
      ]),
    )
    renderShellProbe()
    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    expect(screen.getByTestId('unread-notifications')).toHaveTextContent('1')
    expect(screen.getByTestId('notification-count')).toHaveTextContent('1')

    await user.click(screen.getByRole('button', { name: 'mark-notifications' }))
    expect(screen.getByTestId('unread-notifications')).toHaveTextContent('0')

    await user.click(
      screen.getByRole('button', { name: 'dismiss-notification' }),
    )
    expect(screen.getByTestId('notification-count')).toHaveTextContent('0')
  })
})

function mockBootstrap({
  dashboard,
  snapshot,
}: {
  dashboard: Awaited<ReturnType<typeof seedSnapshot>>['dashboard']
  snapshot: AppSnapshot
}) {
  vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
  vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(getDefaultBuildInfo())
  vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
}

function takeoutInspection(): TakeoutInspection {
  return {
    sourcePath: '/tmp/Takeout',
    dryRun: false,
    recognizedFiles: [],
    quarantinedFiles: [],
    candidateItems: 6,
    importedItems: 4,
    duplicateItems: 2,
    previewEntries: [],
    importBatch: {
      id: 9,
      sourceKind: 'google-takeout',
      sourcePath: '/tmp/Takeout',
      profileId: 'takeout::browser-history',
      createdAt: '2026-04-27T10:00:00.000Z',
      importedAt: '2026-04-27T10:02:00.000Z',
      revertedAt: null,
      status: 'imported',
      candidateItems: 6,
      importedItems: 4,
      duplicateItems: 2,
      visibleItems: 4,
      auditPath: null,
      gitCommit: null,
    },
    notes: [],
    detectedLocale: null,
    previewRangeStart: null,
    previewRangeEnd: null,
  }
}
