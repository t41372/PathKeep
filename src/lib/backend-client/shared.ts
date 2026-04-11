/**
 * This module wraps a focused slice of desktop commands behind a typed front-end client.
 *
 * Why this file exists:
 * - The `backend-client` layer keeps page components from having to know raw command names or transport details.
 * - If a route needs desktop data, start here before reaching for legacy preview helpers.
 *
 * Main declarations:
 * - `call`
 *
 * Source-of-truth notes:
 * - Transport boundaries are defined by `docs/architecture/desktop-command-surface.md`.
 * - This layer should stay typed, boring, and free of user-facing copy so routes can keep ownership of UX decisions.
 */

import { invokeCommand } from '../ipc/bridge'
import { hasDesktopCommandTransport } from '../runtime'

/**
 * Defines the type-level contract for backend args.
 *
 * The backend-client layer exists to keep command names, transport, and route code decoupled, so focused declarations here are intentional.
 */
type BackendArgs = Record<string, unknown> | undefined

/**
 * Explains how call works.
 *
 * The backend-client layer exists to keep command names, transport, and route code decoupled, so focused declarations here are intentional.
 */
export async function call<T>(command: string, args?: BackendArgs): Promise<T> {
  if (hasDesktopCommandTransport()) {
    return invokeCommand<T>(command, args)
  }

  const { backendTestHarness } = await import('../backend')
  return backendTestHarness.call<T>(command, args)
}
