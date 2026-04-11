/**
 * This module creates the router instance used by the shipped app and by focused tests.
 *
 * Why this file exists:
 * - Files under `src/app/` explain how the desktop shell is stitched together before route-specific UI takes over.
 * - This is where shared profile scope, app-lock gating, route metadata, and shell-level loading grammar should stay readable.
 *
 * Main declarations:
 * - `AppRouter`
 * - `createDesktopRouter`
 *
 * Source-of-truth notes:
 * - Keep this aligned with `docs/design/screens-and-nav.md` for information architecture and route semantics.
 * - Keep busy, locked, degraded, and loading behavior aligned with `docs/design/ux-principles.md`.
 */

import { createHashRouter } from 'react-router-dom'
import { appRoutes } from './router'

/**
 * Defines the type-level contract for app router.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
export type AppRouter = ReturnType<typeof createHashRouter>

/**
 * Creates desktop router.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
export function createDesktopRouter() {
  return createHashRouter(appRoutes)
}
