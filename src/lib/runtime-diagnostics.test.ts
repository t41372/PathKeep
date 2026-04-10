import { beforeEach, describe, expect, test, vi } from 'vitest'
import {
  installRuntimeDiagnostics,
  resetRuntimeDiagnosticsForTests,
} from './runtime-diagnostics'

const { attachConsoleMock, invokeCommandMock, isTauriMock, logErrorMock } =
  vi.hoisted(() => ({
    attachConsoleMock: vi.fn().mockResolvedValue(() => undefined),
    invokeCommandMock: vi.fn().mockResolvedValue({ ok: true }),
    isTauriMock: vi.fn(() => true),
    logErrorMock: vi.fn().mockResolvedValue(undefined),
  }))

vi.mock('@tauri-apps/api/core', () => ({
  isTauri: isTauriMock,
}))

vi.mock('@tauri-apps/plugin-log', () => ({
  attachConsole: attachConsoleMock,
  error: logErrorMock,
}))

vi.mock('./ipc/bridge', () => ({
  invokeCommand: invokeCommandMock,
}))

describe('runtime diagnostics', () => {
  beforeEach(() => {
    resetRuntimeDiagnosticsForTests()
    isTauriMock.mockReturnValue(true)
    attachConsoleMock.mockReset().mockResolvedValue(() => undefined)
    invokeCommandMock.mockReset().mockResolvedValue({ ok: true })
    logErrorMock.mockReset().mockResolvedValue(undefined)
  })

  test('skips Tauri-only diagnostics in browser preview mode', async () => {
    isTauriMock.mockReturnValue(false)

    await installRuntimeDiagnostics()
    window.dispatchEvent(new ErrorEvent('error', { message: 'boom' }))

    expect(attachConsoleMock).not.toHaveBeenCalled()
    expect(logErrorMock).not.toHaveBeenCalled()
    expect(invokeCommandMock).not.toHaveBeenCalled()
  })

  test('attaches console forwarding and records uncaught window errors', async () => {
    await installRuntimeDiagnostics()
    window.dispatchEvent(
      new ErrorEvent('error', {
        message: 'boom',
        filename: 'app://main',
        lineno: 12,
        colno: 4,
        error: new Error('boom'),
      }),
    )
    await Promise.resolve()

    expect(attachConsoleMock).toHaveBeenCalledTimes(1)
    expect(logErrorMock).toHaveBeenCalledWith('[window-error] boom')
    expect(invokeCommandMock).toHaveBeenCalledWith('record_frontend_error', {
      request: expect.objectContaining({
        source: 'window-error',
        message: 'boom',
        url: 'app://main',
        line: 12,
        column: 4,
        fatal: true,
      }),
    })
  })

  test('records unhandled promise rejections only once when installed twice', async () => {
    await installRuntimeDiagnostics()
    await installRuntimeDiagnostics()
    window.dispatchEvent(
      new PromiseRejectionEvent('unhandledrejection', {
        promise: Promise.reject(new Error('reject me')).catch(() => undefined),
        reason: new Error('reject me'),
      }),
    )
    await Promise.resolve()

    expect(attachConsoleMock).toHaveBeenCalledTimes(1)
    expect(logErrorMock).toHaveBeenCalledWith('[unhandledrejection] reject me')
    expect(invokeCommandMock).toHaveBeenCalledTimes(1)
  })

  test('keeps recording errors if console forwarding cannot attach', async () => {
    attachConsoleMock.mockRejectedValueOnce(new Error('no console'))

    await installRuntimeDiagnostics()
    window.dispatchEvent(
      new ErrorEvent('error', {
        message: 'boom after console failure',
      }),
    )
    await Promise.resolve()

    expect(logErrorMock).toHaveBeenCalledWith(
      '[window-error] boom after console failure',
    )
    expect(invokeCommandMock).toHaveBeenCalledTimes(1)
  })
})
