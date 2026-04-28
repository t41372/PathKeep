/**
 * @file search-keywords-browser-helpers.ts
 * @description Pure helper owner for the shared Search Keywords browser.
 * @module components/intelligence
 *
 * ## Responsibilities
 * - Keep defensive pagination normalization outside the React render module.
 * - Provide a stable test target for edge cases produced by restored browser form state.
 *
 * ## Not responsible for
 * - Rendering keyword rows, filters, or pagination controls.
 * - Loading search-query data from Core Intelligence.
 *
 * ## Dependencies
 * - Depends only on JavaScript number parsing.
 *
 * ## Performance notes
 * - Runs in constant time on user input; it never inspects keyword result rows.
 */

/**
 * Keeps page jumps deterministic when browser number inputs or restored form state produce invalid text.
 *
 * The UI normally constrains this input to a number, but the shared browser is also used in previews and test fixtures where a persisted string can be replayed.
 */
export function clampSearchKeywordPage(value: string, pageCount: number) {
  const parsed = Number(value)
  const pageNumber = Number.isFinite(parsed) ? parsed : 1
  return Math.min(
    Math.max(Math.trunc(pageNumber) - 1, 0),
    Math.max(pageCount - 1, 0),
  )
}
