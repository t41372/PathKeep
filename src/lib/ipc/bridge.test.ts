/**
 * This test file protects the front-end helper and contract logic in Bridge.
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
    vi.unstubAllEnvs()
    vi.unstubAllGlobals()
    vi.stubGlobal('fetch', fetchMock)
    invokeMock.mockReset()
    isTauriMock.mockReturnValue(true)
    fetchMock.mockReset()
  })

  afterEach(() => {
    vi.unstubAllEnvs()
    vi.unstubAllGlobals()
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

  test('wraps tauri string failures into Error instances so shell refusals stay actionable', async () => {
    invokeMock.mockRejectedValueOnce(
      'database key is required for encrypted archives',
    )

    const { invokeCommand } = await import('./bridge')

    await expect(invokeCommand('app_snapshot')).rejects.toThrow(
      'database key is required for encrypted archives',
    )
  })

  test('rethrows tauri Error instances without changing their message', async () => {
    invokeMock.mockRejectedValueOnce(new Error('desktop refused'))

    const { invokeCommand } = await import('./bridge')

    await expect(invokeCommand('app_snapshot')).rejects.toThrow(
      'desktop refused',
    )
  })

  test('uses tauri invoke when the desktop webview only exposes __TAURI_INTERNALS__', async () => {
    isTauriMock.mockReturnValue(false)
    vi.stubGlobal('__TAURI_INTERNALS__', {
      invoke: vi.fn(),
    })
    invokeMock.mockResolvedValueOnce({ ok: true })

    const { invokeCommand } = await import('./bridge')
    const result = await invokeCommand<{ ok: boolean }>('app_snapshot')

    expect(invokeMock).toHaveBeenCalledWith('app_snapshot', undefined)
    expect(fetchMock).not.toHaveBeenCalled()
    expect(result).toEqual({ ok: true })
  })

  test('falls back to a generic tauri error when the rejection is not readable text', async () => {
    invokeMock.mockRejectedValueOnce({ code: 'boom' })

    const { invokeCommand } = await import('./bridge')

    await expect(invokeCommand('app_snapshot')).rejects.toThrow(
      'PathKeep desktop command "app_snapshot" failed.',
    )
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

  test('shapes unreachable desktop bridge failures into PathKeep-specific errors', async () => {
    isTauriMock.mockReturnValue(false)
    vi.stubEnv('VITE_PATHKEEP_DEV_IPC_URL', 'http://127.0.0.1:43117')
    fetchMock.mockRejectedValueOnce(new TypeError('fetch failed'))

    const { invokeCommand } = await import('./bridge')

    await expect(invokeCommand('app_snapshot')).rejects.toMatchObject({
      message:
        'PathKeep desktop command "app_snapshot" could not reach the local desktop bridge at http://127.0.0.1:43117. fetch failed',
    })
  })

  test('omits fetch detail when the desktop bridge rejection is not an Error object', async () => {
    isTauriMock.mockReturnValue(false)
    vi.stubEnv('VITE_PATHKEEP_DEV_IPC_URL', 'http://127.0.0.1:43117')
    fetchMock.mockRejectedValueOnce('socket closed')

    const { invokeCommand } = await import('./bridge')

    await expect(invokeCommand('app_snapshot')).rejects.toMatchObject({
      message:
        'PathKeep desktop command "app_snapshot" could not reach the local desktop bridge at http://127.0.0.1:43117.',
    })
  })
})
