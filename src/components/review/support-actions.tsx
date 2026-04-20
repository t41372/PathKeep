/**
 * Shared clipboard and path-action grammar for review-heavy support surfaces.
 *
 * Why this file exists:
 * - M12 promotes open/copy-path support affordances into the same canonical
 *   owner as the rest of the app-wide review grammar.
 * - Settings, Audit, Import, Schedule, Security, Lock, and Explorer should
 *   stop hand-rolling the same clipboard and path-row behavior.
 */

import type { ReactNode } from 'react'
import { ReviewCopyStatus, type ReviewCopyFeedback } from './review-surface'

export function ReviewPathActionRow({
  copyFeedback,
  copyKey,
  copyLabel,
  errorMessage,
  label,
  onCopy,
  onOpenPath,
  openPathLabel,
  secondaryAction,
  status,
  successMessage,
  value,
}: {
  copyFeedback: ReviewCopyFeedback | null
  copyKey?: string
  copyLabel: string
  errorMessage: string
  label: ReactNode
  onCopy?: (key: string, value: string) => void
  onOpenPath?: (path: string) => void
  openPathLabel: string
  secondaryAction?: ReactNode
  status?: ReactNode
  successMessage: string
  value: string
}) {
  const resolvedCopyKey = copyKey ?? `path:${value}`
  const hasActions = Boolean(onOpenPath || onCopy || secondaryAction)

  return (
    <div className="review-path-action-row">
      <div className="config-row">
        <span className="config-label">{label}</span>
        <span className="config-value mono" title={value}>
          {value}
        </span>
        {hasActions ? (
          <span className="settings-action-row">
            {onOpenPath ? (
              <button
                className="btn-tiny"
                type="button"
                onClick={() => {
                  void onOpenPath(value)
                }}
              >
                {openPathLabel}
              </button>
            ) : null}
            {onCopy ? (
              <button
                className="btn-tiny"
                type="button"
                onClick={() => {
                  void onCopy(resolvedCopyKey, value)
                }}
              >
                {copyLabel}
              </button>
            ) : null}
            {secondaryAction}
          </span>
        ) : null}
      </div>
      {status ? <div className="dashboard-next-action">{status}</div> : null}
      {onCopy ? (
        <ReviewCopyStatus
          copyFeedback={copyFeedback}
          copyKey={resolvedCopyKey}
          errorMessage={errorMessage}
          successMessage={successMessage}
        />
      ) : null}
    </div>
  )
}
