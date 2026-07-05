/**
 * @file index.tsx
 * @description Snapshot card and list components for the restore flow.
 * @module components/snapshot-restore
 *
 * ## Responsibilities
 * - Render `SnapshotCard` — a single snapshot card with date, size, badge, sourceOp,
 *   and a Restore action button. Encrypted snapshots get an honest "needs your key"
 *   badge instead of the green "Verified" one.
 * - Render `SnapshotRestoreList` — list of snapshot cards with loading / error / empty states.
 * - Render `ArchiveKeyField` — the shared password input used to collect the archive
 *   key before verifying/restoring an encrypted snapshot.
 *
 * ## Not responsible for
 * - Owning the restore state machine (see `use-snapshot-restore.ts`).
 * - Loading snapshot data from the backend.
 * - Deciding when a key is required (callers gate on `snap.encrypted`).
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

export { SnapshotRecoveryPanel } from './recovery-panel'

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

  // Encrypted snapshots can NEVER be verified keyless, so an honest "needs your key"
  // badge replaces the green "Verified" one regardless of the size-only `verifiedOpenable`
  // heuristic — the authoritative keyed check only runs at restore time.
  const badgeClass = snap.encrypted
    ? 'archive-recovery-screen__badge archive-recovery-screen__badge--needs-key'
    : snap.verifiedOpenable
      ? 'archive-recovery-screen__badge archive-recovery-screen__badge--verified'
      : 'archive-recovery-screen__badge archive-recovery-screen__badge--unverified'

  const badgeLabel = snap.encrypted
    ? t('encryptedNeedsKeyBadge')
    : snap.verifiedOpenable
      ? t('verifiedBadge')
      : t('notVerifiedBadge')

  return (
    <div className={className}>
      <div className="archive-recovery-screen__snapshot-meta">
        <span className={badgeClass}>{badgeLabel}</span>
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

// ─── ArchiveKeyField ──────────────────────────────────────────────────────────

export interface ArchiveKeyFieldProps {
  value: string
  onChange: (value: string) => void
  t: (k: string) => string
  id: string
  disabled?: boolean
  /**
   * Optional ref forwarded to the underlying `<input>` so a parent can move
   * keyboard focus onto the key field when the encrypted confirm step opens —
   * the field is the thing the user must fill, so it (not the destructive
   * button) is the correct focus target for keyboard/AT users.
   */
  inputRef?: Ref<HTMLInputElement>
}

/**
 * Renders the shared archive-key password input shown in the confirm step for an
 * encrypted snapshot. Purely presentational — the caller owns the value and gates
 * on `snap.encrypted`. The value flows straight into the restore call and is never
 * logged.
 */
export function ArchiveKeyField({
  value,
  onChange,
  t,
  id,
  disabled,
  inputRef,
}: ArchiveKeyFieldProps) {
  return (
    <label className="field-stack archive-recovery-screen__key-field">
      <span className="mono-kicker">{t('keyFieldLabel')}</span>
      <input
        ref={inputRef}
        id={id}
        type="password"
        autoComplete="current-password"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={t('keyFieldPlaceholder')}
        aria-label={t('keyFieldLabel')}
        disabled={disabled}
      />
      <span className="archive-recovery-screen__key-hint">
        {t('keyFieldHint')}
      </span>
    </label>
  )
}
