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

// ── Browse view-mode persistence ────────────────────────────────────────
// The paper Browse contact sheet can render rows as cards or as a dense
// list. Users keep the same archive open for hours and toggle once at the
// start of a session, so the choice should outlive a route remount /
// reload. We persist in localStorage rather than the app config because
// it's a UI affordance, not user data: switching devices intentionally
// resets to the cards default the design tool ships.

export type ExplorerViewMode = 'cards' | 'list'

const EXPLORER_VIEW_MODE_STORAGE_KEY = 'pathkeep.explorerViewMode'
export const defaultExplorerViewMode: ExplorerViewMode = 'cards'

export function readExplorerViewMode(): ExplorerViewMode {
  if (typeof window === 'undefined') return defaultExplorerViewMode
  try {
    const raw = window.localStorage.getItem(EXPLORER_VIEW_MODE_STORAGE_KEY)
    return raw === 'list' ? 'list' : 'cards'
  } catch {
    return defaultExplorerViewMode
  }
}

export function persistExplorerViewMode(mode: ExplorerViewMode): void {
  if (typeof window === 'undefined') return
  if (readExplorerViewMode() === mode) return
  try {
    window.localStorage.setItem(EXPLORER_VIEW_MODE_STORAGE_KEY, mode)
  } catch {
    // localStorage may be unavailable (private mode, embed). Failing
    // silently is fine — the route uses the in-memory state for the
    // rest of the session and re-attempts persistence on the next toggle.
  }
}

// ── Time format ─────────────────────────────────────────────────────────
// Default to 12-hour clock with AM/PM (上午 / 下午 in zh) because the
// Browse contact sheet is read like a journal — "3:14 PM" matches how
// most users read their own day back. Power users in tech / Europe can
// flip to 24h in Settings → Display. Sparkline / chart axes keep 24h
// regardless because compactness wins there.

export type ClockFormat = '12h' | '24h'

const CLOCK_FORMAT_STORAGE_KEY = 'pathkeep.clockFormat'
export const defaultClockFormat: ClockFormat = '12h'

export function readClockFormat(): ClockFormat {
  if (typeof window === 'undefined') return defaultClockFormat
  try {
    const raw = window.localStorage.getItem(CLOCK_FORMAT_STORAGE_KEY)
    return raw === '24h' ? '24h' : defaultClockFormat
  } catch {
    return defaultClockFormat
  }
}

export function persistClockFormat(format: ClockFormat): void {
  if (typeof window === 'undefined') return
  if (readClockFormat() === format) return
  try {
    window.localStorage.setItem(CLOCK_FORMAT_STORAGE_KEY, format)
  } catch {
    // Mirrors persistExplorerViewMode — silent best-effort.
  }
  // Notify any live route (Browse, Search results, panels) so the new
  // format takes effect without requiring a remount or refresh.
  try {
    window.dispatchEvent(
      new CustomEvent<ClockFormatEventDetail>(CLOCK_FORMAT_EVENT, {
        detail: { format },
      }),
    )
  } catch {
    // Older webviews without CustomEvent constructor: persistence is the
    // source of truth on next mount.
  }
}

/**
 * Custom event dispatched on `window` whenever the clock format changes
 * so already-mounted routes can pick up the new format without a remount.
 */
export const CLOCK_FORMAT_EVENT = 'pathkeep.clockFormatChanged'

export interface ClockFormatEventDetail {
  format: ClockFormat
}
