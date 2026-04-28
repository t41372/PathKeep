/**
 * @file use-audit-data.test.tsx
 * @description Hook-level coverage for Audit run detail, related import batch, and restore workflows.
 * @module pages/audit/hooks
 *
 * ## Responsibilities
 * - Verify audit detail loading, detail-cache refresh, and related import batch lookup.
 * - Protect revert/restore confirmation behavior and snapshot-restore follow-through.
 * - Keep audit workflow failures visible through route-owned state.
 *
 * ## Not responsible for
 * - Re-testing Audit panel rendering or shared review primitives.
 * - Re-testing backend preview command implementations.
 *
 * ## Dependencies
 * - Mocks backend-client methods directly and uses typed audit/import fixtures.
 *
 * ## Performance notes
 * - Hook-level coverage avoids mounting the full Audit route while exercising the state owner.
 */

import { act, renderHook, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../../lib/backend-client'
import type {
  AuditRunDetail,
  BackupReport,
  BackupRunOverview,
  ImportBatchDetail,
  ImportBatchOverview,
  SnapshotRestorePreview,
} from '../../../lib/types'
import { useAuditData } from './use-audit-data'

const labels = {
  commonUnavailable: 'Unavailable',
  importPreviewUnavailable: 'Import preview unavailable',
  restoreConfirm: 'Restore this batch?',
  restoreRecorded: 'Restore recorded',
  revertConfirm: 'Revert this batch?',
  revertRecorded: 'Revert recorded',
  runDetailUnavailable: 'Run detail unavailable',
}

describe('useAuditData', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    vi.spyOn(window, 'confirm').mockReturnValue(true)
  })

  test('loads run detail, run index cache, and related import batch detail', async () => {
    const recentRuns = [runFixture(1, 'import'), runFixture(2, 'backup')]
    const recentImportBatches = [batchFixture(10)]
    vi.spyOn(backend, 'loadAuditRunDetail').mockImplementation((runId) =>
      Promise.resolve(
        detailFixture(runFixture(runId, runId === 1 ? 'import' : 'backup')),
      ),
    )
    vi.spyOn(backend, 'previewImportBatch').mockResolvedValue(
      batchDetailFixture(recentImportBatches[0]),
    )

    const { result } = renderHook(() =>
      useAuditData({
        labels,
        recentImportBatches,
        recentRuns,
        refreshAppData: vi.fn().mockResolvedValue(undefined),
        refreshKey: 1,
        runId: 1,
        selectRun: vi.fn(),
      }),
    )

    await waitFor(() => expect(result.current.detail?.run.id).toBe(1))
    await waitFor(() =>
      expect(result.current.relatedBatchDetail?.batch.id).toBe(10),
    )
    expect(result.current.detailSeverity).toBe('warning')
    expect(result.current.detailCache[1]?.run.id).toBe(1)
    expect(result.current.detailCache[2]?.run.id).toBe(2)
    expect(result.current.loading).toBe(false)
    expect(result.current.relatedImportBatch?.id).toBe(10)
  })

  test('preserves copy feedback for artifact paths', async () => {
    const batch = batchFixture(10)
    const recentRuns = [runFixture(1, 'import')]
    const recentImportBatches = [batch]
    vi.spyOn(backend, 'loadAuditRunDetail').mockResolvedValue(
      detailFixture(runFixture(1, 'import')),
    )
    vi.spyOn(backend, 'previewImportBatch').mockResolvedValue(
      batchDetailFixture(batch),
    )
    const originalClipboard = navigator.clipboard
    const writeText = vi.fn(() => Promise.resolve(undefined))
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    })

    try {
      const { result } = renderHook(() =>
        useAuditData({
          labels,
          recentImportBatches,
          recentRuns,
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          refreshKey: 1,
          runId: 1,
          selectRun: vi.fn(),
        }),
      )

      await waitFor(() => expect(result.current.detail?.run.id).toBe(1))

      await act(async () => {
        await result.current.handleCopyPath('/audit/run-1.json')
      })
      expect(writeText).toHaveBeenCalledWith('/audit/run-1.json')
      expect(result.current.copyFeedback).toEqual({
        key: '/audit/run-1.json',
        tone: 'success',
      })
    } finally {
      Object.defineProperty(navigator, 'clipboard', {
        configurable: true,
        value: originalClipboard,
      })
    }
  })

  test('ignores run index cache results after unmount', async () => {
    let resolveRun: (detail: AuditRunDetail) => void = () => {}
    const runDetailPromise = new Promise<AuditRunDetail>((resolve) => {
      resolveRun = resolve
    })
    vi.spyOn(backend, 'loadAuditRunDetail').mockReturnValue(runDetailPromise)
    const recentImportBatches: ImportBatchOverview[] = []
    const recentRuns = [runFixture(1, 'backup')]

    const { unmount } = renderHook(() =>
      useAuditData({
        labels,
        recentImportBatches,
        recentRuns,
        refreshAppData: vi.fn().mockResolvedValue(undefined),
        refreshKey: 1,
        runId: null,
        selectRun: vi.fn(),
      }),
    )

    unmount()

    await act(async () => {
      resolveRun(detailFixture(runFixture(1, 'backup')))
      await runDetailPromise
    })
  })

  test('ignores run-detail and related-batch completions after unmount', async () => {
    const detailDeferred = deferred<AuditRunDetail>()
    vi.spyOn(backend, 'loadAuditRunDetail').mockReturnValue(
      detailDeferred.promise,
    )
    const noRecentImportBatches: ImportBatchOverview[] = []
    const noRecentRuns: BackupRunOverview[] = []

    const detailHook = renderHook(() =>
      useAuditData({
        labels,
        recentImportBatches: noRecentImportBatches,
        recentRuns: noRecentRuns,
        refreshAppData: vi.fn().mockResolvedValue(undefined),
        refreshKey: 1,
        runId: 1,
        selectRun: vi.fn(),
      }),
    )

    detailHook.unmount()
    await act(async () => {
      detailDeferred.resolve(detailFixture(runFixture(1, 'backup')))
      await detailDeferred.promise
    })

    const rejectedDetail = deferred<AuditRunDetail>()
    vi.spyOn(backend, 'loadAuditRunDetail').mockReturnValue(
      rejectedDetail.promise,
    )

    const rejectedHook = renderHook(() =>
      useAuditData({
        labels,
        recentImportBatches: noRecentImportBatches,
        recentRuns: noRecentRuns,
        refreshAppData: vi.fn().mockResolvedValue(undefined),
        refreshKey: 2,
        runId: 2,
        selectRun: vi.fn(),
      }),
    )

    rejectedHook.unmount()
    await act(async () => {
      rejectedDetail.reject(new Error('late detail failure'))
      await rejectedDetail.promise.catch(() => undefined)
    })

    const batch = batchFixture(10)
    const recentRuns = [runFixture(1, 'import')]
    const recentImportBatches = [batch]
    const relatedBatchDeferred = deferred<ImportBatchDetail>()
    vi.spyOn(backend, 'loadAuditRunDetail').mockResolvedValue(
      detailFixture(runFixture(1, 'import')),
    )
    vi.spyOn(backend, 'previewImportBatch').mockReturnValue(
      relatedBatchDeferred.promise,
    )

    const relatedBatchHook = renderHook(() =>
      useAuditData({
        labels,
        recentImportBatches,
        recentRuns,
        refreshAppData: vi.fn().mockResolvedValue(undefined),
        refreshKey: 3,
        runId: 1,
        selectRun: vi.fn(),
      }),
    )

    await waitFor(() =>
      expect(backend.previewImportBatch).toHaveBeenCalledWith(10),
    )
    relatedBatchHook.unmount()
    await act(async () => {
      relatedBatchDeferred.resolve(batchDetailFixture(batch))
      await relatedBatchDeferred.promise
    })

    const rejectedRelatedBatch = deferred<ImportBatchDetail>()
    vi.spyOn(backend, 'previewImportBatch').mockReturnValue(
      rejectedRelatedBatch.promise,
    )
    const rejectedRelatedBatchHook = renderHook(() =>
      useAuditData({
        labels,
        recentImportBatches,
        recentRuns,
        refreshAppData: vi.fn().mockResolvedValue(undefined),
        refreshKey: 4,
        runId: 1,
        selectRun: vi.fn(),
      }),
    )

    await waitFor(() =>
      expect(backend.previewImportBatch).toHaveBeenLastCalledWith(10),
    )
    rejectedRelatedBatchHook.unmount()
    await act(async () => {
      rejectedRelatedBatch.reject(new Error('late batch failure'))
      await rejectedRelatedBatch.promise.catch(() => undefined)
    })
  })

  test('clears run index cache when there are no recent runs', async () => {
    vi.spyOn(backend, 'loadAuditRunDetail').mockResolvedValue(
      detailFixture(runFixture(1, 'backup')),
    )

    const { result, rerender } = renderHook(
      (props: Parameters<typeof useAuditData>[0]) => useAuditData(props),
      {
        initialProps: {
          labels,
          recentImportBatches: [],
          recentRuns: [runFixture(1, 'backup')],
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          refreshKey: 1,
          runId: null,
          selectRun: vi.fn(),
        },
      },
    )

    await waitFor(() => expect(result.current.detailCache[1]?.run.id).toBe(1))

    rerender({
      labels,
      recentImportBatches: [],
      recentRuns: [],
      refreshAppData: vi.fn().mockResolvedValue(undefined),
      refreshKey: 2,
      runId: null,
      selectRun: vi.fn(),
    })

    await waitFor(() => expect(result.current.detailCache).toEqual({}))
  })

  test('surfaces run-detail and related-batch fallback errors', async () => {
    vi.spyOn(backend, 'loadAuditRunDetail').mockRejectedValueOnce('offline')
    const detailFailureImportBatches: ImportBatchOverview[] = []
    const detailFailureRuns = [runFixture(1, 'backup')]

    const detailFailure = renderHook(() =>
      useAuditData({
        labels,
        recentImportBatches: detailFailureImportBatches,
        recentRuns: detailFailureRuns,
        refreshAppData: vi.fn().mockResolvedValue(undefined),
        refreshKey: 1,
        runId: 1,
        selectRun: vi.fn(),
      }),
    )

    await waitFor(() =>
      expect(detailFailure.result.current.error).toBe('Run detail unavailable'),
    )
    detailFailure.unmount()

    vi.spyOn(backend, 'loadAuditRunDetail').mockResolvedValue(
      detailFixture(runFixture(1, 'import')),
    )
    vi.spyOn(backend, 'previewImportBatch').mockRejectedValue('batch offline')
    const batchFailureImportBatches = [batchFixture(10)]
    const batchFailureRuns = [runFixture(1, 'import')]

    const batchFailure = renderHook(() =>
      useAuditData({
        labels,
        recentImportBatches: batchFailureImportBatches,
        recentRuns: batchFailureRuns,
        refreshAppData: vi.fn().mockResolvedValue(undefined),
        refreshKey: 1,
        runId: 1,
        selectRun: vi.fn(),
      }),
    )

    await waitFor(() =>
      expect(batchFailure.result.current.relatedBatchError).toBe(
        'Import preview unavailable',
      ),
    )
    expect(batchFailure.result.current.relatedBatchDetail).toBeNull()

    vi.spyOn(backend, 'loadAuditRunDetail').mockRejectedValueOnce(
      new Error('detail exploded'),
    )
    const noRecentImportBatches: ImportBatchOverview[] = []
    const noRecentRuns: BackupRunOverview[] = []
    const detailError = renderHook(() =>
      useAuditData({
        labels,
        recentImportBatches: noRecentImportBatches,
        recentRuns: noRecentRuns,
        refreshAppData: vi.fn().mockResolvedValue(undefined),
        refreshKey: 2,
        runId: 2,
        selectRun: vi.fn(),
      }),
    )
    await waitFor(() =>
      expect(detailError.result.current.error).toBe('detail exploded'),
    )
    detailError.unmount()

    vi.spyOn(backend, 'loadAuditRunDetail').mockResolvedValue(
      detailFixture(runFixture(1, 'import')),
    )
    vi.spyOn(backend, 'previewImportBatch').mockRejectedValue(
      new Error('batch exploded'),
    )
    const batchErrorRuns = [runFixture(1, 'import')]
    const batchErrorBatches = [batchFixture(10)]
    const batchError = renderHook(() =>
      useAuditData({
        labels,
        recentImportBatches: batchErrorBatches,
        recentRuns: batchErrorRuns,
        refreshAppData: vi.fn().mockResolvedValue(undefined),
        refreshKey: 3,
        runId: 1,
        selectRun: vi.fn(),
      }),
    )
    await waitFor(() =>
      expect(batchError.result.current.relatedBatchError).toBe(
        'batch exploded',
      ),
    )
  })

  test('handles related import batch revert/restore confirmations and errors', async () => {
    const refreshAppData = vi.fn().mockResolvedValue(undefined)
    const batch = batchFixture(10)
    const recentRuns = [runFixture(1, 'import')]
    const recentImportBatches = [batch]
    vi.spyOn(backend, 'loadAuditRunDetail').mockResolvedValue(
      detailFixture(runFixture(1, 'import')),
    )
    vi.spyOn(backend, 'previewImportBatch').mockResolvedValue(
      batchDetailFixture(batch),
    )
    const revertBatch = vi
      .spyOn(backend, 'revertImportBatch')
      .mockResolvedValue(batchDetailFixture({ ...batch, status: 'reverted' }))
    const restoreBatch = vi
      .spyOn(backend, 'restoreImportBatch')
      .mockRejectedValueOnce(new Error('restore failed'))
      .mockResolvedValue(batchDetailFixture({ ...batch, status: 'imported' }))

    const { result } = renderHook(() =>
      useAuditData({
        labels,
        recentImportBatches,
        recentRuns,
        refreshAppData,
        refreshKey: 1,
        runId: 1,
        selectRun: vi.fn(),
      }),
    )

    await waitFor(() =>
      expect(result.current.relatedBatchDetail?.batch.id).toBe(10),
    )

    vi.mocked(window.confirm).mockReturnValueOnce(false)
    await act(async () => {
      await result.current.handleRelatedBatchMutation('revert')
    })
    expect(revertBatch).not.toHaveBeenCalled()

    await act(async () => {
      await result.current.handleRelatedBatchMutation('revert')
    })
    expect(revertBatch).toHaveBeenCalledWith(10)
    expect(result.current.batchActionNotice).toBe('Revert recorded')
    expect(refreshAppData).toHaveBeenCalledTimes(1)

    await act(async () => {
      await result.current.handleRelatedBatchMutation('restore')
    })
    expect(restoreBatch).toHaveBeenCalledWith(10)
    expect(result.current.batchActionError).toBe('restore failed')

    await act(async () => {
      await result.current.handleRelatedBatchMutation('restore')
    })
    expect(result.current.batchActionNotice).toBe('Restore recorded')
  })

  test('handles related batch mutation before detail and non-error failures', async () => {
    const batch = batchFixture(10)
    const recentRuns = [runFixture(1, 'import')]
    const recentImportBatches = [batch]
    const batchDeferred = deferred<ImportBatchDetail>()
    const revertBatch = vi
      .spyOn(backend, 'revertImportBatch')
      .mockRejectedValueOnce('revert offline')
    vi.spyOn(backend, 'loadAuditRunDetail').mockResolvedValue(
      detailFixture(runFixture(1, 'import')),
    )
    vi.spyOn(backend, 'previewImportBatch').mockReturnValueOnce(
      batchDeferred.promise,
    )

    const { result } = renderHook(() =>
      useAuditData({
        labels,
        recentImportBatches,
        recentRuns,
        refreshAppData: vi.fn().mockResolvedValue(undefined),
        refreshKey: 1,
        runId: 1,
        selectRun: vi.fn(),
      }),
    )

    await waitFor(() =>
      expect(backend.previewImportBatch).toHaveBeenCalledWith(10),
    )
    await act(async () => {
      await result.current.handleRelatedBatchMutation('revert')
    })
    expect(revertBatch).not.toHaveBeenCalled()

    await act(async () => {
      batchDeferred.resolve(batchDetailFixture(batch))
      await batchDeferred.promise
    })
    await waitFor(() =>
      expect(result.current.relatedBatchDetail?.batch.id).toBe(10),
    )

    await act(async () => {
      await result.current.handleRelatedBatchMutation('revert')
    })
    expect(result.current.batchActionError).toBe('Unavailable')
  })

  test('previews and executes snapshot restore with unsupported and failing branches', async () => {
    const refreshAppData = vi.fn().mockResolvedValue(undefined)
    const selectRun = vi.fn()
    const recentImportBatches: ImportBatchOverview[] = []
    const recentRuns = [runFixture(1, 'backup')]
    vi.spyOn(backend, 'loadAuditRunDetail').mockResolvedValue(
      detailFixture(runFixture(1, 'backup')),
    )
    const previewRestore = vi
      .spyOn(backend, 'previewSnapshotRestore')
      .mockResolvedValueOnce(restorePreviewFixture({ executeSupported: false }))
      .mockResolvedValueOnce(restorePreviewFixture({ executeSupported: true }))
      .mockRejectedValueOnce(new Error('preview failed'))
    const runRestore = vi
      .spyOn(backend, 'runSnapshotRestore')
      .mockResolvedValueOnce(backupReportFixture(runFixture(77, 'restore')))
      .mockRejectedValueOnce(new Error('restore failed'))

    const { result } = renderHook(() =>
      useAuditData({
        labels,
        recentImportBatches,
        recentRuns,
        refreshAppData,
        refreshKey: 1,
        runId: 1,
        selectRun,
      }),
    )

    await waitFor(() => expect(result.current.detail?.run.id).toBe(1))

    await act(async () => {
      await result.current.handlePreviewRestore('/snapshots/one.sqlite')
    })
    expect(previewRestore).toHaveBeenCalledWith({
      snapshotPath: '/snapshots/one.sqlite',
    })
    expect(result.current.restorePreview?.executeSupported).toBe(false)

    await act(async () => {
      await result.current.handleExecuteRestore()
    })
    expect(runRestore).not.toHaveBeenCalled()

    await act(async () => {
      await result.current.handlePreviewRestore('/snapshots/two.sqlite')
    })
    await act(async () => {
      await result.current.handleExecuteRestore()
    })
    expect(runRestore).toHaveBeenCalledWith({
      snapshotPath: '/snapshots/two.sqlite',
    })
    expect(refreshAppData).toHaveBeenCalledTimes(1)
    expect(selectRun).toHaveBeenCalledWith(77)
    expect(result.current.restoreNotice).toBe('Restore recorded')

    await act(async () => {
      await result.current.handlePreviewRestore('/snapshots/bad.sqlite')
    })
    expect(result.current.restoreError).toBe('preview failed')

    previewRestore.mockRejectedValueOnce('preview offline')
    await act(async () => {
      await result.current.handlePreviewRestore('/snapshots/string-bad.sqlite')
    })
    expect(result.current.restoreError).toBe('Unavailable')

    act(() => {
      result.current.setDetailTab('warnings')
    })
    expect(result.current.detailTab).toBe('warnings')

    previewRestore.mockResolvedValueOnce(
      restorePreviewFixture({ executeSupported: true }),
    )
    await act(async () => {
      await result.current.handlePreviewRestore('/snapshots/two.sqlite')
    })
    await waitFor(() =>
      expect(result.current.restorePreview?.executeSupported).toBe(true),
    )
    await act(async () => {
      await result.current.handleExecuteRestore()
    })
    expect(result.current.restoreError).toBe('restore failed')
    expect(result.current.restoreBusy).toBe(false)

    previewRestore.mockResolvedValueOnce(
      restorePreviewFixture({
        snapshotPath: '/snapshots/no-run.sqlite',
        executeSupported: true,
      }),
    )
    runRestore.mockResolvedValueOnce({
      ...backupReportFixture(runFixture(88, 'restore')),
      run: null,
    } as BackupReport)
    await act(async () => {
      await result.current.handlePreviewRestore('/snapshots/no-run.sqlite')
    })
    await act(async () => {
      await result.current.handleExecuteRestore()
    })
    expect(selectRun).toHaveBeenCalledTimes(1)

    previewRestore.mockResolvedValueOnce(
      restorePreviewFixture({
        snapshotPath: '/snapshots/string-restore-bad.sqlite',
        executeSupported: true,
      }),
    )
    runRestore.mockRejectedValueOnce('restore offline')
    await act(async () => {
      await result.current.handlePreviewRestore(
        '/snapshots/string-restore-bad.sqlite',
      )
    })
    await act(async () => {
      await result.current.handleExecuteRestore()
    })
    expect(result.current.restoreError).toBe('Unavailable')
  })
})

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((nextResolve, nextReject) => {
    resolve = nextResolve
    reject = nextReject
  })
  return { promise, resolve, reject }
}

