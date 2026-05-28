/**
 * @file shell-runtime-status.ts
 * @description Shared shell hook for the AI queue and Core Intelligence runtime status read model.
 * @module app/shell-runtime-status
 *
 * ## Responsibilities
 * - Own the shell-level runtime status state published through `useShellData()`.
 * - Deduplicate in-flight queue/runtime refreshes for the same unlocked archive scope.
 * - Poll active background work on a short cadence and back off once queues are idle.
 *
 * ## Not responsible for
 * - Rendering Jobs, Sidebar, or Settings runtime panels.
 * - Mutating queue controls such as pause, retry, or cancel actions.
 * - Owning app bootstrap, dashboard refresh, backup progress, or app-lock actions.
 *
 * ## Dependencies
 * - Depends on the backend client for the two runtime read-model commands.
 * - Depends on shell-data helper functions for scope keys, idle status, and active-work counting.
 * - Consumed by `src/app/shell-data.tsx`, which remains the public provider facade.
 *
 * ## Performance notes
 * - Runtime polling is the shared source for multiple surfaces, so this hook avoids duplicate route-level polling.
 * - Active queues poll every 3 seconds; idle queues poll every 15 seconds to reduce command traffic.
 * - In-flight refreshes are reused per archive/profile/pause-state scope to avoid parallel backend reads.
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { backend } from '../lib/backend-client'
import { describeError } from '../lib/errors'
import type { AppSnapshot } from '../lib/types'
import type { ShellRuntimeStatus } from './shell-data-context'
import {
  countActiveRuntimeJobs,
  emptyRuntimeStatus,
  runtimeStatusScopeKey,
  type ShellTranslator,
} from './shell-data-helpers'

const UNAVAILABLE_RUNTIME_STATUS = emptyRuntimeStatus()

interface ShellRuntimeStatusOptions {
  snapshot: AppSnapshot | null
  refreshKey: number
  t: ShellTranslator
}

/**
 * Builds the single shell-owned runtime read model used by background-job surfaces.
 *
 * The hook accepts the current archive snapshot and shell refresh key so it can reset
 * immediately for locked/uninitialized archives, dedupe manual refreshes, and keep a
 * bounded polling loop warm only while background queues have active work.
 */
export function useShellRuntimeStatus({
  snapshot,
  refreshKey,
}: ShellRuntimeStatusOptions) {
  const [runtimeStatus, setRuntimeStatus] =
    useState<ShellRuntimeStatus>(emptyRuntimeStatus)
  const runtimeSnapshotAvailable =
    snapshot !== null &&
    snapshot.config.initialized &&
    snapshot.archiveStatus.unlocked
  const runtimeRefreshPromiseRef = useRef<Promise<ShellRuntimeStatus> | null>(
    null,
  )
  const runtimeRefreshScopeKeyRef = useRef<string | null>(null)
  // Stryker disable ArrayDeclaration: any constant dependency array preserves this stable ref/setState callback contract.
  const resetRuntimeStatus = useCallback(() => {
    const nextStatus = emptyRuntimeStatus()
    runtimeRefreshPromiseRef.current = null
    runtimeRefreshScopeKeyRef.current = runtimeStatusScopeKey(null)
    setRuntimeStatus(nextStatus)
    return nextStatus
  }, [])
  // Stryker restore ArrayDeclaration

  const refreshRuntimeStatus = useCallback(
    async (nextSnapshot: AppSnapshot | null = snapshot) => {
      const nextScopeKey = runtimeStatusScopeKey(nextSnapshot)
      if (
        !nextSnapshot?.config.initialized ||
        !nextSnapshot.archiveStatus.unlocked
      ) {
        const nextStatus = emptyRuntimeStatus()
        runtimeRefreshPromiseRef.current = null
        runtimeRefreshScopeKeyRef.current = nextScopeKey
        setRuntimeStatus(nextStatus)
        return nextStatus
      }

      if (
        runtimeRefreshPromiseRef.current &&
        runtimeRefreshScopeKeyRef.current === nextScopeKey
      ) {
        return runtimeRefreshPromiseRef.current
      }

      const shouldKeepCurrentStatus =
        runtimeRefreshScopeKeyRef.current === nextScopeKey
      setRuntimeStatus((current) => ({
        ...(shouldKeepCurrentStatus ? current : emptyRuntimeStatus()),
        loading: true,
        error: null,
      }))

      const nextRequest = Promise.all([
        backend.loadAiQueueStatus(),
        backend.loadIntelligenceRuntime(),
      ])
        .then(([nextAiQueue, nextRuntime]) => {
          const nextStatus: ShellRuntimeStatus = {
            aiQueue: nextAiQueue,
            intelligence: nextRuntime,
            loading: false,
            error: null,
          }
          if (runtimeRefreshPromiseRef.current === nextRequest) {
            setRuntimeStatus(nextStatus)
          }
          return nextStatus
        })
        .catch((nextError) => {
          const message = describeError(nextError, 'load_runtime_status')
          const nextStatus: ShellRuntimeStatus = {
            aiQueue: null,
            intelligence: null,
            loading: false,
            error: message,
          }
          if (runtimeRefreshPromiseRef.current === nextRequest) {
            setRuntimeStatus(nextStatus)
          }
          return nextStatus
        })
        .finally(() => {
          if (runtimeRefreshPromiseRef.current === nextRequest) {
            runtimeRefreshPromiseRef.current = null
          }
        })

      runtimeRefreshScopeKeyRef.current = nextScopeKey
      runtimeRefreshPromiseRef.current = nextRequest
      return nextRequest
    },
    [snapshot],
  )

  useEffect(() => {
    if (!runtimeSnapshotAvailable) {
      runtimeRefreshPromiseRef.current = null
      runtimeRefreshScopeKeyRef.current = runtimeStatusScopeKey(snapshot)
      return
    }

    let cancelled = false
    let timeoutId: number | null = null

    const scheduleNext = (delayMs: number) => {
      timeoutId = window.setTimeout(() => {
        void load()
      }, delayMs)
    }

    const load = async () => {
      const next = await refreshRuntimeStatus(snapshot)
      if (cancelled) {
        return
      }
      scheduleNext(countActiveRuntimeJobs(next) > 0 ? 3000 : 15000)
    }

    void load()

    return () => {
      cancelled = true
      if (timeoutId !== null) {
        window.clearTimeout(timeoutId)
      }
    }
  }, [refreshKey, refreshRuntimeStatus, runtimeSnapshotAvailable, snapshot])

  return {
    runtimeStatus: runtimeSnapshotAvailable
      ? runtimeStatus
      : UNAVAILABLE_RUNTIME_STATUS,
    refreshRuntimeStatus,
    resetRuntimeStatus,
  }
}
