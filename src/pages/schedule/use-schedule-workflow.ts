/**
 * @file use-schedule-workflow.ts
 * @description Route-owned workflow state for Scheduled Backup Settings.
 * @module pages/schedule
 *
 * ## Responsibilities
 * - Load schedule preview/status and expose the current state-machine inputs.
 * - Own install, update, remove, repair, verify, and re-detect progress.
 * - Keep interval persistence tied to the same action flow that applies native scheduler artifacts.
 *
 * ## Not responsible for
 * - Rendering panels, buttons, or copy.
 * - Translating backend issue keys into user-visible labels.
 * - Running arbitrary shell commands outside the existing schedule IPC surface.
 *
 * ## Dependencies
 * - `useShellData` for config persistence and profile selection.
 * - `backend` schedule commands for native scheduler actions.
 * - `waitForNextPaint` so inline progress can paint before long native work.
 *
 * ## Performance notes
 * - The hook performs at most one preview/status pair per detection cycle and
 *   never scans archive history itself.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useShellData } from '../../app/shell-data-context'
import { backend } from '../../lib/backend-client'
import { waitForNextPaint } from '../../lib/wait-for-next-paint'
import type {
  AppSnapshot,
  ApplyResult,
  SchedulePlan,
  ScheduleStatus,
} from '../../lib/types'
import {
  deriveScheduleUiState,
  visibleScheduleIssues,
  type ScheduleUiState,
} from './schedule-ui-state'

/**
 * Names every route action that can place the Scheduled Backup Settings page in a busy/result state.
 *
 * The route keeps these names explicit so action feedback, tests, and user
 * copy do not drift when a new scheduler recovery path is added.
 */
export type ScheduleOperationKind =
  | 'detect'
  | 'refresh'
  | 'install'
  | 'update'
  | 'remove'
  | 'repair'
  | 'verify'
  | 'copy-diagnostics'

/**
 * Provides enough progress metadata for multi-step native scheduler actions.
 *
 * The UI displays this inline instead of blocking the route with modal progress.
 */
export interface ScheduleOperationProgress {
  kind: ScheduleOperationKind
  current: number
  total: number
  messageKey: string
}

/**
 * Records the visible result for the last user-triggered scheduler action.
 *
 * Keeping this route-local avoids leaking backend English warning strings into
 * the long-lived schedule read model.
 */
export interface ScheduleActionResult {
  kind: ScheduleOperationKind
  status: 'success' | 'error'
  message: string
  auditPath?: string | null
  at: Date
}

interface ScheduleLoadState {
  requestKey: number
  plan: SchedulePlan | null
  status: ScheduleStatus | null
  error: string | null
}

/**
 * Provides the full Scheduled Backup Settings workflow contract to the route.
 */