function runFixture(id: number, runType: string): BackupRunOverview {
  return {
    id,
    startedAt: `2026-04-25T0${id}:00:00.000Z`,
    finishedAt: `2026-04-25T0${id}:01:00.000Z`,
    status: 'success',
    runType,
    trigger: 'manual',
    profileScope: ['chrome:Default'],
    manifestHash: `hash-${id}`,
    profilesProcessed: 1,
    newVisits: 2,
    newUrls: 2,
    newDownloads: 0,
  }
}

function detailFixture(run: BackupRunOverview): AuditRunDetail {
  return {
    run,
    trigger: run.trigger ?? 'manual',
    timezone: 'UTC',
    dueOnly: false,
    profileScope: run.profileScope ?? [],
    warnings: run.runType === 'import' ? ['review imported records'] : [],
    errorMessage: null,
    stats: {},
    manifestPath: `/audit/run-${run.id}.json`,
    manifestHash: run.manifestHash,
    artifacts: [
      {
        kind: 'manifest',
        path: `/audit/run-${run.id}.json`,
        checksum: run.manifestHash,
        sizeBytes: 128,
        createdAt: run.finishedAt ?? run.startedAt,
      },
    ],
  }
}

function batchFixture(id: number): ImportBatchOverview {
  return {
    id,
    sourceKind: 'browser-history',
    sourcePath:
      '/Users/test/Library/Application Support/Chrome/Default/History',
    profileId: 'chrome:Default',
    createdAt: '2026-04-25T00:59:00.000Z',
    importedAt: '2026-04-25T01:00:30.000Z',
    revertedAt: null,
    status: 'imported',
    candidateItems: 2,
    importedItems: 2,
    duplicateItems: 0,
    visibleItems: 2,
    auditPath: '/audit/import-10.json',
    gitCommit: null,
  }
}

function batchDetailFixture(batch: ImportBatchOverview): ImportBatchDetail {
  return {
    batch,
    previewEntries: [],
    recognizedFiles: [],
    quarantinedFiles: [],
    notes: [],
  }
}

function restorePreviewFixture(
  overrides: Partial<SnapshotRestorePreview> = {},
): SnapshotRestorePreview {
  return {
    snapshotPath: '/snapshots/two.sqlite',
    snapshotKind: 'checkpoint',
    sourceRunId: 1,
    sourceProfileId: 'chrome:Default',
    sourceBrowserName: 'Google Chrome',
    createdAt: '2026-04-25T01:01:00.000Z',
    reason: 'test',
    executeSupported: true,
    estimatedVisits: 2,
    estimatedUrls: 2,
    estimatedDownloads: 0,
    warnings: [],
    ...overrides,
  }
}

function backupReportFixture(run: BackupRunOverview): BackupReport {
  return {
    dueSkipped: false,
    reason: null,
    run,
    profiles: [],
    manifestPath: `/audit/run-${run.id}.json`,
    gitCommit: null,
    warnings: [],
    remoteBackup: null,
  }
}
