/**
 * @file preload.ts
 * @description Prioritized idle warming for the non-custom `/intelligence` overview presets.
 * @module lib/core-intelligence
 *
 * ## Responsibilities
 * - Warm the all-time overview (the default scope) first, on app start and after
 *   archive data changes, so the route paints from the persisted snapshot.
 * - Then, at lower priority, warm the other non-custom presets the user can open
 *   instantly within a session/day — month, quarter, year (in that order) — each
 *   staggered behind the previous one via idle callbacks so background warming
 *   never competes with all-time or first paint.
 *
 * ## Not responsible for
 * - Deciding when the archive changed (the shell owns that signal via `refreshKey`).
 * - Building or persisting the all-time snapshot (the backend owns that, keyed by
 *   an archive fingerprint, so repeat all-time warms are cheap when nothing changed).
 * - Choosing which profile scope to warm (the shell passes the route's scope).
 *
 * ## Dependencies
 * - `dateRangeFromPreset` for each preset's window mapping.
 * - The overview loaders, which seed the same module-level cache the route reads.
 *
 * ## Performance notes
 * - All-time is forced (`force: true`) because the backend serves it from a
 *   fingerprint-keyed snapshot, so a re-warm after a data change is a fingerprint
 *   check, not a full recompute. Month/quarter/year have NO backend snapshot, so
 *   each cold warm is a real (but smaller) recompute over the whole ~14.4M-visit
 *   archive; they are warmed with `force: false` so an already-warm same-day cache
 *   entry is a no-op. Their cache key includes the range whose `end` is today, so
 *   it naturally invalidates on the next calendar day. Because `force: false`
 *   cannot replace a same-day entry on its own, every mutation that changes the
 *   visible archive must call `clearIntelligenceOverviewCache()` before triggering
 *   the re-warm; the import, settings rebuild/reset, and audit revert/restore
 *   seams do this so the next warm refills from fresh data. (A caller that mutates
 *   the archive and forgets the clear would serve stale same-day counts until the
 *   day rolls over — see those seams for the contract.) Only primary overviews are
 *   warmed for the bounded presets; their secondary grids stay lazy to bound
 *   background cost.
 */

import {
  loadIntelligencePrimaryOverview,
  loadIntelligenceSecondaryOverview,
} from './api'
import { dateRangeFromPreset } from './hooks'
import type { TimeRangePreset } from './types'

type IdleWindow = Window & {
  requestIdleCallback?: (
    callback: IdleRequestCallback,
    options?: IdleRequestOptions,
  ) => number
  cancelIdleCallback?: (handle: number) => void
}

/**
 * Lower-priority presets warmed after all-time, in descending likelihood of
 * being opened next. Month is the most-used bounded scope, so it leads; year is
 * the broadest of the three, so it trails.
 */
const LOWER_PRIORITY_PRESETS: readonly TimeRangePreset[] = [
  'month',
  'quarter',
  'year',
]

/**
 * Schedules `callback` for an idle slot on `targetWindow` and returns a cleanup
 * that cancels the pending slot.
 *
 * Falls back to a short macrotask when `requestIdleCallback` is unavailable
 * (jsdom, older Safari) so warming still happens without blocking first paint.
 */
function scheduleIdle(targetWindow: IdleWindow, callback: () => void) {
  if (typeof targetWindow.requestIdleCallback === 'function') {
    const handle = targetWindow.requestIdleCallback(() => callback(), {
      timeout: 1200,
    })
    return () => targetWindow.cancelIdleCallback?.(handle)
  }

  const handle = targetWindow.setTimeout(callback, 160)
  return () => targetWindow.clearTimeout(handle)
}

/**
 * Warms the all-time overview (primary then secondary) and resolves once both
 * settle. Forced so a same-day data change replaces the stale warm entry; the
 * backend snapshot keeps the unchanged case cheap.
 */
