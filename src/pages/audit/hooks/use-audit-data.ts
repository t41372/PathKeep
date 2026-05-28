/**
 * This module contains route-level hooks that support the Audit surface.
 *
 * Why this file exists:
 * - Route files are where PathKeep turns design-system primitives, desktop read models, and shell scope into user-facing workflow.
 * - They should make deep links, trust copy, loading states, and repair actions obvious without forcing readers to reconstruct the whole page mentally.
 *
 * Main declarations:
 * - `useAuditData`
 *
 * Source-of-truth notes:
 * - Stay aligned with `docs/design/screens-and-nav.md` for route purpose, navigation, and shared profile-scope rules.
 * - Stay aligned with `docs/design/ux-principles.md` for PME, trust warning grammar, and the no-hidden-state loading contract.
 */

import { useEffect, useMemo, useState } from 'react'
import {
  copyReviewValue,
  type ReviewCopyFeedback,
} from '../../../components/review'
import { backend } from '../../../lib/backend-client'
import { describeError } from '../../../lib/errors'
import { auditSeverity } from '../../../lib/trust-review'
import type {
  AuditRunDetail,
  BackupRunOverview,
  ImportBatchDetail,
  ImportBatchOverview,
  SnapshotRestorePreview,
} from '../../../lib/types'
import {
  pickRelatedImportBatch,
  type AuditDetailState,
  type AuditDetailTab,
} from '../types'

/**
 * Collects the inputs needed by `UseAuditData`.
 *
 * Keeping this as a named declaration makes the Audit surface easier to review and test than burying the behavior inside another anonymous callback.
 */
interface UseAuditDataOptions {
  labels: {
    commonUnavailable: string
    importPreviewUnavailable: string
    restoreConfirm: string
    restoreRecorded: string
    revertConfirm: string
    revertRecorded: string
    runDetailUnavailable: string
  }
  recentImportBatches: ImportBatchOverview[]
  recentRuns: BackupRunOverview[]
  refreshAppData: () => Promise<void>
  refreshKey: number
  runId: number | null
  selectRun: (runId: number) => void
}

/**
 * Provides the `useAuditData` hook.
 *
 * Keeping this as a named declaration makes the Audit surface easier to review and test than burying the behavior inside another anonymous callback.
 */
