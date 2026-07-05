/**
 * @file index.tsx
 * @description Full-screen, calm first-run gate shown when a healthy archive is version-behind
 * and the one-time v0.2.0 → v0.3.0 upgrade migration is pending. Replaces the opaque busy
 * overlay with an honest, per-phase progress experience driven by real backend events.
 * @module components/archive-upgrade-screen
 *
 * ## Responsibilities
 * - Render a focused, paper-aesthetic full-screen overlay while the one-time upgrade runs.
 * - Subscribe to `pathkeep://archive-upgrade` progress and drive `initialize_archive` exactly
 *   once per attempt (subscribe FIRST, then invoke — mirrors `runImport`).
 * - Render each STREAMED phase as a determinate bar (honest counts) or the canonical
 *   indeterminate sweep (opaque index/finalize work); render the non-streamed Intelligence
 *   phase as an informational line, never a bar stuck at zero.
 * - Transition to the shell ONLY when `initialize_archive` RESOLVES (its resolution implies the
 *   terminal `done` event was emitted). Surface a retryable error on rejection — never a dead end.
 * - a11y: role="dialog", aria-modal, on-mount (and on-Retry) focus into the status region, and a
 *   throttled aria-live region that announces phase changes + 25% milestones without SR spam.
 *
 * ## Not responsible for
 * - Deciding WHEN to mount/unmount — that gate lives in `AppBody` (`src/app/index.tsx`), seeded
 *   by the cheap `assess_archive_upgrade` pre-check in shell-data.
 * - Running the migration itself (that is backend/off-thread inside `initialize_archive`).
 * - Routing or rendering any route content.
 *
 * ## Dependencies
 * - `useShellData()` for `finishArchiveUpgrade` (clears the gate + re-bootstraps the shell).
 * - `backend.initializeArchive` to drive the migration; `subscribeToArchiveUpgradeProgress` for ticks.
 * - `useI18n('archiveUpgrade')` for all copy; `describeError` to render an honest failure detail.
 *
 * ## Performance notes
 * - The heavy work runs off the main thread in the backend; this screen only listens and paints.
 * - The aria-live effect keys on `[currentPhase, announceBucket]` so it re-announces at most once
 *   per phase/milestone — never on every progress tick.
 */

import { useEffect, useRef, useState } from 'react'
import { useShellData } from '../../app/shell-data-context'
import { backend } from '../../lib/backend-client'
import { describeError } from '../../lib/errors'
import { useI18n } from '../../lib/i18n'
import { subscribeToArchiveUpgradeProgress } from '../../lib/ipc/archive-upgrade-progress'
import type {
  AppConfig,
  ArchiveUpgradeAssessment,
  ArchiveUpgradePhase,
  ArchiveUpgradeProgress,
} from '../../lib/types'

interface ArchiveUpgradeScreenProps {
  assessment: ArchiveUpgradeAssessment
  config: AppConfig
}

/** Clamps a processed/total ratio to an integer percent in the inclusive 0–100 range. */
function clampPercent(processed: number, total: number) {
  const raw = Math.round((processed / total) * 100)
  return Math.min(100, Math.max(0, raw))
}

/**
 * Renders the blocking one-time upgrade overlay. Mounts when shell-data's cheap
 * pre-check reports a pending upgrade; unmounts when `initialize_archive`
 * resolves and `finishArchiveUpgrade` re-bootstraps the shell (upgrade no longer
 * pending).
 */
