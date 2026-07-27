/**
 * This module gates the paper-redesign QA overlay mounts to dev builds.
 *
 * Why this file exists:
 * - Settings / Intelligence / Audit / Import mount half-finished paper
 *   redesign panels on top of the shipped UI when the route carries
 *   `?layout=paper`. Those mounts are QA-only overlays, not product surface.
 * - 2026-07-26 user decision: withdraw these QA mounts from production
 *   builds. Production ignores the `layout=paper` param entirely; only dev
 *   builds keep the overlay for inline QA of the redesign.
 * - The `devBuild` parameter is injectable (defaulting to
 *   `import.meta.env.DEV`) so BOTH branches — dev shows the overlay,
 *   production ignores the param — can be tested honestly instead of only
 *   asserting whatever mode the test runner happens to run in.
 *
 * Main declarations:
 * - `isPaperQaLayoutEnabled`
 *
 * Source-of-truth notes:
 * - Explorer's `surface=search` / `surface=starred` params are REAL route
 *   params, not QA overlays — they are intentionally not routed through this
 *   gate.
 */

/**
 * Returns whether the paper-redesign QA overlay should mount for this route.
 *
 * Exists so every QA mount site shares one dev-only gate instead of each
 * route re-deciding what `?layout=paper` means in production.
 */
export function isPaperQaLayoutEnabled(
  searchParams: URLSearchParams,
  devBuild: boolean = import.meta.env.DEV,
): boolean {
  return devBuild && searchParams.get('layout') === 'paper'
}
