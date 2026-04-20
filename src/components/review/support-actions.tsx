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
import {
  ReviewCopyStatus,
  type ReviewCopyFeedback,
} from './review-surface'

/**
 * Copies a value to the clipboard and optionally reports shared feedback.
 *
 * The helper stays generic so routes that do not need visible status can still
 * reuse the same clipboard boundary without inventing a second implementation.
 */
export async function copyReviewValue(
  value: string,
  options?: {
    key?: string
    onFeedback?: (feedback: ReviewCopyFeedback) => void
  },
) {
  let tone: ReviewCopyFeedback['tone'] = 'success'

  try {
    const clipboard = globalThis.navigator?.clipboard
    if (!clipboard?.writeText) {
      throw new Error('clipboard unavailable')
    }
    await clipboard.writeText(value)
  } catch {
    tone = 'error'
  }

  options?.onFeedback?.({
    key: options.key ?? value,
    tone,
  })

  return tone
}

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
