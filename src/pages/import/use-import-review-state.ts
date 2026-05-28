/**
 * @file use-import-review-state.ts
 * @description Canonical follow-through state owner for Import batch review, doctor/repair flows, and audit-path support actions.
 * @module pages/import
 *
 * ## Responsibilities
 * - Own selected-batch deep-link sync and loaded batch-detail lifecycle.
 * - Own doctor/repair and revert/restore review state for the Import route.
 * - Keep review-surface error and copy feedback in one route-local owner.
 *
 * ## Not responsible for
 * - Scanning or importing a new source path.
 * - Rendering the wizard or review panels.
 * - Defining shared cross-route support-action primitives.
 *
 * ## Dependencies
 * - Depends on `backend-client` for preview, doctor, repair, revert, restore, and open-path calls.
 * - Depends on `shared.ts` helpers for URL batch parsing and active-batch fallback derivation.
 * - Uses the shared clipboard helper instead of hand-rolling review copy feedback.
 *
 * ## Performance notes
 * - Keeps one selected-batch preview owner inside the Import route so follow-through review does not fan out duplicate preview reads.
 * - Only manages light route-local state; heavy import work still lives in backend commands.
 */

import { useCallback, useEffect, useMemo, useState } from 'react'
import { useSearchParams } from 'react-router-dom'
import {
  copyReviewValue,
  type ReviewCopyFeedback,
} from '../../components/review'
import { backend } from '../../lib/backend-client'
import { describeError } from '../../lib/errors'
import type {
  HealthReport,
  ImportBatchDetail,
  ImportBatchOverview,
  TakeoutInspection,
} from '../../lib/types'
import {
  deriveActiveImportBatchDetail,
  parseImportBatchId,
  resolveSelectedImportBatchId,
  type ImportTranslate,
} from './shared'

interface UseImportReviewStateArgs {
  importResult: TakeoutInspection | null
  recentImportBatches: ImportBatchOverview[] | null | undefined
  refreshAppData: () => Promise<void>
  t: ImportTranslate
}

/**
 * Composes the Import route's review/follow-through state into one focused hook.
 *
 * `ImportPage` stays readable by delegating batch review, doctor/repair, and
 * support-path state here while still owning the scan/import workflow itself.
 */
