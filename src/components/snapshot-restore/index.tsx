/**
 * @file index.tsx
 * @description Snapshot card and list components for the restore flow.
 * @module components/snapshot-restore
 *
 * ## Responsibilities
 * - Render `SnapshotCard` — a single snapshot card with date, size, badge, sourceOp,
 *   and a Restore action button.
 * - Render `SnapshotRestoreList` — list of snapshot cards with loading / error / empty states.
 *
 * ## Not responsible for
 * - Owning the restore state machine (see `use-snapshot-restore.ts`).
 * - Loading snapshot data from the backend.
 *
 * ## Dependencies
 * - `formatBytes` for file size display.
 * - `useI18n('recovery')` for all copy.
 * - `sourceOpLabel` from `use-snapshot-restore.ts`.
 *
 * ## Performance notes
 * - Pure presentational components with no internal async work.
 */

import type { Ref } from 'react'
import { useI18n } from '../../lib/i18n'
import { formatBytes } from '../../lib/format'
import type { RecoverySnapshot } from '../../lib/types'
import { sourceOpLabel } from './use-snapshot-restore'

// ─── SnapshotCard ─────────────────────────────────────────────────────────────

interface SnapshotCardProps {
  snap: RecoverySnapshot
  onRestore: () => void
  busy: boolean
  t: (k: string) => string
  className?: string
  /**
   * Optional ref forwarded to the primary "Restore this" button so a parent
   * (e.g. the recovery screen) can move keyboard focus to it on mount.
   */
  primaryRef?: Ref<HTMLButtonElement>
}

/**
 * Renders a single snapshot card with date, size, verification badge,
 * source operation label, and a Restore action button.
 */
export function SnapshotCard({
  snap,
  onRestore,
  busy,
  t,
  className = 'archive-recovery-screen__headline-card',
  primaryRef,
}: SnapshotCardProps) {
  const dateLabel = snap.createdAt
    ? t('snapshotDate').replace(
        '{date}',
        new Date(snap.createdAt).toLocaleString(),
      )
    : t('snapshotDateUnknown')

  const badgeClass = snap.verifiedOpenable
    ? 'archive-recovery-screen__badge archive-recovery-screen__badge--verified'
    : 'archive-recovery-screen__badge archive-recovery-screen__badge--unverified'

  return (
    <div className={className}>
      <div className="archive-recovery-screen__snapshot-meta">
        <span className={badgeClass}>
          {snap.verifiedOpenable ? t('verifiedBadge') : t('notVerifiedBadge')}
        </span>
        <span className="archive-recovery-screen__detail-mono">
          {dateLabel}
        </span>
        <span className="archive-recovery-screen__detail-mono">
          {t('snapshotSize').replace('{size}', formatBytes(snap.sizeBytes))}
        </span>
        <span className="archive-recovery-screen__detail-mono">
          {sourceOpLabel(snap.sourceOp, t)}
        </span>
      </div>
      <div className="archive-recovery-screen__actions">
        <button
          ref={primaryRef}
          type="button"
          className="btn-primary"
          onClick={onRestore}
          disabled={busy}
          aria-label={t('restoreThisAria')}
        >
          {t('restoreThis')}
        </button>
      </div>
    </div>
  )
}

// ─── SnapshotRestoreList ──────────────────────────────────────────────────────

export interface SnapshotRestoreListProps {
  snapshots: RecoverySnapshot[]
  loading: boolean
  error: string | null
  onRestore: (snap: RecoverySnapshot) => void
  busy: boolean
}

/**
 * Renders the full snapshot list with loading / error / empty states.
 * Consumers own data fetching; this component is purely presentational.
 */
export function SnapshotRestoreList({
  snapshots,
  loading,
  error,
  onRestore,
  busy,
}: SnapshotRestoreListProps) {
  const { t } = useI18n('recovery')

  if (loading) {
    return (
      <p aria-label={t('loadingSnapshotsAria')} aria-busy="true">
        {t('loadingSnapshots')}
      </p>
    )
  }

  if (error) {
    return (
      <div role="alert" aria-label={t('loadErrorAria')}>
        <p>{t('loadError')}</p>
        <p className="archive-recovery-screen__detail-mono">{error}</p>
      </div>
    )
  }

  if (snapshots.length === 0) {
    return (
      <div className="archive-recovery-screen__empty">
        <p>{t('emptyTitle')}</p>
        <p>{t('emptyBody')}</p>
      </div>
    )
  }

  return (
    <div className="archive-recovery-screen__list">
      {snapshots.map((snap) => (
        <div key={snap.id} className="archive-recovery-screen__snapshot-row">
          <SnapshotCard
            snap={snap}
            onRestore={() => onRestore(snap)}
            busy={busy}
            t={t}
            className="archive-recovery-screen__snapshot-row"
          />
        </div>
      ))}
    </div>
  )
}
