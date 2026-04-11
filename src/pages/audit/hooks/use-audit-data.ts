import { useEffect, useMemo, useState } from 'react'
import { backend } from '../../../lib/backend-client'
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

interface UseAuditDataOptions {
  labels: {
    commonUnavailable: string
    copyFailed: string
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
  const [copyFeedback, setCopyFeedback] = useState<{
    path: string
    tone: 'success' | 'error'
  } | null>(null)
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
    const loadDetail = async () => {
      try {
        const response = await backend.loadAuditRunDetail(runId)
        if (!cancelled) setDetailState({ runId, detail: response, error: null })
      } catch (error) {
        if (cancelled) return
        setDetailState({
          runId,
          detail: null,
          error:
            error instanceof Error
              ? error.message
              : labels.runDetailUnavailable,
        })
      }
    }
    void loadDetail()
    return () => {
      cancelled = true
    }
  }, [labels.runDetailUnavailable, refreshKey, runId])

  useEffect(() => {
    if (!recentRuns.length) {
      setDetailCache({})
      return
    }

    let cancelled = false
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
        setRelatedBatchError(
          error instanceof Error
            ? error.message
            : labels.importPreviewUnavailable,
        )
      }
    }

    void loadRelatedBatch()
    return () => {
      cancelled = true
    }
  }, [labels.importPreviewUnavailable, relatedImportBatch])

  async function handleCopyPath(path: string) {
    try {
      if (!navigator.clipboard?.writeText) {
        throw new Error(labels.copyFailed)
      }
      await navigator.clipboard.writeText(path)
      setCopyFeedback({ path, tone: 'success' })
    } catch {
      setCopyFeedback({ path, tone: 'error' })
    }
  }

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
      setBatchActionError(
        error instanceof Error ? error.message : labels.commonUnavailable,
      )
    }
  }

  async function handlePreviewRestore(snapshotPath: string) {
    setRestoreBusy(true)
    setRestoreError(null)
    setRestoreNotice(null)
    try {
      const preview = await backend.previewSnapshotRestore({ snapshotPath })
      setRestorePreview(preview)
    } catch (error) {
      setRestorePreview(null)
      setRestoreError(
        error instanceof Error ? error.message : labels.commonUnavailable,
      )
    } finally {
      setRestoreBusy(false)
    }
  }

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
      setRestoreError(
        error instanceof Error ? error.message : labels.commonUnavailable,
      )
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