export function ArchiveUpgradeScreen({
  assessment,
  config,
}: ArchiveUpgradeScreenProps) {
  const { finishArchiveUpgrade } = useShellData()
  const { t } = useI18n('archiveUpgrade')

  const [progress, setProgress] = useState<ArchiveUpgradeProgress | null>(null)
  const [status, setStatus] = useState<'working' | 'error'>('working')
  const [error, setError] = useState('')
  const [retryCount, setRetryCount] = useState(0)
  const [announcement, setAnnouncement] = useState('')

  const statusRegionRef = useRef<HTMLDivElement | null>(null)
  // Fires the backend init exactly once per attempt (retryCount), so an
  // accidental double-invoke (e.g. StrictMode remount) cannot start the
  // migration twice.
  const startedAttemptRef = useRef(-1)
  const unsubscribeRef = useRef<(() => void) | null>(null)
  // Guards the async subscribe→drive against a teardown that fires before the
  // subscription resolves (StrictMode setup→cleanup→setup, or a real rapid
  // unmount). A ref — not a captured local — so a re-mount can *un-cancel* the
  // still-pending run: otherwise the `startedAttemptRef` guard would block the
  // second setup and strand the drive entirely.
  const cancelledRef = useRef(false)
  // Read the stable-in-practice props through refs so the init effect can key
  // only on `retryCount` — a config identity change must never tear down the
  // live progress subscription mid-upgrade.
  const configRef = useRef(config)
  const finishRef = useRef(finishArchiveUpgrade)
  const tRef = useRef(t)

  // Keep the latest props/callbacks reachable from the effects below without
  // making them effect dependencies (refs must not be written during render).
  useEffect(() => {
    configRef.current = config
    finishRef.current = finishArchiveUpgrade
    tRef.current = t
  })

  // Move focus into the status region on mount so screen-reader users land
  // inside the dialog and keyboard focus does not drop to document.body.
  // Restore prior focus on unmount (best-effort a11y).
  useEffect(() => {
    const previouslyFocused = document.activeElement as HTMLElement | null
    statusRegionRef.current?.focus()
    return () => {
      // Stryker disable next-line OptionalChaining: defensive — element may be gone.
      previouslyFocused?.focus?.()
    }
  }, [])

  // Subscribe FIRST, THEN drive the migration — the exact ordering used by
  // `runImport` so no early tick is missed. On resolve the terminal `done`
  // event has already been emitted, so it is safe to hand off to the shell.
  useEffect(() => {
    // Any (re)mount un-cancels an in-flight run so a StrictMode
    // setup→cleanup→setup cycle still drives the migration exactly once — the
    // guard below keeps the second setup from starting a *second* run.
    cancelledRef.current = false

    if (startedAttemptRef.current !== retryCount) {
      startedAttemptRef.current = retryCount

      const run = async () => {
        const unsubscribe = await subscribeToArchiveUpgradeProgress((event) => {
          setProgress(event)
        })
        if (cancelledRef.current) {
          // Torn down before the subscription resolved and NOT re-mounted:
          // clean up the just-registered listener and abort before starting the
          // migration on a dead component.
          unsubscribe()
          return
        }
        unsubscribeRef.current = unsubscribe
        try {
          await backend.initializeArchive(configRef.current)
        } catch (nextError) {
          setStatus('error')
          setError(describeError(nextError, 'initialize_archive'))
          return
        }
        // Handoff: the shell (refreshAppData) owns surfacing any re-bootstrap
        // error; the gate is already unmounting, so swallow to avoid an
        // unhandled rejection or a setState on an unmounted component.
        await finishRef.current().catch(() => {})
      }

      void run()
    }

    return () => {
      cancelledRef.current = true
      unsubscribeRef.current?.()
      unsubscribeRef.current = null
    }
  }, [retryCount])

  // ─── Derived progress view ──────────────────────────────────────────────────
  const streamedPending = assessment.phases.find(
    (phase) => phase.pending && phase.streamed,
  )
  // Defensive: the gate only mounts on a real pending upgrade, which always
  // carries at least one phase. This fallback keeps a malformed (empty-phases)
  // assessment from white-screening on `phases[0]`.
  /* v8 ignore next -- defensive: phases is never empty on a real upgrade. */
  const firstPhase = assessment.phases[0]?.phase ?? 'schemaMigration'
  const currentPhase: ArchiveUpgradePhase =
    progress?.phase ?? streamedPending?.phase ?? firstPhase

  // Steps are derived over STREAMED pending phases only: the non-streamed
  // Intelligence phase is surfaced as an info line (never a `currentPhase`), so
  // counting it would inflate the total and strand the user on a step that
  // never arrives. The terminal `finalizing` event is not in `assessment.phases`
  // either, so `findIndex` returns -1 there — clamp that to the LAST step
  // instead of regressing to "Step 1" during "Finishing up".
  const pendingPhases = assessment.phases.filter(
    (phase) => phase.pending && phase.streamed,
  )
  const stepTotal = pendingPhases.length
  const stepPosition = pendingPhases.findIndex(
    (phase) => phase.phase === currentPhase,
  )
  const stepCurrent = stepPosition === -1 ? stepTotal : stepPosition + 1

  const isDeterminate = progress !== null && progress.total > 0
  const pct = isDeterminate
    ? clampPercent(progress.processed, progress.total)
    : 0

  // The terminal/finalizing moment earns an honest "Almost done…" label instead
  // of the generic "Working…": the backend's single terminal tick carries
  // `phase: 'finalizing'` + `done: true`, and the finalizing phase itself is
  // opaque (0/0) work.
  const isFinishing = progress?.done === true || currentPhase === 'finalizing'

  const showIntelligenceInfo = assessment.phases.some(
    (phase) =>
      phase.phase === 'intelligence' && phase.pending && !phase.streamed,
  )

  // The count/status line under the bar: preparing (no event yet), the finishing
  // label (finalizing/terminal), a human count (determinate), or a generic
  // working label (opaque 0/0 sweep).
  const statusLine =
    progress === null
      ? t('preparing')
      : isFinishing
        ? t('finishing')
        : isDeterminate
          ? t('countProgress', {
              processed: progress.processed.toLocaleString(),
              total: progress.total.toLocaleString(),
            })
          : t('working')

  // Throttled announcement bucket: 25% milestones for a determinate phase, a
  // single 'indeterminate' bucket otherwise. The announce effect below keys on
  // [currentPhase, announceBucket], so it re-announces at most once per phase or
  // milestone crossing — never on every tick.
  const announceBucket = isDeterminate
    ? String(Math.floor(pct / 25) * 25)
    : 'indeterminate'

  useEffect(() => {
    const phaseName = tRef.current(`phase.${currentPhase}`)
    const detail = isFinishing
      ? tRef.current('finishing')
      : announceBucket === 'indeterminate'
        ? tRef.current('working')
        : `${announceBucket}%`
    setAnnouncement(`${phaseName} — ${detail}`)
  }, [currentPhase, announceBucket, isFinishing])

  return (
    <div
      className="archive-upgrade-screen"
      role="dialog"
      aria-modal="true"
      aria-labelledby="archive-upgrade-title"
      data-testid="archive-upgrade-screen"
    >
      <div className="archive-upgrade-screen__backdrop" aria-hidden="true" />
      <div className="archive-upgrade-screen__panel">
        <div
          className="archive-upgrade-screen__eyebrow mono-kicker"
          aria-hidden="true"
        >
          {t('eyebrow')}
        </div>
        <h1
          id="archive-upgrade-title"
          className="archive-upgrade-screen__title"
        >
          {t('title')}
        </h1>

        <div
          ref={statusRegionRef}
          tabIndex={-1}
          className="archive-upgrade-screen__status"
          aria-label={t('statusAria')}
        >
          {status === 'working' ? (
            <>
              <p className="archive-upgrade-screen__body">{t('body')}</p>

              <ul className="archive-upgrade-screen__reassurances">
                <li>{t('oneTimeNote')}</li>
                <li>{t('dataSafeNote')}</li>
                <li>{t('resumableNote')}</li>
              </ul>

              <p className="archive-upgrade-screen__step mono-kicker">
                {t('stepIndicator', { current: stepCurrent, total: stepTotal })}
              </p>

              <p className="archive-upgrade-screen__phase">
                {t(`phase.${currentPhase}`)}
              </p>

              <div
                className="archive-upgrade-screen__bar"
                role="progressbar"
                aria-label={t('progressAria')}
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={isDeterminate ? pct : undefined}
              >
                <div
                  data-testid="archive-upgrade-fill"
                  className={
                    isDeterminate
                      ? 'archive-upgrade-screen__fill'
                      : 'archive-upgrade-screen__fill pk-indeterminate-bar'
                  }
                  style={isDeterminate ? { width: `${pct}%` } : undefined}
                />
              </div>

              <p className="archive-upgrade-screen__count">{statusLine}</p>

              {showIntelligenceInfo ? (
                <p className="archive-upgrade-screen__info">
                  {t('intelligenceInfo')}
                </p>
              ) : null}
            </>
          ) : (
            <div role="alert" className="archive-upgrade-screen__error">
              <p className="archive-upgrade-screen__title">{t('errorTitle')}</p>
              <p className="archive-upgrade-screen__detail-mono">
                {t('errorDetail', { detail: error })}
              </p>
              <button
                type="button"
                className="btn-secondary"
                onClick={() => {
                  setError('')
                  setProgress(null)
                  setStatus('working')
                  setRetryCount((value) => value + 1)
                  // The error content (incl. this button) is about to unmount;
                  // move focus back into the persistent status region so it does
                  // not drop to document.body.
                  statusRegionRef.current?.focus()
                }}
                aria-label={t('retryAria')}
              >
                {t('retry')}
              </button>
            </div>
          )}
        </div>

        <p
          data-testid="archive-upgrade-live"
          className="archive-upgrade-screen__live"
          aria-live="polite"
        >
          {announcement}
        </p>
      </div>
    </div>
  )
}
