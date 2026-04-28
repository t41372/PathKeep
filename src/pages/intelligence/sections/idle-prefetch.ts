/**
 * @file idle-prefetch.ts
 * @description Idle-prefetch scheduling helper for Intelligence sections.
 * @module pages/intelligence/sections
 *
 * ## Responsibilities
 * - Wrap `requestIdleCallback` with a deterministic timeout.
 * - Provide a no-op cleanup path for non-browser test and preview contexts.
 *
 * ## Not responsible for
 * - Choosing which Intelligence data should be prefetched.
 * - Rendering Search Activity or Activity Mix sections.
 *
 * ## Dependencies
 * - Depends only on optional browser idle-callback APIs.
 *
 * ## Performance notes
 * - Uses idle time when available and falls back to a short macrotask so hidden tabs do not block first paint.
 */

/**
 * Schedules a non-critical prefetch without requiring every test environment to expose idle callbacks.
 *
 * Route sections use this to keep secondary reads off the initial render path while still returning a cleanup function for unmount races.
 */
export function scheduleIdlePrefetch(
  callback: () => void,
  targetWindow:
    | (Window & {
        requestIdleCallback?: (
          cb: IdleRequestCallback,
          options?: IdleRequestOptions,
        ) => number
        cancelIdleCallback?: (handle: number) => void
      })
    | null = window,
) {
  if (!targetWindow) {
    return () => {}
  }

  if (typeof targetWindow.requestIdleCallback === 'function') {
    const handle = targetWindow.requestIdleCallback(() => callback(), {
      timeout: 1200,
    })
    return () => targetWindow.cancelIdleCallback?.(handle)
  }

  const handle = targetWindow.setTimeout(callback, 160)
  return () => targetWindow.clearTimeout(handle)
}
