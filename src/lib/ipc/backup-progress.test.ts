import { afterEach, describe, expect, test, vi } from 'vitest'
import { subscribeToBackupProgress } from './backup-progress'

const listen = vi.fn()

vi.mock('@tauri-apps/api/event', () => ({
  listen,
}))

describe('subscribeToBackupProgress', () => {
  afterEach(() => {
    listen.mockReset()
  })

  test('subscribes to the desktop backup progress channel and forwards payloads', async () => {
    const unsubscribe = vi.fn()
    const listener = vi.fn()
    listen.mockImplementation((_event, handler) => {
      handler({
        payload: {
          phase: 'ingest-profile',
          label: 'Write canonical archive facts',
          detail: 'chrome:Default (1/1)',
          step: 1,
          totalSteps: 3,
          completedProfiles: 0,
          totalProfiles: 1,
          profileId: 'chrome:Default',
        },
      })
      return Promise.resolve(unsubscribe)
    })

    const result = await subscribeToBackupProgress(listener)

    expect(listen).toHaveBeenCalledWith(
      'pathkeep://backup-progress',
      expect.any(Function),
    )
    expect(listener).toHaveBeenCalledWith(
      expect.objectContaining({
        phase: 'ingest-profile',
        profileId: 'chrome:Default',
      }),
    )
    expect(result).toBe(unsubscribe)
  })
})
