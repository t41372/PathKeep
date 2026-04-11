import { afterEach, describe, expect, test, vi } from 'vitest'
import { subscribeToUpdaterProgress } from './updater-progress'

const listen = vi.fn()

vi.mock('@tauri-apps/api/event', () => ({
  listen,
}))

describe('subscribeToUpdaterProgress', () => {
  afterEach(() => {
    listen.mockReset()
  })

  test('subscribes to the desktop updater progress channel and forwards payloads', async () => {
    const unsubscribe = vi.fn()
    const listener = vi.fn()
    listen.mockImplementation((_event, handler) => {
      handler({
        payload: {
          phase: 'downloading',
          version: '0.2.0',
          downloadedBytes: 40,
          contentLength: 100,
          message: 'Downloading PathKeep 0.2.0...',
        },
      })
      return Promise.resolve(unsubscribe)
    })

    const result = await subscribeToUpdaterProgress(listener)

    expect(listen).toHaveBeenCalledWith(
      'pathkeep://updater-progress',
      expect.any(Function),
    )
    expect(listener).toHaveBeenCalledWith(
      expect.objectContaining({
        phase: 'downloading',
        version: '0.2.0',
      }),
    )
    expect(result).toBe(unsubscribe)
  })
})
