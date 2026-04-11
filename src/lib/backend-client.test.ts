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
})
