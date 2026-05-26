/**
 * @file use-import-review-state.test.tsx
 * @description Focused hook-level regression coverage for the Import route's follow-through state owner.
 * @module pages/import
 *
 * ## Responsibilities
 * - Verify selected-batch deep links load the expected review detail.
 * - Verify revert/restore follow-through stays wired after moving review state into a focused hook.
 *
 * ## Not responsible for
 * - Re-testing the full Import route workflow or panel rendering.
 * - Covering scan/import mutation behavior that remains with the route shell.
 *
 * ## Dependencies
 * - Depends on MemoryRouter because the hook owns `useSearchParams` deep-link sync.
 * - Mocks backend review mutations directly instead of mounting the full route.
 *
 * ## Performance notes
 * - Hook-level tests keep the new owner honest without paying for full route harness setup on every branch.
 */

import { act, renderHook, waitFor } from '@testing-library/react'
import type { ReactNode } from 'react'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import type { ImportBatchDetail, ImportBatchOverview } from '../../lib/types'
import { useImportReviewState } from './use-import-review-state'

const translate = (key: string, vars?: Record<string, string | number>) => {
  switch (key) {
    case 'common.unavailable':
      return 'Unavailable'
    case 'import.revertConfirm':
      return 'Revert this batch?'
    case 'import.restoreConfirm':
      return 'Restore this batch?'
    case 'import.repairSummary':
      return `${vars?.derivedRows ?? 0}/${vars?.visibilityRows ?? 0}/${vars?.importAudits ?? 0}`
    default:
      return key
  }
}

const recentBatches: ImportBatchOverview[] = [
  {
    id: 1,
    sourceKind: 'takeout',
    sourcePath: '/tmp/one',
    profileId: 'takeout::browser-history',
    createdAt: '2026-04-21T10:00:00.000Z',
    importedAt: '2026-04-21T10:01:00.000Z',
    revertedAt: null,
    status: 'imported',
    candidateItems: 2,
    importedItems: 2,
    duplicateItems: 0,
    visibleItems: 2,
    auditPath: '/tmp/one-audit.json',
    gitCommit: null,
  },
  {
    id: 2,
    sourceKind: 'takeout',
    sourcePath: '/tmp/two',
    profileId: 'takeout::browser-history',
    createdAt: '2026-04-21T11:00:00.000Z',
    importedAt: '2026-04-21T11:01:00.000Z',
    revertedAt: null,
    status: 'imported',
    candidateItems: 4,
    importedItems: 3,
    duplicateItems: 1,
    visibleItems: 3,
    auditPath: '/tmp/two-audit.json',
    gitCommit: null,
  },
]

const detailFor = (batch: ImportBatchOverview): ImportBatchDetail => ({
  batch,
  previewEntries: [],
  recognizedFiles: [],
  quarantinedFiles: [],
  notes: [],
})

function createWrapper(initialEntry: string) {
  return function Wrapper({ children }: { children: ReactNode }) {
    return (
      <MemoryRouter initialEntries={[initialEntry]}>{children}</MemoryRouter>
    )
  }
}

