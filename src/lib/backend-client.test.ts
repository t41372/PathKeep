/**
 * This test file protects the front-end helper and contract logic in Backend Client.
 *
 * Why this file exists:
 * - Pure helpers are where we keep UI policy testable without booting the whole shell.
 * - When these tests fail, they usually point at a contract drift that would otherwise show up as subtle route regressions.
 *
 * Main declarations:
 * - This file is mostly internal implementation detail.
 *
 * Source-of-truth notes:
 * - Helper behavior should stay aligned with the same design, feature, and architecture docs that guide the UI surfaces consuming it.
 * - Prefer focused behavioral assertions over snapshotting implementation detail.
 */

import { beforeEach, describe, expect, test, vi } from 'vitest'

const {
  invokeCommandMock,
  hasDesktopCommandTransportMock,
  backendHarnessMock,
} = vi.hoisted(() => ({
  invokeCommandMock: vi.fn(),
  hasDesktopCommandTransportMock: vi.fn(() => false),
  backendHarnessMock: {
    call: vi.fn(),
  },
}))

vi.mock('./ipc/bridge', () => ({
  invokeCommand: invokeCommandMock,
}))

vi.mock('./runtime', () => ({
  hasDesktopCommandTransport: hasDesktopCommandTransportMock,
}))

vi.mock('./backend', () => ({
  backendTestHarness: backendHarnessMock,
}))

describe('backend client', () => {
  beforeEach(() => {
    invokeCommandMock.mockReset()
    backendHarnessMock.call.mockReset()
    hasDesktopCommandTransportMock.mockReturnValue(false)
    ;(
      window as Window & {
        __PATHKEEP_DESKTOP_COMMAND_METRICS__?: Array<{
          command: string
          durationMs: number
          requestBytes: number
          responseBytes: number
          recordedAt: string
        }>
      }
    ).__PATHKEEP_DESKTOP_COMMAND_METRICS__ = []
  })

  test('uses the live desktop command transport when available', async () => {
    hasDesktopCommandTransportMock.mockReturnValue(true)
    invokeCommandMock.mockResolvedValueOnce({ version: '0.1.0' })

    const { backend } = await import('./backend-client')
    const result = await backend.getAppBuildInfo()

    expect(invokeCommandMock).toHaveBeenCalledWith('app_build_info', undefined)
    expect(result).toEqual({ version: '0.1.0' })
    expect(backendHarnessMock.call).not.toHaveBeenCalled()
  })

  test('falls back to the browser preview harness when no desktop transport exists', async () => {
    backendHarnessMock.call.mockResolvedValueOnce({ version: 'preview' })

    const { backend } = await import('./backend-client')
    const result = await backend.getAppBuildInfo()

    expect(backendHarnessMock.call).toHaveBeenCalledWith(
      'app_build_info',
      undefined,
    )
    expect(result).toEqual({ version: 'preview' })
    expect(invokeCommandMock).not.toHaveBeenCalled()
  })

  test('records desktop metrics without traversing the entire response payload', async () => {
    hasDesktopCommandTransportMock.mockReturnValue(true)
    const rows = Array.from({ length: 8 }, (_, index) => ({ index }))
    rows.length = 20
    Object.defineProperty(rows, 15, {
      enumerable: true,
      get() {
        throw new Error('desktop metrics should not read deep array entries')
      },
    })
    invokeCommandMock.mockResolvedValueOnce({
      version: '0.1.0',
      rows,
    })

    const { backend } = await import('./backend-client')
    const result = await backend.getAppBuildInfo()

    expect(result).toEqual({
      version: '0.1.0',
      rows,
    })
    const metrics = (
      window as Window & {
        __PATHKEEP_DESKTOP_COMMAND_METRICS__?: Array<{
          responseBytes: number
        }>
      }
    ).__PATHKEEP_DESKTOP_COMMAND_METRICS__
    expect(metrics).toHaveLength(1)
    expect(metrics?.[0]?.responseBytes).toBeGreaterThan(0)
  })
})
