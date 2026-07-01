/**
 * @file use-snapshot-restore.ts
 * @description State machine hook for the snapshot restore flow.
 * @module components/snapshot-restore
 *
 * ## Responsibilities
 * - Own confirm / restoring / error state for restore flows.
 * - Provide a stable action API so both the recovery screen and settings section
 *   share one tested, consistent workflow.
 *
 * ## Not responsible for
 * - Rendering any UI; this is a pure hook.
 * - Loading the snapshot list from the backend.
 *
 * ## Dependencies
 * - `describeError` for user-facing error messages.
 *
 * ## Performance notes
 * - All state transitions are synchronous React updates.
 */

import { useCallback, useState } from 'react'
import { describeError } from '../../lib/errors'
import type {
  FullArchiveRestoreReport,
  RecoverySnapshot,
} from '../../lib/types'

export interface UseSnapshotRestoreOptions {
  /** Shell-level restore action (wraps `runFullArchiveRestore` IPC). */
  runFullArchiveRestore: (
    snapshotPath: string,
  ) => Promise<FullArchiveRestoreReport>
  /** Called after a successful restore (e.g. to reload the snapshot list). */
  onSuccess?: () => void | Promise<void>
}

export interface SnapshotRestoreState {
  confirming: RecoverySnapshot | null
  restoring: boolean
  restoreError: string | null
  restoreSucceeded: boolean
  startRestore: (snap: RecoverySnapshot) => void
  cancelRestore: () => void
  confirmRestore: (snap: RecoverySnapshot) => Promise<void>
  resetError: () => void
}

/**
 * Manages the confirm / restoring / error state machine for a snapshot restore
 * flow. Consumers supply the restore action and an optional success callback.
 */
export function useSnapshotRestore({
  runFullArchiveRestore,
  onSuccess,
}: UseSnapshotRestoreOptions): SnapshotRestoreState {
  const [confirming, setConfirming] = useState<RecoverySnapshot | null>(null)
  const [restoring, setRestoring] = useState(false)
  const [restoreError, setRestoreError] = useState<string | null>(null)
  const [restoreSucceeded, setRestoreSucceeded] = useState(false)

  const startRestore = useCallback((snap: RecoverySnapshot) => {
    setConfirming(snap)
    setRestoreError(null)
    setRestoreSucceeded(false)
  }, [])

  const cancelRestore = useCallback(() => {
    setConfirming(null)
    setRestoreError(null)
    setRestoreSucceeded(false)
  }, [])

  const confirmRestore = useCallback(
    async (snap: RecoverySnapshot) => {
      // Re-entry guard: block a destructive double-submit while a restore is in flight.
      if (restoring) return
      setRestoring(true)
      setRestoreError(null)
      try {
        await runFullArchiveRestore(snap.path)
        // On success: the recovery screen unmounts (runFullArchiveRestore clears `recovery`
        // via refreshAppData), so these resets are harmless there. The settings section stays
        // mounted, so it must leave the restoring/confirm state and surface the success notice.
        setRestoring(false)
        setConfirming(null)
        setRestoreSucceeded(true)
        void onSuccess?.()
      } catch (err) {
        setRestoreError(describeError(err, 'archive_restore'))
        setRestoring(false)
      }
    },
    [restoring, runFullArchiveRestore, onSuccess],
  )

  const resetError = useCallback(() => {
    setRestoreError(null)
    setConfirming(null)
    setRestoreSucceeded(false)
  }, [])

  return {
    confirming,
    restoring,
    restoreError,
    restoreSucceeded,
    startRestore,
    cancelRestore,
    confirmRestore,
    resetError,
  }
}

// ─── sourceOp label helper ───────────────────────────────────────────────────

/**
 * Resolves a human-readable label for the snapshot source operation.
 *
 * Maps the backend KNOWN_OPS (rekey | reconcile | import | periodic | unknown)
 * to localized strings. Uses dot-path keys resolved from the nested `sourceOp`
 * object in the recovery catalog.
 */
export function sourceOpLabel(
  sourceOp: string,
  t: (k: string) => string,
): string {
  const known: Record<string, string> = {
    rekey: t('sourceOp.rekey'),
    reconcile: t('sourceOp.reconcile'),
    import: t('sourceOp.import'),
    periodic: t('sourceOp.periodic'),
  }
  return known[sourceOp] ?? t('sourceOp.unknown')
}
