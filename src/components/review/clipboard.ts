/**
 * Shared clipboard boundary for review-heavy support surfaces.
 *
 * Why this file exists:
 * - M12 promotes clipboard feedback into the same canonical owner as the
 *   app-wide review grammar.
 * - Routes that only need clipboard behavior should not re-implement the same
 *   `navigator.clipboard` branching logic.
 */

import type { ReviewCopyFeedback } from './review-surface'

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