export function useScheduleWorkflow() {
  const { refreshAppData, refreshKey, saveConfig, snapshot } = useShellData()
  const mountedRef = useRef(true)
  const [loadState, setLoadState] = useState<ScheduleLoadState>({
    requestKey: -1,
    plan: null,
    status: null,
    error: null,
  })
  const [draftDueAfterHours, setDraftDueAfterHours] = useState<number | null>(
    null,
  )
  const [operation, setOperation] = useState<ScheduleOperationProgress | null>(
    null,
  )
  const [actionResult, setActionResult] = useState<ScheduleActionResult | null>(
    null,
  )
  const [dismissedIssues, setDismissedIssues] = useState<Set<string>>(
    () => new Set(),
  )
  const [lastCheckedAt, setLastCheckedAt] = useState<Date | null>(null)

  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
    }
  }, [])

  const detectSchedule = useCallback(
    async (kind: ScheduleOperationKind = 'detect') => {
      setOperation({
        kind,
        current: 1,
        total: 1,
        messageKey: 'schedule.detectingStatus',
      })
      setLoadState((current) => ({
        ...current,
        requestKey: -1,
        error: null,
      }))
      try {
        await waitForNextPaint()
        const [nextPlan, nextStatus] = await Promise.all([
          backend.previewSchedule(),
          backend.scheduleStatus(),
        ])
        /* v8 ignore next -- React teardown guard; mounted paths are covered by workflow tests. */
        if (!mountedRef.current) return
        const checkedAt = nextStatus.checkedAt
          ? new Date(nextStatus.checkedAt)
          : new Date()
        setLastCheckedAt(checkedAt)
        setLoadState({
          requestKey: refreshKey,
          plan: nextPlan,
          status: nextStatus,
          error: null,
        })
        if (kind === 'verify' || kind === 'detect') {
          setActionResult({
            kind,
            status: 'success',
            message: 'schedule.detectComplete',
            auditPath: nextStatus.auditPath,
            at: checkedAt,
          })
        }
      } catch (nextError) {
        setLoadState({
          requestKey: refreshKey,
          plan: null,
          status: null,
          error:
            nextError instanceof Error
              ? nextError.message
              : 'schedule.unavailableBody',
        })
        setActionResult({
          kind,
          status: 'error',
          message:
            nextError instanceof Error
              ? nextError.message
              : 'schedule.unavailableBody',
          at: new Date(),
        })
      } finally {
        /* v8 ignore next -- React teardown guard; mounted paths are covered by workflow tests. */
        if (mountedRef.current) {
          setOperation(null)
        }
      }
    },
    [refreshKey],
  )

  useEffect(() => {
    void detectSchedule('detect')
  }, [detectSchedule])

  useEffect(() => {
    const nextDueAfterHours =
      snapshot?.config.dueAfterHours ?? loadState.status?.dueAfterHours ?? null
    if (nextDueAfterHours !== null) {
      setDraftDueAfterHours(nextDueAfterHours)
    }
  }, [loadState.status?.dueAfterHours, snapshot?.config.dueAfterHours])

  const persistedDueAfterHours =
    snapshot?.config.dueAfterHours ?? loadState.status?.dueAfterHours ?? null
  const selectedDueAfterHours =
    draftDueAfterHours ??
    persistedDueAfterHours ??
    loadState.status?.dueAfterHours ??
    24
  const intervalDirty =
    persistedDueAfterHours !== null &&
    selectedDueAfterHours !== persistedDueAfterHours

  const plan = loadState.requestKey === refreshKey ? loadState.plan : null
  const status = loadState.requestKey === refreshKey ? loadState.status : null
  const loading = loadState.requestKey !== refreshKey

  const visibleIssues = useMemo(
    () => visibleScheduleIssues(status?.issues ?? [], dismissedIssues),
    [dismissedIssues, status?.issues],
  )
  const hasNeverRun = !(
    status?.lastSuccessfulBackupAt ??
    snapshot?.archiveStatus.lastSuccessfulBackupAt
  )
  const uiState: ScheduleUiState = deriveScheduleUiState({
    hasNeverRun,
    loading,
    status: status
      ? {
          ...status,
          issues: visibleIssues,
        }
      : null,
  })

  const persistInterval = useCallback(async (): Promise<AppSnapshot> => {
    if (!snapshot?.config) {
      throw new Error('schedule.initializeArchiveFirst')
    }
    const nextSnapshot = await saveConfig({
      ...snapshot.config,
      dueAfterHours: selectedDueAfterHours,
    })
    setDraftDueAfterHours(nextSnapshot.config.dueAfterHours)
    return nextSnapshot
  }, [saveConfig, selectedDueAfterHours, snapshot?.config])

  const runNativeAction = useCallback(
    async (kind: Exclude<ScheduleOperationKind, 'detect' | 'verify'>) => {
      if (!plan) return
      const total = kind === 'install' || kind === 'update' ? 3 : 2
      setOperation({
        kind,
        current: 1,
        total,
        messageKey: operationStartKey(kind),
      })
      setActionResult(null)

      try {
        await waitForNextPaint()
        let planForAction = plan
        if ((kind === 'install' || kind === 'update') && intervalDirty) {
          await persistInterval()
          /* v8 ignore next -- React teardown guard; mounted paths are covered by workflow tests. */
          if (!mountedRef.current) return
          setOperation({
            kind,
            current: 2,
            total,
            messageKey: 'schedule.progressRefreshingPlan',
          })
          planForAction = await backend.previewSchedule()
          /* v8 ignore next -- React teardown guard; mounted paths are covered by workflow tests. */
          if (!mountedRef.current) return
        }

        /* v8 ignore next -- React teardown guard; mounted paths are covered by workflow tests. */
        if (!mountedRef.current) return
        setOperation({
          kind,
          current: total,
          total,
          messageKey: operationExecuteKey(kind),
        })

        let result: ApplyResult
        if (kind === 'remove') {
          result = await backend.removeSchedule(planForAction)
        } else if (kind === 'repair') {
          result = await backend.repairSchedule(planForAction)
        } else {
          result = await backend.applySchedule(planForAction)
        }

        /* v8 ignore next -- React teardown guard; mounted paths are covered by workflow tests. */
        if (!mountedRef.current) return
        setActionResult({
          kind,
          status: result.applied || kind === 'remove' ? 'success' : 'error',
          message: result.message,
          auditPath: result.auditPath,
          at: new Date(),
        })
        await refreshAppData()
        /* v8 ignore next -- React teardown guard; mounted paths are covered by workflow tests. */
        if (!mountedRef.current) return
        await detectSchedule('refresh')
      } catch (nextError) {
        /* v8 ignore next -- React teardown guard; mounted paths are covered by workflow tests. */
        if (!mountedRef.current) return
        setActionResult({
          kind,
          status: 'error',
          message:
            nextError instanceof Error
              ? nextError.message
              : 'schedule.operationFailed',
          at: new Date(),
        })
      } finally {
        /* v8 ignore next -- React teardown guard; mounted paths are covered by workflow tests. */
        if (mountedRef.current) {
          setOperation(null)
        }
      }
    },
    [detectSchedule, intervalDirty, persistInterval, plan, refreshAppData],
  )

  const copyDiagnostics = useCallback(async () => {
    if (!status) return
    setOperation({
      kind: 'copy-diagnostics',
      current: 1,
      total: 1,
      messageKey: 'schedule.progressCopyingDiagnostics',
    })
    setActionResult(null)
    try {
      await waitForNextPaint()
      if (!navigator.clipboard?.writeText) {
        throw new Error('schedule.diagnosticsClipboardUnavailable')
      }
      await navigator.clipboard.writeText(
        JSON.stringify(
          {
            checkedAt: diagnosticsCheckedAt(status, lastCheckedAt),
            installState: status.installState,
            issues: status.issues ?? [],
            label: status.label,
            platform: status.platform,
            plan: plan
              ? {
                  label: plan.label,
                  platform: plan.platform,
                  generatedFiles: plan.generatedFiles.map((file) => ({
                    path: file.absolutePath ?? file.relativePath,
                    purpose: file.purpose,
                  })),
                }
              : null,
            verificationChecks: status.verificationChecks ?? [],
          },
          null,
          2,
        ),
      )
      /* v8 ignore next -- React teardown guard; mounted paths are covered by workflow tests. */
      if (!mountedRef.current) return
      setActionResult({
        kind: 'copy-diagnostics',
        status: 'success',
        message: 'schedule.diagnosticsCopied',
        at: new Date(),
      })
    } catch (nextError) {
      /* v8 ignore next -- React teardown guard; mounted paths are covered by workflow tests. */
      if (!mountedRef.current) return
      setActionResult({
        kind: 'copy-diagnostics',
        status: 'error',
        message:
          nextError instanceof Error
            ? nextError.message
            : 'schedule.diagnosticsCopyFailed',
        at: new Date(),
      })
    } finally {
      /* v8 ignore next -- React teardown guard; mounted paths are covered by workflow tests. */
      if (mountedRef.current) {
        setOperation(null)
      }
    }
  }, [lastCheckedAt, plan, status])

  const dismissIssue = useCallback((code: string) => {
    setDismissedIssues((current) => new Set([...current, code]))
  }, [])

  return {
    actionResult,
    copyDiagnostics,
    detectSchedule,
    dismissIssue,
    draftDueAfterHours: selectedDueAfterHours,
    error: loadState.error,
    hasNeverRun,
    intervalDirty,
    lastCheckedAt,
    loading,
    operation,
    plan,
    runNativeAction,
    setDraftDueAfterHours,
    snapshot,
    status,
    uiState,
    visibleIssues,
  }
}

function operationStartKey(kind: ScheduleOperationKind): string {
  if (kind === 'remove') return 'schedule.progressRemoving'
  if (kind === 'repair') return 'schedule.progressRepairing'
  return 'schedule.progressSaving'
}

function operationExecuteKey(kind: ScheduleOperationKind): string {
  if (kind === 'remove') return 'schedule.progressRemovingNative'
  if (kind === 'repair') return 'schedule.progressRepairingNative'
  return 'schedule.progressInstallingNative'
}

function diagnosticsCheckedAt(
  status: ScheduleStatus,
  lastCheckedAt: Date | null,
): string | null {
  if (status.checkedAt) return status.checkedAt
  /* v8 ignore next -- status is only set after detection records a fallback timestamp. */
  if (lastCheckedAt) return lastCheckedAt.toISOString()
  /* v8 ignore next -- copy diagnostics is only available after detection records a timestamp. */
  return null
}
