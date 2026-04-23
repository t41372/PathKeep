/**
 * @file explorer-preferences.ts
 * @description Shared Explorer preference bounds used by the Settings route, preview state, and Explorer data hooks.
 * @module lib
 *
 * ## Responsibilities
 * - Define the shipped default for Explorer background page prefetching.
 * - Keep the allowed range explicit so config writes and preview fixtures do not drift.
 * - Provide one normalization helper that turns invalid values back into the accepted default.
 *
 * ## Not responsible for
 * - Persisting Explorer page-size preferences in local storage.
 * - Running history queries or managing route-level loading state.
 * - Replacing backend-side config normalization.
 *
 * ## Dependencies
 * - No runtime dependencies.
 *
 * ## Performance notes
 * - The prefetch window is intentionally bounded because each extra page can fan out into another history query against very large archives.
 */

export const defaultExplorerBackgroundPrefetchPages = 5
export const maxExplorerBackgroundPrefetchPages = 10

/**
 * Clamps the Explorer background prefetch window back to the shipped range.
 *
 * The Settings route and preview backend both use this helper so manual config
 * edits or invalid browser-preview writes do not accidentally create unbounded
 * background history reads.
 */
export function normalizeExplorerBackgroundPrefetchPages(
  value: number | null | undefined,
) {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return defaultExplorerBackgroundPrefetchPages
  }

  const normalized = Math.trunc(value)
  if (normalized < 0) {
    return 0
  }

  if (normalized > maxExplorerBackgroundPrefetchPages) {
    return maxExplorerBackgroundPrefetchPages
  }

  return normalized
}

export const explorerBackgroundPrefetchPageOptions = Array.from(
  { length: maxExplorerBackgroundPrefetchPages + 1 },
  (_, index) => index,
)
