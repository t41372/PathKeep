/**
 * Backup-failure alert — the unmissable notification shown when a backup
 * attempt fails.
 *
 * ## Responsibilities
 * - Mount at the SAME fixed bottom slot the `AmbientTaskBar` strip just
 *   vacated, so the failure signal materializes exactly where the user's eye is
 *   already resting. This converts the old misdirection (a big motion at the
 *   bottom hiding a static message at the top) into attention transfer.
 * - Read as an error pre-attentively: a solid `--error` rail + red heading +
 *   warning glyph — the only saturated-red element in the shell — so it pops via
 *   the von Restorff isolation effect even with motion disabled (reduced-motion
 *   safe; the entrance animation is an enhancement, not the only signal).
 * - Help the user recover: reassurance that the existing archive is intact, the
 *   failure cause, a Full Disk Access deep-link when relevant, retry, a
 *   copy-able diagnostic report for bug reports, and the raw error behind a
 *   progressive-disclosure `<details>` — NOT a Finder folder dump.
 *
 * ## Not responsible for
 * - Deciding WHEN a failure surfaces (the shell action / provider own that and
 *   feed us `message` / `rawError` / `errorKind`).
 * - Recording the failed run in the audit ledger (the backend already does).
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { TriangleAlertIcon } from 'lucide-react'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import type { ShellErrorKind } from '@/app/shell-data-context'
import type { AppBuildInfo } from '@/lib/types'
import { buildBackupDiagnosticReport } from './backup-failure-diagnostics'

const COPIED_RESET_MS = 2000

export interface BackupFailureToastProps {
  /** The user-facing failure message (already localized when applicable). */
  message: string
  /** The raw, untranslated backend error for the diagnostic report. */
  rawError: string | null
  /** Locale-independent classification used to gate remediation affordances. */
  errorKind: ShellErrorKind
  /** Build metadata stamped into the diagnostic report. */
  buildInfo: AppBuildInfo | null
  onRetry: () => void
  onDismiss: () => void
  onOpenFdaSettings: () => void
  onRevealLogs: () => void
}

export function BackupFailureToast({
  message,
  rawError,
  errorKind,
  buildInfo,
  onRetry,
  onDismiss,
  onOpenFdaSettings,
  onRevealLogs,
}: BackupFailureToastProps) {
  const { t } = useI18n()
  const alertRef = useRef<HTMLDivElement>(null)
  const [copied, setCopied] = useState(false)

  // Stamp the report once per mount so the <pre> the user reads and the text the
  // copy button writes are identical, and the timestamp does not jitter on every
  // render. The toast re-mounts per failure (the busy strip shows between
  // attempts), so this stays fresh.
  const [report] = useState(() =>
    buildBackupDiagnosticReport({
      message,
      rawError,
      errorKind,
      buildInfo,
      timestamp: new Date().toISOString(),
      userAgent: navigator.userAgent,
    }),
  )

  // Move focus to the alert so keyboard / screen-reader users land on the
  // recovery actions immediately. Not a focus trap — Tab exits into the app.
  useEffect(() => {
    const node = alertRef.current
    /* v8 ignore next -- the ref is attached to the rendered alert before this effect runs */
    if (!node) return
    node.focus({ preventScroll: true })
  }, [])

  // Auto-revert the "Copied" affordance so it reads as a transient confirmation.
  useEffect(() => {
    if (!copied) return
    const timer = window.setTimeout(() => setCopied(false), COPIED_RESET_MS)
    return () => window.clearTimeout(timer)
  }, [copied])

  const handleCopy = useCallback(() => {
    // Wrap the (possibly missing) clipboard call so a synchronous throw becomes
    // a rejection we can swallow — clipboard access can be blocked, and the user
    // can still select the report text in the details panel below.
    void Promise.resolve()
      .then(() => navigator.clipboard.writeText(report))
      .then(() => setCopied(true))
      .catch(() => {})
  }, [report])

  const isFullDiskAccess = errorKind === 'full-disk-access'

  return (
    <div
      ref={alertRef}
      role="alert"
      aria-atomic="true"
      aria-labelledby="pk-backup-failure-heading"
      tabIndex={-1}
      data-testid="backup-failure-toast"
      className={cn(
        'fixed inset-x-0 bottom-[32px] z-[20] flex items-stretch outline-none',
        'border-t border-error bg-paper/95 backdrop-blur supports-[backdrop-filter]:bg-paper/85',
        'shadow-[0_-2px_10px_rgba(28,24,20,0.10)]',
        'animate-[pk-backup-failure-in_300ms_ease-out_both]',
      )}
    >
      {/* Solid full-height error rail: the static, motion-independent pop-out. */}
      <div aria-hidden="true" className="w-1 shrink-0 bg-error" />
      <div className="flex min-w-0 flex-1 flex-col gap-1.5 px-4 py-3">
        <div className="flex items-center gap-2">
          <TriangleAlertIcon
            aria-hidden="true"
            className="size-4 shrink-0 text-error"
          />
          <span
            id="pk-backup-failure-heading"
            className="flex-1 font-sans text-[13px] font-semibold leading-snug text-error"
          >
            {t('shell.backupFailedHeading')}
          </span>
          <div className="flex flex-wrap items-center gap-2">
            {isFullDiskAccess ? (
              <button
                type="button"
                className="btn-secondary text-[12px]"
                onClick={onOpenFdaSettings}
              >
                {t('shell.fullDiskAccessOpenSettings')}
              </button>
            ) : null}
            <button
              type="button"
              className="btn-secondary text-[12px]"
              onClick={onRetry}
            >
              {t('shell.backupFailedRetry')}
            </button>
            <button
              type="button"
              className="btn-secondary text-[12px]"
              onClick={handleCopy}
            >
              {copied
                ? t('shell.backupFailedCopied')
                : t('shell.backupFailedCopyDiagnostics')}
            </button>
          </div>
          <button
            type="button"
            aria-label={t('shell.backupFailedDismiss')}
            className="text-ink-muted hover:text-ink hover:bg-hover ml-1 shrink-0 rounded-[3px] p-1 font-mono text-[12px] leading-none transition-colors"
            onClick={onDismiss}
          >
            <span aria-hidden="true">✕</span>
          </button>
        </div>
        <p className="m-0 font-sans text-[12px] leading-[1.45] text-ink-muted">
          {t('shell.backupFailedReassurance')}
        </p>
        <p className="m-0 font-sans text-[12px] leading-[1.5] text-ink-secondary">
          {message}
        </p>
        <details className="mt-0.5">
          <summary className="text-ink-muted hover:text-ink cursor-pointer select-none font-sans text-[11.5px]">
            {t('shell.backupFailedShowDetails')}
          </summary>
          <pre className="pk-scrollbar bg-hover mt-1.5 max-h-[120px] overflow-auto whitespace-pre-wrap break-all rounded-[3px] px-2 py-2 font-mono text-[11px] leading-[1.5] text-ink-secondary">
            {report}
          </pre>
          <div className="mt-1.5 flex gap-2">
            <button
              type="button"
              aria-label={t('shell.revealLogsAriaLabel')}
              className="btn-secondary text-[12px]"
              onClick={onRevealLogs}
            >
              {t('shell.revealLogs')}
            </button>
          </div>
        </details>
      </div>
      {/* Polite confirmation for the copy action without re-firing the assertive alert. */}
      <span className="sr-only" role="status">
        {copied ? t('shell.backupFailedCopied') : ''}
      </span>
    </div>
  )
}
