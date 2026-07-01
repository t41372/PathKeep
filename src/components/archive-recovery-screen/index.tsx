/**
 * @file index.tsx
 * @description Full-screen blocking recovery gate shown when the archive cannot open on
 * launch and verified snapshots are available to restore from.
 * @module components/archive-recovery-screen
 *
 * ## Responsibilities
 * - Render a focused, paper-aesthetic full-screen overlay when a launch-time archive
 *   recovery is required.
 * - List available recovery snapshots (newest first, with expand/collapse for the full list).
 * - Walk the user through a confirm step before executing the destructive restore.
 * - Full a11y: role="dialog", aria-modal, focus trap. Escape cancels confirm; otherwise
 *   Escape does NOT dismiss (app is unusable until recovery completes or fails).
 *
 * ## Not responsible for
 * - Loading the recovery snapshot list (provided via `ArchiveRecoveryReport` from shell-data).
 * - Deciding when to mount/unmount — that logic lives in `AppBody` in `src/app/index.tsx`.
 * - Performing navigation or routing.
 *
 * ## Dependencies
 * - `useShellData()` for `runFullArchiveRestore`.
 * - `backend` client for `revealLogs`.
 * - `useI18n('recovery')` for all copy.
 * - Shared `useSnapshotRestore`, `SnapshotCard` from `../snapshot-restore`.
 *
 * ## Performance notes
 * - Focus trap is a lightweight keyboard listener; this gate is only mounted during
 *   the rare recovery scenario, so it adds no overhead to the normal launch path.
 */

import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
} from 'react'
import { useShellData } from '../../app/shell-data-context'
import { backend } from '../../lib/backend-client'
import { useI18n } from '../../lib/i18n'
import type { ArchiveRecoveryReport, RecoverySnapshot } from '../../lib/types'
import { SnapshotCard } from '../snapshot-restore'
import { useSnapshotRestore } from '../snapshot-restore/use-snapshot-restore'

interface ArchiveRecoveryScreenProps {
  report: ArchiveRecoveryReport
}

/**
 * Renders the blocking archive-recovery overlay. Mounts when the shell detects a
 * launch-time archive failure with available snapshots; unmounts when `runFullArchiveRestore`
 * succeeds and `refreshAppData` clears `recovery` from the shell context.
 */