function warmAllTimeOverview(profileId: string | null): Promise<void> {
  const range = dateRangeFromPreset('all')
  return loadIntelligencePrimaryOverview(range, profileId, {
    force: true,
  })
    .then(() =>
      loadIntelligenceSecondaryOverview(range, profileId, {
        force: true,
      }),
    )
    .then(() => {})
}

/**
 * Warms a bounded preset's primary overview only, and resolves once it settles.
 *
 * `force: false` makes an already-warm same-day entry a no-op; the secondary
 * grid is intentionally left lazy to bound background recompute cost for the
 * snapshot-less bounded presets.
 */
function warmPresetPrimaryOverview(
  preset: TimeRangePreset,
  profileId: string | null,
): Promise<void> {
  const range = dateRangeFromPreset(preset)
  return loadIntelligencePrimaryOverview(range, profileId, {
    force: false,
  }).then(() => {})
}

/**
 * Schedules an idle warm of the all-time overview for the given profile scope
 * and returns a cleanup function that cancels the pending warm.
 *
 * Kept as the focused entry point for callers that only want the snapshot-backed
 * all-time scope warmed (e.g. the shell's archive-wide companion warm, which must
 * not pay the bounded presets' cold-recompute cost a second time). Delegates to
 * the same scheduling and warm primitives as {@link preloadIntelligenceOverviews}.
 *
 * `profileId` should match the scope the route will request (archive-wide when
 * null) so the warmed cache entry is reused on open. Failures are swallowed:
 * warming is best-effort and the route still loads on its own.
 */
export function preloadAllTimeIntelligenceOverview(
  profileId: string | null,
  targetWindow: IdleWindow | null = window,
): () => void {
  if (!targetWindow) {
    return () => {}
  }

  return scheduleIdle(targetWindow, () => {
    void warmAllTimeOverview(profileId).catch(() => {})
  })
}

/**
 * Schedules a prioritized, idle-staggered warm of the non-custom `/intelligence`
 * presets for the given profile scope and returns a cleanup that cancels every
 * pending idle handle.
 *
 * Priority order: all-time first (forced, snapshot-backed, cheap), then month,
 * quarter, year — each scheduled only after the previous one settles, on its own
 * idle slot, so the lower-priority warms never compete with all-time or first
 * paint. Each bounded preset warms its primary overview with `force: false`, so a
 * warm same-day cache entry is a no-op and the heavy recompute happens only when
 * the day rolls over or the cache was cleared by an archive change.
 *
 * `profileId` should match the scope the route will request (archive-wide when
 * null). All warms are best-effort: loader rejections are swallowed and the next
 * preset is still scheduled, because a failed warm must never break the chain or
 * the route's own on-open load.
 */
export function preloadIntelligenceOverviews(
  profileId: string | null,
  targetWindow: IdleWindow | null = window,
): () => void {
  if (!targetWindow) {
    return () => {}
  }

  // Pending idle handles, newest last. Cleanup cancels each so a dependency
  // change (scope switch, archive re-warm) cannot leave a staggered warm armed.
  const cancels: Array<() => void> = []
  let cancelled = false

  // Schedules the next lower-priority preset (if any) once the previous warm
  // settles, chaining one idle slot per preset so they stay staggered and never
  // batch onto the same frame as all-time.
  const scheduleNext = (index: number) => {
    if (cancelled || index >= LOWER_PRIORITY_PRESETS.length) {
      return
    }
    const preset = LOWER_PRIORITY_PRESETS[index]
    const cancel = scheduleIdle(targetWindow, () => {
      void warmPresetPrimaryOverview(preset, profileId)
        .catch(() => {})
        .then(() => scheduleNext(index + 1))
    })
    cancels.push(cancel)
  }

  const cancelAllTime = scheduleIdle(targetWindow, () => {
    void warmAllTimeOverview(profileId)
      .catch(() => {})
      .then(() => scheduleNext(0))
  })
  cancels.push(cancelAllTime)

  return () => {
    cancelled = true
    for (const cancel of cancels) {
      cancel()
    }
  }
}