describe('useImportReviewState', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    vi.spyOn(window, 'confirm').mockReturnValue(true)
  })

  test('loads the requested batch from the route search params', async () => {
    vi.spyOn(backend, 'previewImportBatch').mockResolvedValue(
      detailFor(recentBatches[1]),
    )

    const { result } = renderHook(
      () =>
        useImportReviewState({
          importResult: null,
          recentImportBatches: recentBatches,
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          t: translate,
        }),
      {
        wrapper: createWrapper('/import?batch=2'),
      },
    )

    await waitFor(() => expect(result.current.selectedBatchId).toBe(2))
    await waitFor(() =>
      expect(result.current.activeBatchDetail?.batch.id).toBe(2),
    )
    expect(backend.previewImportBatch).toHaveBeenCalledWith(2)
  })

  test('does not auto-select the newest batch on initial load', async () => {
    const previewSpy = vi.spyOn(backend, 'previewImportBatch')

    const { result } = renderHook(
      () =>
        useImportReviewState({
          importResult: null,
          recentImportBatches: recentBatches,
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          t: translate,
        }),
      {
        wrapper: createWrapper('/import'),
      },
    )

    await waitFor(() => expect(result.current.selectedBatchId).toBeNull())
    expect(result.current.activeBatchDetail).toBeNull()
    expect(previewSpy).not.toHaveBeenCalled()
  })

  test('keeps missing recent-batch snapshots inert', async () => {
    const previewSpy = vi.spyOn(backend, 'previewImportBatch')

    const { result } = renderHook(
      () =>
        useImportReviewState({
          importResult: null,
          recentImportBatches: undefined,
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          t: translate,
        }),
      {
        wrapper: createWrapper('/import'),
      },
    )

    await waitFor(() => expect(result.current.selectedBatchId).toBeNull())
    expect(result.current.activeBatchDetail).toBeNull()
    expect(previewSpy).not.toHaveBeenCalled()
  })

  test('drops late batch preview completion after unmount', async () => {
    let rejectPreview!: (error: Error) => void
    vi.spyOn(backend, 'previewImportBatch').mockReturnValue(
      new Promise<ImportBatchDetail>((_resolve, reject) => {
        rejectPreview = reject
      }),
    )

    const { unmount } = renderHook(
      () =>
        useImportReviewState({
          importResult: null,
          recentImportBatches: recentBatches,
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          t: translate,
        }),
      {
        wrapper: createWrapper('/import?batch=1'),
      },
    )

    unmount()

    await act(async () => {
      rejectPreview(new Error('late preview failure'))
      await Promise.resolve()
    })
  })

  test('revert/restore follow-through refreshes app data and updates loaded detail', async () => {
    const refreshAppData = vi.fn().mockResolvedValue(undefined)
    vi.spyOn(backend, 'previewImportBatch').mockResolvedValue(
      detailFor(recentBatches[0]),
    )
    vi.spyOn(backend, 'revertImportBatch').mockResolvedValue(
      detailFor({
        ...recentBatches[0],
        status: 'reverted',
        revertedAt: '2026-04-21T12:00:00.000Z',
      }),
    )
    vi.spyOn(backend, 'restoreImportBatch').mockResolvedValue(
      detailFor({
        ...recentBatches[0],
        status: 'imported',
        revertedAt: null,
      }),
    )

    const { result } = renderHook(
      () =>
        useImportReviewState({
          importResult: null,
          recentImportBatches: recentBatches,
          refreshAppData,
          t: translate,
        }),
      {
        wrapper: createWrapper('/import?batch=1'),
      },
    )

    await waitFor(() =>
      expect(result.current.activeBatchDetail?.batch.status).toBe('imported'),
    )

    await act(async () => {
      await result.current.handleBatchMutation(recentBatches[0], 'revert')
    })

    expect(refreshAppData).toHaveBeenCalledTimes(1)
    await waitFor(() =>
      expect(result.current.activeBatchDetail?.batch.status).toBe('reverted'),
    )

    await act(async () => {
      await result.current.handleBatchMutation(recentBatches[0], 'restore')
    })

    expect(refreshAppData).toHaveBeenCalledTimes(2)
    await waitFor(() =>
      expect(result.current.activeBatchDetail?.batch.status).toBe('imported'),
    )
  })

  test('runs batch mutations when host confirm is unavailable', async () => {
    const refreshAppData = vi.fn().mockResolvedValue(undefined)
    vi.spyOn(backend, 'previewImportBatch').mockResolvedValue(
      detailFor(recentBatches[0]),
    )
    vi.spyOn(backend, 'revertImportBatch').mockResolvedValue(
      detailFor({
        ...recentBatches[0],
        status: 'reverted',
        revertedAt: '2026-04-21T12:00:00.000Z',
      }),
    )
    const originalConfirm = window.confirm
    Reflect.deleteProperty(window, 'confirm')

    try {
      const { result } = renderHook(
        () =>
          useImportReviewState({
            importResult: null,
            recentImportBatches: recentBatches,
            refreshAppData,
            t: translate,
          }),
        {
          wrapper: createWrapper('/import?batch=1'),
        },
      )

      await waitFor(() =>
        expect(result.current.activeBatchDetail?.batch.status).toBe('imported'),
      )

      await act(async () => {
        await result.current.handleBatchMutation(recentBatches[0], 'revert')
      })

      expect(backend.revertImportBatch).toHaveBeenCalledWith(1)
      expect(refreshAppData).toHaveBeenCalledTimes(1)
    } finally {
      Object.defineProperty(window, 'confirm', {
        configurable: true,
        value: originalConfirm,
      })
    }
  })

  test('runs doctor, repair, support path actions, and manual detail updates', async () => {
    const openPath = vi
      .spyOn(backend, 'openPathInFileManager')
      .mockResolvedValue('/tmp/two-audit.json')
    vi.spyOn(backend, 'doctor')
      .mockResolvedValueOnce({
        checks: [{ message: 'Initial', name: 'initial', status: 'ok' }],
        generatedAt: '2026-04-25T12:00:00Z',
      })
      .mockResolvedValueOnce({
        checks: [{ message: 'Repaired', name: 'repaired', status: 'ok' }],
        generatedAt: '2026-04-25T12:01:00Z',
      })
    vi.spyOn(backend, 'repairHealth').mockResolvedValue({
      clearedDerivedRows: 5,
      notes: [],
      repairedImportAudits: 7,
      repairedVisibilityRows: 6,
      runId: 9,
    })
    const originalClipboard = navigator.clipboard
    const writeText = vi.fn(() => Promise.resolve(undefined))
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    })

    try {
      const { result } = renderHook(
        () =>
          useImportReviewState({
            importResult: null,
            recentImportBatches: recentBatches,
            refreshAppData: vi.fn().mockResolvedValue(undefined),
            t: translate,
          }),
        {
          wrapper: createWrapper('/import'),
        },
      )

      act(() => {
        result.current.setLoadedBatchDetail(detailFor(recentBatches[1]))
      })
      expect(result.current.activeBatchDetail?.batch.id).toBe(2)

      await act(async () => {
        await result.current.handleRunDoctor()
      })
      expect(result.current.healthReport?.checks[0].name).toBe('initial')
      expect(result.current.repairNotice).toBeNull()

      await act(async () => {
        await result.current.handleRepairHealth()
      })
      expect(result.current.repairNotice).toBe('5/6/7')
      expect(result.current.healthReport?.checks[0].name).toBe('repaired')

      await act(async () => {
        await result.current.handleSupportPathCopy(
          'audit:path',
          '/tmp/two-audit.json',
        )
      })
      expect(writeText).toHaveBeenCalledWith('/tmp/two-audit.json')
      expect(result.current.supportCopyFeedback).toEqual({
        key: 'audit:path',
        tone: 'success',
      })

      act(() => {
        result.current.handleSupportPathOpen('/tmp/two-audit.json')
      })
      expect(openPath).toHaveBeenCalledWith('/tmp/two-audit.json')
    } finally {
      Object.defineProperty(navigator, 'clipboard', {
        configurable: true,
        value: originalClipboard,
      })
    }
  })

  test('keeps failed or cancelled review actions visible without mutating state', async () => {
    const refreshAppData = vi.fn().mockResolvedValue(undefined)
    vi.spyOn(backend, 'previewImportBatch').mockRejectedValue(
      new Error('preview failed'),
    )
    vi.spyOn(backend, 'restoreImportBatch').mockRejectedValue(
      new Error('restore failed'),
    )
    vi.spyOn(backend, 'doctor').mockRejectedValue(new Error('doctor failed'))
    vi.spyOn(backend, 'repairHealth').mockRejectedValue(
      new Error('repair failed'),
    )

    const { result } = renderHook(
      () =>
        useImportReviewState({
          importResult: null,
          recentImportBatches: recentBatches,
          refreshAppData,
          t: translate,
        }),
      {
        wrapper: createWrapper('/import?batch=1'),
      },
    )

    await waitFor(() =>
      expect(result.current.actionError).toBe('preview failed'),
    )

    act(() => {
      result.current.reportActionError('plain failure')
    })
    expect(result.current.actionError).toBe('plain failure')

    vi.mocked(window.confirm).mockReturnValueOnce(false)
    await act(async () => {
      await result.current.handleBatchMutation(recentBatches[0], 'restore')
    })
    expect(backend.restoreImportBatch).not.toHaveBeenCalled()
    expect(refreshAppData).not.toHaveBeenCalled()

    await act(async () => {
      await result.current.handleBatchMutation(recentBatches[0], 'restore')
    })
    expect(result.current.actionError).toBe('restore failed')

    act(() => {
      result.current.clearActionError()
    })
    expect(result.current.actionError).toBeNull()

    await act(async () => {
      await result.current.handleRunDoctor()
    })
    expect(result.current.actionError).toBe('doctor failed')

    await act(async () => {
      await result.current.handleRepairHealth()
    })
    expect(result.current.actionError).toBe('repair failed')
  })
})
