/**
 * @file snapshot-restore-section.tsx
 * @description Settings → Restore from Snapshot section.
 * @module pages/settings
 *
 * ## Responsibilities
 * - Fetch the list of available recovery snapshots on mount and on retry.
 * - Render loading / error / empty / list states using the shared SnapshotCard.
 * - Wire the confirm inline panel using `useSnapshotRestore`.
 * - Reload the snapshot list after a successful restore (the panel stays visible
 *   since the archive recovered cleanly; the user can continue using Settings).
 *
 * ## Not responsible for
 * - Full-screen blocking recovery (that is `ArchiveRecoveryScreen`).
 * - Deciding whether recovery is required on launch.
 * - Owning any other settings section.
 *
 * ## Dependencies
 * - `useShellData()` for `runFullArchiveRestore`.
 * - `backend` client for `listRecoverySnapshots`.
 * - `useI18n('recovery')` for all copy.
 * - `useSnapshotRestore`, `SnapshotCard` from `../../components/snapshot-restore`.
 * - `PaperCard`, `PaperCardBody`, `PaperCardHeader` from the design system.
 *
 * ## Performance notes
 * - Single fetch on mount; no background polling.
 * - List is expected to be < 20 items; no virtualization needed.
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { PaperCard, PaperCardBody, PaperCardHeader } from '@/components/cards'
import { backend } from '@/lib/backend-client'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { useShellData } from '../../app/shell-data-context'
import type { RecoverySnapshot } from '../../lib/types'
import { SnapshotCard } from '../../components/snapshot-restore'
import { useSnapshotRestore } from '../../components/snapshot-restore/use-snapshot-restore'
import type { SettingsSectionNavItem } from './section-nav-items'

export interface SnapshotRestoreSectionProps {
  navItem: SettingsSectionNavItem
}

/**
 * Renders the Settings → Restore from Snapshot section.
 *
 * Loads the snapshot list on mount and provides restore interactions inline,
 * reusing the shared `useSnapshotRestore` hook for consistent behavior with
 * `ArchiveRecoveryScreen`.
 */
export function SnapshotRestoreSection({
  navItem,
}: SnapshotRestoreSectionProps) {
  const { runFullArchiveRestore } = useShellData()
  const { t } = useI18n('recovery')

  const [snapshots, setSnapshots] = useState<RecoverySnapshot[]>([])
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState<string | null>(null)

  // Track mounted state to avoid setting state after unmount
  const mountedRef = useRef(true)
  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
    }
  }, [])

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
    onSuccess: () => {
      void reload()
    },
  })

  return (
    <PaperCard id={navItem.id}>
      <PaperCardHeader title={t('sectionTitle')} />
      <PaperCardBody>
        <p>{t('sectionDescription')}</p>

        {/* Success notice — this section stays mounted after a clean restore. */}
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

        {/* Load error */}
        {!loading && loadError ? (
          <div role="alert" aria-label={t('loadErrorAria')}>
            <p>{t('loadError')}</p>
            <p>{loadError}</p>
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

        {/* Restore error */}
        {restore.restoreError ? (
          <div role="alert" className="archive-recovery-screen__error">
            <p>{t('restoreError')}</p>
            <p>
              {t('restoreErrorDetail').replace(
                '{detail}',
                restore.restoreError,
              )}
            </p>
            <button
              type="button"
              className="btn-secondary"
              onClick={restore.resetError}
              aria-label={t('retryAria')}
            >
              {t('retry')}
            </button>
          </div>
        ) : null}

        {/* Snapshot list */}
        {!loading && !loadError ? (
          snapshots.length === 0 ? (
            <div className="archive-recovery-screen__empty">
              <p>{t('emptyTitle')}</p>
              <p>{t('emptyBody')}</p>
            </div>
          ) : (
            <div className="archive-recovery-screen__list">
              {snapshots.map((snap) => (
                <SnapshotCard
                  key={snap.id}
                  snap={snap}
                  onRestore={() => restore.startRestore(snap)}
                  busy={restore.restoring}
                  t={t}
                  className="archive-recovery-screen__snapshot-row"
                />
              ))}
            </div>
          )
        ) : null}

        {/* Confirm panel */}
        {restore.confirming && !restore.restoreError ? (
          <div className="archive-recovery-screen__confirm">
            <p>{t('confirmTitle')}</p>
            <p id="settings-confirm-body">
              {restore.confirming.createdAt
                ? t('confirmBody').replace(
                    '{date}',
                    new Date(restore.confirming.createdAt).toLocaleString(),
                  )
                : t('confirmBodyDateUnknown')}
            </p>
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
                type="button"
                className="btn-primary"
                onClick={() => void restore.confirmRestore(restore.confirming!)}
                aria-label={t('restoreNowAria')}
                aria-describedby="settings-confirm-body"
                disabled={restore.restoring}
                aria-busy={restore.restoring}
              >
                {restore.restoring ? t('restoring') : t('restoreNow')}
              </button>
            </div>
          </div>
        ) : null}
      </PaperCardBody>
    </PaperCard>
  )
}