export function useAuditData({
  labels,
  recentImportBatches,
  recentRuns,
  refreshAppData,
  refreshKey,
  runId,
  selectRun,
}: UseAuditDataOptions) {
  const [detailState, setDetailState] = useState<AuditDetailState>({
    runId: null,
    detail: null,
    error: null,
  })
  const [copyFeedback, setCopyFeedback] = useState<ReviewCopyFeedback | null>(
    null,
  )
  const [detailCache, setDetailCache] = useState<
    Record<number, AuditRunDetail>
  >({})
  const [relatedBatchDetail, setRelatedBatchDetail] =
    useState<ImportBatchDetail | null>(null)
  const [relatedBatchError, setRelatedBatchError] = useState<string | null>(
    null,
  )
  const [batchActionError, setBatchActionError] = useState<string | null>(null)
  const [batchActionNotice, setBatchActionNotice] = useState<string | null>(
    null,
  )
  const [detailTab, setDetailTab] = useState<AuditDetailTab>('summary')
  const [restorePreview, setRestorePreview] =
    useState<SnapshotRestorePreview | null>(null)
  const [restoreError, setRestoreError] = useState<string | null>(null)
  const [restoreNotice, setRestoreNotice] = useState<string | null>(null)
  const [restoreBusy, setRestoreBusy] = useState(false)

  useEffect(() => {
    if (!runId) return
    let cancelled = false
    /**
     * Loads detail.
     *
     * Keeping this as a named declaration makes the Audit surface easier to review and test than burying the behavior inside another anonymous callback.
     */
    const loadDetail = async () => {
      try {
        const response = await backend.loadAuditRunDetail(runId)
        if (!cancelled) setDetailState({ runId, detail: response, error: null })
      } catch (error) {
        if (cancelled) return
        setDetailState({
          runId,
          detail: null,
          error: describeError(error, 'load_audit_run_detail'),
        })
      }
    }
    void loadDetail()
    return () => {
      cancelled = true
    }
  }, [refreshKey, runId])

  useEffect(() => {
    if (!recentRuns.length) {
      setDetailCache({})
      return
    }

    let cancelled = false
    /**
     * Loads run index.
     *
     * Keeping this as a named declaration makes the Audit surface easier to review and test than burying the behavior inside another anonymous callback.
     */
    const loadRunIndex = async () => {
      const entries = await Promise.allSettled(
        recentRuns.map(
          async (run) =>
            [run.id, await backend.loadAuditRunDetail(run.id)] as const,
        ),
      )
      if (cancelled) {
        return
      }
      const nextCache: Record<number, AuditRunDetail> = {}
      for (const entry of entries) {
        if (entry.status !== 'fulfilled') {
          continue
        }
        const [nextRunId, nextDetail] = entry.value
        nextCache[nextRunId] = nextDetail
      }
      setDetailCache(nextCache)
    }

    void loadRunIndex()
    return () => {
      cancelled = true
    }
  }, [recentRuns, refreshKey])

  const detail = detailState.runId === runId ? detailState.detail : null
  const error = detailState.runId === runId ? detailState.error : null
  const loading = Boolean(runId) && detailState.runId !== runId
  const detailSeverity = detail ? auditSeverity(detail) : null
  const relatedImportBatch = useMemo(
    () => pickRelatedImportBatch(detail, recentImportBatches),
    [detail, recentImportBatches],
  )
  const loadingRelatedBatch =
    Boolean(relatedImportBatch) &&
    relatedBatchDetail?.batch.id !== relatedImportBatch?.id &&
    !relatedBatchError

  useEffect(() => {
    setDetailTab('summary')
    setRestorePreview(null)
    setRestoreError(null)
    setRestoreNotice(null)
  }, [runId])

  useEffect(() => {
    setBatchActionError(null)
    setBatchActionNotice(null)
  }, [runId])

  useEffect(() => {
    if (!relatedImportBatch) {
      setRelatedBatchDetail(null)
      setRelatedBatchError(null)
      return
    }

    let cancelled = false
    /**
     * Loads related batch.
     *
     * Keeping this as a named declaration makes the Audit surface easier to review and test than burying the behavior inside another anonymous callback.
     */
    const loadRelatedBatch = async () => {
      try {
        setRelatedBatchError(null)
        const response = await backend.previewImportBatch(relatedImportBatch.id)
        if (!cancelled) {
          setRelatedBatchDetail(response)
        }
      } catch (error) {
        if (cancelled) return
        setRelatedBatchDetail(null)
        setRelatedBatchError(describeError(error, 'preview_import_batch'))
      }
    }

    void loadRelatedBatch()
    return () => {
      cancelled = true
    }
  }, [relatedImportBatch])

  /**
   * Handles copy path.
   *
   * Keeping this as a named declaration makes the Audit surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleCopyPath(path: string) {
    await copyReviewValue(path, {
      key: path,
      onFeedback: setCopyFeedback,
    })
  }

  /**
   * Handles related batch mutation.
   *
   * Keeping this as a named declaration makes the Audit surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleRelatedBatchMutation(action: 'revert' | 'restore') {
    if (!relatedBatchDetail) return
    const message =
      action === 'revert' ? labels.revertConfirm : labels.restoreConfirm

    if (typeof window !== 'undefined' && 'confirm' in window) {
      if (!window.confirm(message)) {
        return
      }
    }

    setBatchActionError(null)
    setBatchActionNotice(null)
    try {
      const response =
        action === 'revert'
          ? await backend.revertImportBatch(relatedBatchDetail.batch.id)
          : await backend.restoreImportBatch(relatedBatchDetail.batch.id)
      setRelatedBatchDetail(response)
      setBatchActionNotice(
        action === 'revert' ? labels.revertRecorded : labels.restoreRecorded,
      )
      await refreshAppData()
    } catch (error) {
      setBatchActionError(describeError(error, `${action}_import_batch`))
    }
  }

  /**
   * Handles preview restore.
   *
   * Keeping this as a named declaration makes the Audit surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handlePreviewRestore(snapshotPath: string) {
    setRestoreBusy(true)
    setRestoreError(null)
    setRestoreNotice(null)
    try {
      const preview = await backend.previewSnapshotRestore({ snapshotPath })
      setRestorePreview(preview)
    } catch (error) {
      setRestorePreview(null)
      setRestoreError(describeError(error, 'preview_snapshot_restore'))
    } finally {
      setRestoreBusy(false)
    }
  }

  /**
   * Handles execute restore.
   *
   * Keeping this as a named declaration makes the Audit surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleExecuteRestore() {
    if (!restorePreview?.executeSupported) {
      return
    }
    setRestoreBusy(true)
    setRestoreError(null)
    setRestoreNotice(null)
    try {
      const report = await backend.runSnapshotRestore({
        snapshotPath: restorePreview.snapshotPath,
      })
      await refreshAppData()
      setRestoreNotice(labels.restoreRecorded)
      if (report.run?.id) {
        selectRun(report.run.id)
      }
    } catch (error) {
      setRestoreError(describeError(error, 'run_snapshot_restore'))
    } finally {
      setRestoreBusy(false)
    }
  }

  return {
    batchActionError,
    batchActionNotice,
    copyFeedback,
    detail,
    detailCache,
    detailSeverity,
    detailTab,
    error,
    handleCopyPath,
    handleExecuteRestore,
    handlePreviewRestore,
    handleRelatedBatchMutation,
    loading,
    loadingRelatedBatch,
    relatedBatchDetail,
    relatedBatchError,
    relatedImportBatch,
    restoreBusy,
    restoreError,
    restoreNotice,
    restorePreview,
    setDetailTab,
  }
}
