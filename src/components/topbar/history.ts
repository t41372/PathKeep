/**
 * @file history.ts
 * @description Browser-history helper owner for topbar navigation controls.
 * @module components/topbar
 *
 * ## Responsibilities
 * - Read the React Router browser history index defensively.
 * - Keep pure navigation helpers outside the Topbar component module for Fast Refresh stability.
 *
 * ## Not responsible for
 * - Rendering topbar chrome or handling button clicks.
 * - Deciding which route is active.
 *
 * ## Dependencies
 * - Depends only on the browser `window.history.state` shape used by React Router.
 *
 * ## Performance notes
 * - Runs in constant time during render to decide whether the back button is enabled.
 */

/**
 * Reads the current React Router history index without assuming a browser global exists.
 *
 * The topbar is exercised in browser preview tests and route-level unit tests, so missing globals or unexpected state must degrade to "no back history" instead of throwing.
 */
export function readRouteHistoryIndex(
  targetWindow: Pick<Window, 'history'> | null | undefined = typeof window ===
  'undefined'
    ? undefined
    : window,
) {
  if (!targetWindow) {
    return 0
  }

  const historyState = targetWindow.history.state as { idx?: unknown } | null
  return typeof historyState?.idx === 'number' ? historyState.idx : 0
}
