import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'

const { invokeMock, isTauriMock, fetchMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  isTauriMock: vi.fn(() => true),
  fetchMock: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({
  isTauri: isTauriMock,
  invoke: invokeMock,
}))

describe('ipc bridge', () => {
  beforeEach(() => {
    vi.stubGlobal('fetch', fetchMock)
    vi.unstubAllEnvs()
    invokeMock.mockReset()
    isTauriMock.mockReturnValue(true)
    fetchMock.mockReset()
  })

  afterEach(() => {
    vi.unstubAllEnvs()
  })

  test('forwards typed commands to the tauri invoke layer', async () => {
    invokeMock.mockResolvedValueOnce({ ok: true })

    const { invokeCommand } = await import('./bridge')
    const result = await invokeCommand<{ ok: boolean }>('app_snapshot', {
      includePreview: true,
    })

    expect(invokeMock).toHaveBeenCalledWith('app_snapshot', {
      includePreview: true,
    })
    expect(result).toEqual({ ok: true })
  })

  test('falls back to the desktop bridge when chrome is connected to the tauri runtime', async () => {
    isTauriMock.mockReturnValue(false)
    vi.stubEnv('VITE_PATHKEEP_DEV_IPC_URL', 'http://127.0.0.1:43117/')
    fetchMock.mockResolvedValueOnce({
      ok: true,
      status: 200,
      text: () => Promise.resolve('{"ok":true}'),
    })

    const { invokeCommand } = await import('./bridge')
    const result = await invokeCommand<{ ok: boolean }>('app_snapshot', {
      includePreview: true,
    })

    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:43117/commands/app_snapshot',
      {
        method: 'POST',
        headers: {
          'content-type': 'application/json',
        },
        body: '{"includePreview":true}',
      },
    )
    expect(result).toEqual({ ok: true })
  })

  test('throws a clear error when browser preview has no desktop bridge configured', async () => {
    isTauriMock.mockReturnValue(false)

    const { invokeCommand } = await import('./bridge')

    await expect(invokeCommand('app_snapshot')).rejects.toThrow(
      'PathKeep desktop command "app_snapshot" is unavailable in browser preview mode.',
    )
    expect(fetchMock).not.toHaveBeenCalled()
  })

  test('accepts empty desktop bridge bodies for void commands', async () => {
    isTauriMock.mockReturnValue(false)
    vi.stubEnv('VITE_PATHKEEP_DEV_IPC_URL', 'http://127.0.0.1:43117')
    fetchMock.mockResolvedValueOnce({
      ok: true,
      status: 200,
      text: () => Promise.resolve(''),
    })

    const { invokeCommand } = await import('./bridge')
    const result = await invokeCommand<null>('clear_session_database_key')

    expect(result).toBeNull()
  })

  test('surfaces desktop bridge failures as errors', async () => {
    isTauriMock.mockReturnValue(false)
    vi.stubEnv('VITE_PATHKEEP_DEV_IPC_URL', 'http://127.0.0.1:43117')
    fetchMock.mockResolvedValueOnce({
      ok: false,
      status: 500,
      text: () => Promise.resolve('{"error":"bridge exploded"}'),
    })

    const { invokeCommand } = await import('./bridge')

    await expect(invokeCommand('app_snapshot')).rejects.toThrow(
      'bridge exploded',
    )
  })

  test('falls back to the HTTP status when the desktop bridge omits an error message', async () => {
    isTauriMock.mockReturnValue(false)
    vi.stubEnv('VITE_PATHKEEP_DEV_IPC_URL', 'http://127.0.0.1:43117')
    fetchMock.mockResolvedValueOnce({
      ok: false,
      status: 502,
      text: () => Promise.resolve('{"message":"bad gateway"}'),
    })

    const { invokeCommand } = await import('./bridge')

    await expect(invokeCommand('app_snapshot')).rejects.toThrow(
      'PathKeep desktop command "app_snapshot" failed with HTTP 502.',
    )
  })
})
