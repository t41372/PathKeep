/**
 * This module records front-end runtime failures so diagnostics stay visible in the shipped desktop app.
 *
 * Why this file exists:
 * - Files in `src/lib/` are where UI policy becomes testable without inflating every route component.
 * - If you are trying to understand a front-end contract quickly, these helpers usually explain the reusable part of the story.
 *
 * Main declarations:
 * - `installRuntimeDiagnostics`
 * - `resetRuntimeDiagnosticsForTests`
 *
 * Source-of-truth notes:
 * - Keep helper behavior aligned with the shipping design, feature, and architecture docs rather than local route assumptions.
 * - Avoid burying user-visible copy or route-only workflow rules here unless the helper truly owns that cross-cutting contract.
 */

import { attachConsole, error as logError } from '@tauri-apps/plugin-log'
import { invokeCommand } from './ipc/bridge'
import { hasDesktopCommandTransport, hasTauriGuestApi } from './runtime'

/**
 * Describes a request payload in this front-end contract.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
interface FrontendErrorReportRequest {
  source: string
  message: string
  stack?: string | null
  url?: string | null
  line?: number | null
  column?: number | null
  fatal: boolean
}

let diagnosticsInstalled = false
let errorHandler: ((event: ErrorEvent) => void) | null = null
let rejectionHandler: ((event: PromiseRejectionEvent) => void) | null = null

/**
 * Explains how install runtime diagnostics works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export async function installRuntimeDiagnostics() {
  if (diagnosticsInstalled || !hasDesktopCommandTransport()) {
    return
  }
  diagnosticsInstalled = true

  if (hasTauriGuestApi()) {
    try {
      await attachConsole()
    } catch {
      // Logs should still reach the file targets even if console forwarding fails.
    }
  }

  errorHandler = (event) => {
    void persistFrontendError({
      source: 'window-error',
      message: event.message || 'Unhandled window error',
      stack:
        event.error instanceof Error
          ? (event.error.stack ?? event.error.message)
          : null,
      url: event.filename || null,
      line: event.lineno || null,
      column: event.colno || null,
      fatal: true,
    })
  }
  rejectionHandler = (event) => {
    const reason = describeUnhandledReason(event.reason)
    void persistFrontendError({
      source: 'unhandledrejection',
      message: reason.message,
      stack: reason.stack,
      url: null,
      line: null,
      column: null,
      fatal: true,
    })
  }

  window.addEventListener('error', errorHandler)
  window.addEventListener('unhandledrejection', rejectionHandler)
}

/**
 * Explains how persist frontend error works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
async function persistFrontendError(request: FrontendErrorReportRequest) {
  const summary = `[${request.source}] ${request.message}`
  const writes: Promise<unknown>[] = [
    invokeCommand('record_frontend_error', { request }),
  ]
  if (hasTauriGuestApi()) {
    writes.unshift(logError(summary))
  }
  await Promise.allSettled(writes)
}

/**
 * Explains how describe unhandled reason works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
function describeUnhandledReason(reason: unknown) {
  if (reason instanceof Error) {
    return {
      message: reason.message || 'Unhandled promise rejection',
      stack: reason.stack ?? null,
    }
  }

  if (typeof reason === 'string') {
    const message = reason.trim()
    if (!message) {
      return {
        message: 'Unhandled promise rejection',
        stack: null,
      }
    }
    return {
      message,
      stack: null,
    }
  }

  try {
    return {
      message: JSON.stringify(reason) || 'Unhandled promise rejection',
      stack: null,
    }
  } catch {
    return {
      message: 'Unhandled promise rejection with a non-serializable reason.',
      stack: null,
    }
  }
}

/**
 * Explains how reset runtime diagnostics for tests works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function resetRuntimeDiagnosticsForTests() {
  diagnosticsInstalled = false
  if (errorHandler) {
    window.removeEventListener('error', errorHandler)
    errorHandler = null
  }
  if (rejectionHandler) {
    window.removeEventListener('unhandledrejection', rejectionHandler)
    rejectionHandler = null
  }
}