export function ArchiveRecoveryScreen({ report }: ArchiveRecoveryScreenProps) {
  const { runFullArchiveRestore } = useShellData()
  const { t } = useI18n('recovery')

  const [expanded, setExpanded] = useState(false)
  const primaryBtnRef = useRef<HTMLButtonElement | null>(null)
  const confirmBtnRef = useRef<HTMLButtonElement | null>(null)
  const restoringStatusRef = useRef<HTMLParagraphElement | null>(null)

  const restore = useSnapshotRestore({ runFullArchiveRestore })

  // Compute snapshot subsets
  const snapshots = report.recoverySnapshots
  const verified = snapshots.filter((s) => s.verifiedOpenable)
  const newest: RecoverySnapshot | null = verified[0] ?? snapshots[0] ?? null
  const remaining = snapshots.filter((s) => s !== newest)

  // Autofocus the primary action on mount; restore prior focus on unmount. In the
  // guided state this focuses the headline "Restore this" button (via primaryRef); in
  // the empty state it focuses the "Reveal logs" button.
  useEffect(() => {
    const previouslyFocused = document.activeElement as HTMLElement | null
    primaryBtnRef.current?.focus()
    return () => {
      // Stryker disable next-line OptionalChaining: defensive — element may be gone.
      previouslyFocused?.focus?.()
    }
  }, [])

  // When the confirm step opens, move focus to its primary "Restore now" button so
  // keyboard users land on the destructive confirmation rather than nowhere.
  useEffect(() => {
    if (restore.confirming) {
      // Stryker disable next-line OptionalChaining: defensive — element may be gone.
      confirmBtnRef.current?.focus?.()
    }
  }, [restore.confirming])

  // When the restoring state begins, move focus to the status region so screen-reader
  // users hear the announcement and keyboard focus does not drop to the document body.
  useEffect(() => {
    if (restore.restoring) {
      // Stryker disable next-line OptionalChaining: defensive — element may be gone.
      restoringStatusRef.current?.focus?.()
    }
  }, [restore.restoring])

  // Destructure for stable useCallback deps (React Compiler requires direct values,
  // not property access chains, in the dependency array).
  const { confirming: restoreConfirming, cancelRestore } = restore

  // Focus trap: Tab / Shift+Tab cycles inside the dialog. Escape cancels confirm
  // step (if active) but does NOT dismiss the screen.
  const handleTrapKeyDown = useCallback(
    (event: ReactKeyboardEvent<HTMLDivElement>) => {
      if (event.key === 'Escape') {
        if (restoreConfirming) {
          cancelRestore()
        }
        return
      }
      if (event.key !== 'Tab') return
      const focusable = Array.from(
        event.currentTarget.querySelectorAll<HTMLElement>(
          'input,button,select,textarea,a[href],[tabindex]:not([tabindex="-1"])',
        ),
      ).filter((el) => !el.hasAttribute('disabled') && el.tabIndex >= 0)
      const first = focusable[0]
      const last = focusable[focusable.length - 1]
      if (!first || !last) return
      if (event.shiftKey) {
        if (document.activeElement === first) {
          event.preventDefault()
          last.focus()
        }
      } else if (document.activeElement === last) {
        event.preventDefault()
        first.focus()
      }
    },
    [restoreConfirming, cancelRestore],
  )

  // Body copy: distinct strings for each scenario to avoid English-only pluralisation
  // leaking into the Chinese locales (no `{plural}` interpolation).
  const bodyText =
    snapshots.length === 0
      ? t('bodyNoSnapshots')
      : verified.length === 0
        ? t('bodyUnverifiedOnly')
        : verified.length === 1
          ? t('bodyOne')
          : t('bodyMany').replace('{count}', String(verified.length))

  // ─── Empty state ──────────────────────────────────────────────────────────
  if (snapshots.length === 0) {
    return (
      <div
        className="archive-recovery-screen"
        role="dialog"
        aria-modal="true"
        aria-labelledby="recovery-screen-title"
        aria-describedby="recovery-screen-desc"
        onKeyDown={handleTrapKeyDown}
        data-testid="archive-recovery-screen"
      >
        <div className="archive-recovery-screen__backdrop" aria-hidden="true" />
        <div className="archive-recovery-screen__panel">
          <div
            className="archive-recovery-screen__eyebrow mono-kicker"
            aria-hidden="true"
          >
            {t('eyebrow')}
          </div>
          <h1
            id="recovery-screen-title"
            className="archive-recovery-screen__title"
          >
            {t('title')}
          </h1>
          <p
            id="recovery-screen-desc"
            className="archive-recovery-screen__body"
          >
            {bodyText}
          </p>
          <div className="archive-recovery-screen__empty">
            <p>{t('emptyTitle')}</p>
            <p>{t('emptyBody')}</p>
            <p>{t('emptyReassurance')}</p>
            <p className="archive-recovery-screen__detail-mono">
              {report.detail}
            </p>
          </div>
          <div className="archive-recovery-screen__actions">
            <button
              ref={primaryBtnRef}
              type="button"
              className="btn-secondary"
              onClick={() => void backend.revealLogs()}
              aria-label={t('revealLogsAria')}
            >
              {t('revealLogs')}
            </button>
          </div>
        </div>
      </div>
    )
  }

  // ─── Restoring state ──────────────────────────────────────────────────────
  if (restore.restoring) {
    return (
      <div
        className="archive-recovery-screen"
        role="dialog"
        aria-modal="true"
        aria-labelledby="recovery-screen-title"
        aria-describedby="recovery-screen-desc"
        onKeyDown={handleTrapKeyDown}
        data-testid="archive-recovery-screen"
      >
        <div className="archive-recovery-screen__backdrop" aria-hidden="true" />
        <div className="archive-recovery-screen__panel">
          <div
            className="archive-recovery-screen__eyebrow mono-kicker"
            aria-hidden="true"
          >
            {t('eyebrow')}
          </div>
          <h1
            id="recovery-screen-title"
            className="archive-recovery-screen__title"
          >
            {t('title')}
          </h1>
          <p
            ref={restoringStatusRef}
            tabIndex={-1}
            id="recovery-screen-desc"
            className="archive-recovery-screen__body"
            aria-live="polite"
          >
            {t('restoring')}
          </p>
        </div>
      </div>
    )
  }

  // ─── Main panel ───────────────────────────────────────────────────────────
  return (
    <div
      className="archive-recovery-screen"
      role="dialog"
      aria-modal="true"
      aria-labelledby="recovery-screen-title"
      aria-describedby="recovery-screen-desc"
      onKeyDown={handleTrapKeyDown}
      data-testid="archive-recovery-screen"
    >
      <div className="archive-recovery-screen__backdrop" aria-hidden="true" />
      <div className="archive-recovery-screen__panel">
        <div
          className="archive-recovery-screen__eyebrow mono-kicker"
          aria-hidden="true"
        >
          {t('eyebrow')}
        </div>
        <h1
          id="recovery-screen-title"
          className="archive-recovery-screen__title"
        >
          {t('title')}
        </h1>
        <p id="recovery-screen-desc" className="archive-recovery-screen__body">
          {bodyText}
        </p>

        {/* Restore error state */}
        {restore.restoreError ? (
          <div role="alert" className="archive-recovery-screen__error">
            <p>{t('restoreError')}</p>
            <p className="archive-recovery-screen__detail-mono">
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

        {/* Confirm step */}
        {restore.confirming && !restore.restoreError ? (
          <div className="archive-recovery-screen__confirm">
            <p className="archive-recovery-screen__title">
              {t('confirmTitle')}
            </p>
            <p
              id="recovery-confirm-body"
              className="archive-recovery-screen__body"
            >
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
              >
                {t('cancelRestore')}
              </button>
              <button
                ref={confirmBtnRef}
                type="button"
                className="btn-primary"
                onClick={() => void restore.confirmRestore(restore.confirming!)}
                aria-label={t('restoreNowAria')}
                aria-describedby="recovery-confirm-body"
                disabled={restore.restoring}
              >
                {t('restoreNow')}
              </button>
            </div>
          </div>
        ) : null}

        {/* Headline card (newest snapshot) */}
        {!restore.confirming && !restore.restoreError && newest ? (
          <SnapshotCard
            snap={newest}
            onRestore={() => restore.startRestore(newest)}
            busy={false}
            t={t}
            className="archive-recovery-screen__headline-card"
            primaryRef={primaryBtnRef}
          />
        ) : null}

        {/* Expand/collapse remaining snapshots */}
        {!restore.confirming &&
        !restore.restoreError &&
        remaining.length > 0 ? (
          <>
            <button
              type="button"
              className="btn-secondary"
              aria-expanded={expanded}
              aria-controls="recovery-snapshot-list"
              onClick={() => setExpanded((v) => !v)}
              aria-label={expanded ? t('hideAllAria') : t('seeAllAria')}
            >
              {expanded ? t('hideAll') : t('seeAll')}
            </button>
            {expanded ? (
              <div
                id="recovery-snapshot-list"
                className="archive-recovery-screen__list"
              >
                {remaining.map((snap) => (
                  <SnapshotCard
                    key={snap.id}
                    snap={snap}
                    onRestore={() => restore.startRestore(snap)}
                    busy={false}
                    t={t}
                    className="archive-recovery-screen__snapshot-row"
                  />
                ))}
              </div>
            ) : null}
          </>
        ) : null}
      </div>
    </div>
  )
}
