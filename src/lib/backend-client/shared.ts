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

interface DesktopCommandMetric {
  command: string
  durationMs: number
  requestBytes: number
  responseBytes: number
  recordedAt: string
}

function serializedBytes(value: unknown) {
  try {
    return JSON.stringify(value).length
  } catch {
    return 0
  }
}

function recordDesktopCommandMetric(metric: DesktopCommandMetric) {
  if (typeof window === 'undefined') {
    return
  }

  const runtimeWindow = window as Window & {
    __PATHKEEP_DESKTOP_COMMAND_METRICS__?: DesktopCommandMetric[]
  }
  const next = runtimeWindow.__PATHKEEP_DESKTOP_COMMAND_METRICS__ ?? []
  next.push(metric)
  if (next.length > 200) {
    next.splice(0, next.length - 200)
  }
  runtimeWindow.__PATHKEEP_DESKTOP_COMMAND_METRICS__ = next
}

/**
 * Explains how call works.
 *
 * The backend-client layer exists to keep command names, transport, and route code decoupled, so focused declarations here are intentional.
 */
export async function call<T>(command: string, args?: BackendArgs): Promise<T> {
  const startedAt = performance.now()
  if (hasDesktopCommandTransport()) {
    const result = await invokeCommand<T>(command, args)
    recordDesktopCommandMetric({
      command,
      durationMs: performance.now() - startedAt,
      requestBytes: serializedBytes(args),
      responseBytes: serializedBytes(result),
      recordedAt: new Date().toISOString(),
    })
    return result
  }

  const { backendTestHarness } = await import('../backend')
  const result = await backendTestHarness.call<T>(command, args)
  recordDesktopCommandMetric({
    command,
    durationMs: performance.now() - startedAt,
    requestBytes: serializedBytes(args),
    responseBytes: serializedBytes(result),
    recordedAt: new Date().toISOString(),
  })
  return result
}
