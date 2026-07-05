/**
 * @file recovery-panel.tsx
 * @description Shared snapshot-recovery panel: the reusable body behind both the unlock-gate
 * escape hatch and any other surface that needs a self-contained "restore from a snapshot" flow.
 * @module components/snapshot-restore
 *
 * ## Responsibilities
 * - Load the recovery snapshot list (with loading / error+retry / empty states).
 * - Own the restore state machine via `useSnapshotRestore`, seeded with an optional `initialKey`.
 * - Collect the archive key (via `ArchiveKeyField`) when the chosen snapshot is encrypted.
 * - Surface an honest, actionable restore error (role="alert") that keeps the user in the flow
 *   (retry the same snapshot after fixing the key, or pick another).
 * - Guarantee a PERSISTENT forward path ("Reveal logs") so the panel is never a dead end —
 *   even when the list is empty or every snapshot is encrypted and no key has been entered.
 *
 * ## Not responsible for
 * - Rendering the surrounding dialog shell / title (the unlock gate owns that).
 * - Deciding when recovery is required, or seeding the session key after a restore.
 * - Persisting or logging the archive key (it flows straight into the restore call).
 *
 * ## Dependencies
 * - `backend` client for `listRecoverySnapshots` and `revealLogs`.
 * - `useI18n('recovery')` for all copy.
 * - Shared `SnapshotCard`, `ArchiveKeyField`, `useSnapshotRestore`.
 *
 * ## Performance notes
 * - Single fetch on mount; no polling. The list is expected to be < 20 items (no virtualization).
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { backend } from '../../lib/backend-client'
import { describeError } from '../../lib/errors'
import { useI18n } from '../../lib/i18n'
import type {
  FullArchiveRestoreReport,
  RecoverySnapshot,
} from '../../lib/types'
import { ArchiveKeyField, SnapshotCard } from './index'
import { useSnapshotRestore } from './use-snapshot-restore'

interface SnapshotRecoveryPanelProps {
  runFullArchiveRestore: (
    snapshotPath: string,
    key?: string | null,
  ) => Promise<FullArchiveRestoreReport>
  onRestored?: () => void
  initialKey?: string
}

/**
 * Renders the reusable snapshot-recovery body. Loads the available snapshots, walks the
 * user through a keyed confirm/restore, and always keeps a "Reveal logs" forward path so
 * the surface can never dead-end.
 */
