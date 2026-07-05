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
    key?: string | null,
  ) => Promise<FullArchiveRestoreReport>
  /** Called after a successful restore (e.g. to reload the snapshot list). */
  onSuccess?: () => void | Promise<void>
  /**
   * Seeds the archive-key field (e.g. a password the user already typed at the
   * unlock gate). Purely an initial value — never re-read after the first render.
   */
  initialKey?: string
}

export interface SnapshotRestoreState {
  confirming: RecoverySnapshot | null
  restoring: boolean
  restoreError: string | null
  restoreSucceeded: boolean
  /** The archive key the user has entered for an encrypted-snapshot restore. */
  archiveKey: string
  /** Updates the archive-key field. */
  setArchiveKey: (value: string) => void
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
  initialKey,
}: UseSnapshotRestoreOptions): SnapshotRestoreState {
  const [confirming, setConfirming] = useState<RecoverySnapshot | null>(null)
  const [restoring, setRestoring] = useState(false)
  const [restoreError, setRestoreError] = useState<string | null>(null)
  const [restoreSucceeded, setRestoreSucceeded] = useState(false)
  // The archive key for an encrypted-snapshot restore. Seeded from `initialKey`
  // (e.g. the password the user already typed at the unlock gate). Held only in
  // component state and NEVER logged — it flows straight into the restore call.
  const [archiveKey, setArchiveKey] = useState(initialKey ?? '')

  const startRestore = useCallback((snap: RecoverySnapshot) => {
    // Do NOT clear archiveKey here: a key the user already typed carries into the
    // confirm step so they never have to re-enter it when picking a snapshot.
    setConfirming(snap)
    setRestoreError(null)
    setRestoreSucceeded(false)
  }, [])

  const cancelRestore = useCallback(() => {
    setConfirming(null)
    setRestoreError(null)
    setRestoreSucceeded(false)
    // Leaving the flow: never keep the key around.
    setArchiveKey('')
  }, [])

  const confirmRestore = useCallback(
    async (snap: RecoverySnapshot) => {
      // Re-entry guard: block a destructive double-submit while a restore is in flight.
      if (restoring) return
      setRestoring(true)
      setRestoreError(null)
      try {
        // Pass the trimmed key (or null for a plaintext snapshot / empty field).
        const trimmedKey = archiveKey.trim()
        await runFullArchiveRestore(snap.path, trimmedKey ? trimmedKey : null)
        // On success: the recovery screen unmounts (runFullArchiveRestore clears `recovery`
        // via refreshAppData), so these resets are harmless there. The settings section stays
        // mounted, so it must leave the restoring/confirm state and surface the success notice.
        setRestoring(false)
        setConfirming(null)
        setRestoreSucceeded(true)
        // Never keep the key past a successful restore.
        setArchiveKey('')
        void onSuccess?.()
      } catch (err) {
        // Restore failed: surface the error and stop the spinner. `confirming` and
        // `archiveKey` are left untouched (not cleared here) — every surface hides
        // the confirm/key step while an error is shown and offers only "Try another
        // snapshot" (resetError), which clears both. We simply don't clear state
        // mid-error; the key itself is never logged.
        setRestoreError(describeError(err, 'archive_restore'))
        setRestoring(false)
      }
    },
    [restoring, runFullArchiveRestore, onSuccess, archiveKey],
  )

  const resetError = useCallback(() => {
    setRestoreError(null)
    setConfirming(null)
    setRestoreSucceeded(false)
    // Leaving the flow: never keep the key around.
    setArchiveKey('')
  }, [])

  return {
    confirming,
    restoring,
    restoreError,
    restoreSucceeded,
    archiveKey,
    setArchiveKey,
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
