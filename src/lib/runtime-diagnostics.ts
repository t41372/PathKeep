import { isTauri } from '@tauri-apps/api/core'
import { attachConsole, error as logError } from '@tauri-apps/plugin-log'
import { invokeCommand } from './ipc/bridge'

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

export async function installRuntimeDiagnostics() {
  if (diagnosticsInstalled || !isTauri()) {
    diagnosticsInstalled = diagnosticsInstalled || isTauri()
    return
  }
  diagnosticsInstalled = true

  try {
    await attachConsole()
  } catch {
    // Logs should still reach the file targets even if console forwarding fails.
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

async function persistFrontendError(request: FrontendErrorReportRequest) {
  const summary = `[${request.source}] ${request.message}`
  await Promise.allSettled([
    logError(summary),
    invokeCommand('record_frontend_error', { request }),
  ])
}

function describeUnhandledReason(reason: unknown) {
  if (reason instanceof Error) {
    return {
      message: reason.message || 'Unhandled promise rejection',
      stack: reason.stack ?? null,
    }
  }

  if (typeof reason === 'string' && reason.trim().length > 0) {
    return {
      message: reason,
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