export function SnapshotRecoveryPanel({
  runFullArchiveRestore,
  onRestored,
  initialKey,
}: SnapshotRecoveryPanelProps) {
  const { t } = useI18n('recovery')

  const [snapshots, setSnapshots] = useState<RecoverySnapshot[]>([])
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState<string | null>(null)

  // Guard state writes against an unmount mid-fetch (the gate can unmount the panel
  // when a plaintext restore flips the archive to unlocked).
  const mountedRef = useRef(true)
  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
    }
  }, [])

  // ── Focus management ────────────────────────────────────────────────────────
  // This panel is embedded in the unlock gate as a modal over a still-focusable
  // shell. Every internal transition unmounts the currently-focused control (the
  // "Restore this" card button on confirm; the confirm/error controls on
  // cancel/reset), which would drop focus to document.body and let the next Tab
  // escape the modal onto a control BEHIND the locked gate. So we move focus
  // deterministically on each transition, keeping it inside the dialog at all
  // times (mirrors the launch ArchiveRecoveryScreen).
  const confirmKeyRef = useRef<HTMLInputElement | null>(null)
  const confirmButtonRef = useRef<HTMLButtonElement | null>(null)
  const retryButtonRef = useRef<HTMLButtonElement | null>(null)
  const restoringStatusRef = useRef<HTMLParagraphElement | null>(null)
  const firstCardButtonRef = useRef<HTMLButtonElement | null>(null)
  // Tracks whether we were in the confirm/error flow last render, so we can move
  // focus back to the list only on the RETURN transition (not on initial mount).
  const prevInFlowRef = useRef(false)

  const reload = useCallback(async () => {
    setLoading(true)
    setLoadError(null)
    try {
      const result = await backend.listRecoverySnapshots()
      if (mountedRef.current) {
        setSnapshots(result)
      }
    } catch (err) {
      if (mountedRef.current) {
        setLoadError(describeError(err, 'list_recovery_snapshots'))
      }
    } finally {
      if (mountedRef.current) {
        setLoading(false)
      }
    }
  }, [])

  useEffect(() => {
    void reload()
  }, [reload])

  const restore = useSnapshotRestore({
    runFullArchiveRestore,
    initialKey,
    onSuccess: () => {
      void reload()
      onRestored?.()
    },
  })

  // Confirm step opened: focus its primary control. For an encrypted snapshot
  // that is the key field (the thing the user must fill); for a plaintext one it
  // is the confirm button.
  useEffect(() => {
    if (restore.confirming && !restore.restoreError) {
      if (restore.confirming.encrypted) {
        confirmKeyRef.current?.focus?.()
      } else {
        confirmButtonRef.current?.focus?.()
      }
    }
  }, [restore.confirming, restore.restoreError])

  // Restore failed: move focus to the "Try another snapshot" control so keyboard
  // users land on the actionable affordance next to the (role="alert") error.
  useEffect(() => {
    if (restore.restoreError) {
      retryButtonRef.current?.focus?.()
    }
  }, [restore.restoreError])

  // Restore in flight: the focused confirm control becomes disabled, so move
  // focus to the status region (announces progress, keeps focus in the dialog).
  useEffect(() => {
    if (restore.restoring) {
      restoringStatusRef.current?.focus?.()
    }
  }, [restore.restoring])

  // Returned to the list (cancel / "Try another"): the confirm/error controls
  // unmount, so move focus to a stable list anchor — the first snapshot's restore
  // button — rather than letting it fall to document.body. Only fires on the
  // RETURN transition (guarded by prevInFlowRef) so it never steals initial focus.
  useEffect(() => {
    const inFlow = Boolean(restore.confirming) || Boolean(restore.restoreError)
    if (prevInFlowRef.current && !inFlow) {
      firstCardButtonRef.current?.focus?.()
    }
    prevInFlowRef.current = inFlow
  }, [restore.confirming, restore.restoreError])

  return (
    <div className="archive-recovery-screen__panel-body">
      {/* Success notice (transient — the gate usually unmounts the panel after a restore). */}
      {restore.restoreSucceeded ? (
        <p
          className="archive-recovery-screen__success"
          role="status"
          aria-live="polite"
        >
          {t('restoreSuccess')}
        </p>
      ) : null}

      {/* Loading state */}
      {loading ? (
        <p aria-label={t('loadingSnapshotsAria')} aria-busy="true">
          {t('loadingSnapshots')}
        </p>
      ) : null}

      {/* List load error + retry */}
      {!loading && loadError ? (
        <div role="alert" aria-label={t('loadErrorAria')}>
          <p>{t('loadError')}</p>
          <p className="archive-recovery-screen__detail-mono">{loadError}</p>
          <button
            type="button"
            className="btn-secondary"
            onClick={() => void reload()}
            aria-label={t('loadRetryAria')}
          >
            {t('loadRetry')}
          </button>
        </div>
      ) : null}

      {/* Restore error — honest + actionable, keeps the user in the flow. */}
      {restore.restoreError ? (
        <div role="alert" className="archive-recovery-screen__error">
          <p>{t('restoreError')}</p>
          <p className="archive-recovery-screen__detail-mono">
            {t('restoreErrorDetail').replace('{detail}', restore.restoreError)}
          </p>
          <button
            ref={retryButtonRef}
            type="button"
            className="btn-secondary"
            onClick={restore.resetError}
            aria-label={t('retryAria')}
          >
            {t('retry')}
          </button>
        </div>
      ) : null}

      {/* Confirm step (with key entry for an encrypted snapshot). */}
      {restore.confirming && !restore.restoreError ? (
        <div className="archive-recovery-screen__confirm">
          <p className="archive-recovery-screen__title">{t('confirmTitle')}</p>
          <p
            id="recovery-panel-confirm-body"
            className="archive-recovery-screen__body"
          >
            {restore.confirming.createdAt
              ? t('confirmBody').replace(
                  '{date}',
                  new Date(restore.confirming.createdAt).toLocaleString(),
                )
              : t('confirmBodyDateUnknown')}
          </p>
          {restore.confirming.encrypted ? (
            <ArchiveKeyField
              id="recovery-panel-key"
              inputRef={confirmKeyRef}
              value={restore.archiveKey}
              onChange={restore.setArchiveKey}
              t={t}
              disabled={restore.restoring}
            />
          ) : null}
          {restore.restoring ? (
            <p
              ref={restoringStatusRef}
              tabIndex={-1}
              role="status"
              aria-live="polite"
            >
              {t('restoring')}
            </p>
          ) : null}
          <div className="archive-recovery-screen__actions">
            <button
              type="button"
              className="btn-secondary"
              onClick={restore.cancelRestore}
              aria-label={t('cancelRestoreAria')}
              disabled={restore.restoring}
            >
              {t('cancelRestore')}
            </button>
            <button
              ref={confirmButtonRef}
              type="button"
              className="btn-primary"
              onClick={() => void restore.confirmRestore(restore.confirming!)}
              aria-label={t('restoreNowAria')}
              aria-describedby="recovery-panel-confirm-body"
              disabled={restore.restoring}
              aria-busy={restore.restoring}
            >
              {restore.restoring ? t('restoring') : t('restoreNow')}
            </button>
          </div>
        </div>
      ) : null}

      {/* Snapshot list / empty state (hidden while confirming or in a restore error). */}
      {!loading &&
      !loadError &&
      !restore.confirming &&
      !restore.restoreError ? (
        snapshots.length === 0 ? (
          <div className="archive-recovery-screen__empty">
            <p>{t('emptyTitle')}</p>
            <p>{t('emptyBody')}</p>
          </div>
        ) : (
          <div className="archive-recovery-screen__list">
            {snapshots.map((snap, index) => (
              <SnapshotCard
                key={snap.id}
                snap={snap}
                onRestore={() => restore.startRestore(snap)}
                busy={restore.restoring}
                t={t}
                className="archive-recovery-screen__snapshot-row"
                primaryRef={index === 0 ? firstCardButtonRef : undefined}
              />
            ))}
          </div>
        )
      ) : null}

      {/* PERSISTENT forward path — always reachable so the panel is never a dead end. */}
      <div className="archive-recovery-screen__actions">
        <button
          type="button"
          className="btn-ghost"
          onClick={() => void backend.revealLogs()}
          aria-label={t('revealLogsAria')}
        >
          {t('revealLogs')}
        </button>
      </div>
    </div>
  )
}
