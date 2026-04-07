import { describe, expect, test, vi } from 'vitest'

const invokeMock = vi.hoisted(() => vi.fn())

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}))

describe('ipc bridge', () => {
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
})
