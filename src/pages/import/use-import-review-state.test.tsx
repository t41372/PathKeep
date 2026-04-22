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
  })
})