export function useImportReviewState({
  importResult,
  recentImportBatches,
  refreshAppData,
  t,
}: UseImportReviewStateArgs) {
  const [searchParams, setSearchParams] = useSearchParams()
  const [actionError, setActionError] = useState<string | null>(null)
  const [selectedBatchId, setSelectedBatchId] = useState<number | null>(null)
  const [selectedBatchDetail, setSelectedBatchDetail] =
    useState<ImportBatchDetail | null>(null)
  const [loadingBatch, setLoadingBatch] = useState(false)
  const [healthReport, setHealthReport] = useState<HealthReport | null>(null)
  const [repairNotice, setRepairNotice] = useState<string | null>(null)
  const [supportCopyFeedback, setSupportCopyFeedback] =
    useState<ReviewCopyFeedback | null>(null)

  const batchIdFromParams = useMemo(
    () => parseImportBatchId(searchParams),
    [searchParams],
  )

  useEffect(() => {
    const recentBatches = recentImportBatches ?? []
    const nextBatchId = resolveSelectedImportBatchId(
      recentBatches,
      batchIdFromParams,
      null,
    )

    if (nextBatchId === null) {
      setSelectedBatchId(null)
      setSelectedBatchDetail(null)
      return
    }

    setSelectedBatchId((currentBatchId) =>
      resolveSelectedImportBatchId(
        recentBatches,
        batchIdFromParams,
        currentBatchId,
      ),
    )
  }, [batchIdFromParams, recentImportBatches])

  useEffect(() => {
    if (!selectedBatchId) {
      return
    }

    const currentBatch = searchParams.get('batch')
    if (currentBatch === String(selectedBatchId)) {
      return
    }

    const nextParams = new URLSearchParams(searchParams)
    nextParams.set('batch', String(selectedBatchId))
    setSearchParams(nextParams, { replace: true })
  }, [searchParams, selectedBatchId, setSearchParams])

  const reportActionError = useCallback((nextError: unknown) => {
    setActionError(describeError(nextError, 'import_review_action'))
  }, [])

  const clearActionError = useCallback(() => {
    setActionError(null)
  }, [])

  useEffect(() => {
    if (!selectedBatchId) {
      setSelectedBatchDetail(null)
      return
    }

    let cancelled = false

    const loadBatch = async () => {
      setSelectedBatchDetail(null)
      clearActionError()
      setLoadingBatch(true)
      try {
        const detail = await backend.previewImportBatch(selectedBatchId)
        if (!cancelled) {
          setSelectedBatchDetail(detail)
        }
      } catch (nextError) {
        if (!cancelled) {
          setSelectedBatchDetail(null)
          reportActionError(nextError)
        }
      } finally {
        if (!cancelled) {
          setLoadingBatch(false)
        }
      }
    }

    void loadBatch()
    return () => {
      cancelled = true
    }
  }, [clearActionError, reportActionError, selectedBatchId])

  const activeBatchDetail = useMemo(
    () => deriveActiveImportBatchDetail(selectedBatchDetail, importResult),
    [importResult, selectedBatchDetail],
  )

  const selectBatchId = useCallback((batchId: number | null) => {
    setSelectedBatchId(batchId)
  }, [])

  const setLoadedBatchDetail = useCallback(
    (detail: ImportBatchDetail | null) => {
      setSelectedBatchDetail(detail)
    },
    [],
  )

  const handleRunDoctor = useCallback(async () => {
    clearActionError()
    try {
      const report = await backend.doctor()
      setHealthReport(report)
      setRepairNotice(null)
    } catch (nextError) {
      reportActionError(nextError)
    }
  }, [clearActionError, reportActionError])

  const handleRepairHealth = useCallback(async () => {
    clearActionError()
    try {
      const report = await backend.repairHealth()
      setRepairNotice(
        t('import.repairSummary', {
          derivedRows: report.clearedDerivedRows,
          visibilityRows: report.repairedVisibilityRows,
          importAudits: report.repairedImportAudits,
        }),
      )
      const nextHealthReport = await backend.doctor()
      setHealthReport(nextHealthReport)
    } catch (nextError) {
      reportActionError(nextError)
    }
  }, [clearActionError, reportActionError, t])

  const handleBatchMutation = useCallback(
    async (batch: ImportBatchOverview, action: 'revert' | 'restore') => {
      const message =
        action === 'revert'
          ? t('import.revertConfirm')
          : t('import.restoreConfirm')

      if (typeof window !== 'undefined' && 'confirm' in window) {
        if (!window.confirm(message)) {
          return
        }
      }

      clearActionError()
      try {
        const detail =
          action === 'revert'
            ? await backend.revertImportBatch(batch.id)
            : await backend.restoreImportBatch(batch.id)
        await refreshAppData()
        setSelectedBatchId(detail.batch.id)
        setSelectedBatchDetail(detail)
      } catch (nextError) {
        reportActionError(nextError)
      }
    },
    [clearActionError, refreshAppData, reportActionError, t],
  )

  const handleSupportPathCopy = useCallback(
    async (key: string, value: string) => {
      await copyReviewValue(value, {
        key,
        onFeedback: setSupportCopyFeedback,
      })
    },
    [],
  )

  const handleSupportPathOpen = useCallback((path: string) => {
    void backend.openPathInFileManager(path)
  }, [])

  return {
    actionError,
    activeBatchDetail,
    clearActionError,
    handleBatchMutation,
    handleRepairHealth,
    handleRunDoctor,
    handleSupportPathCopy,
    handleSupportPathOpen,
    healthReport,
    loadingBatch,
    repairNotice,
    reportActionError,
    selectBatchId,
    selectedBatchId,
    setLoadedBatchDetail,
    supportCopyFeedback,
  }
}
