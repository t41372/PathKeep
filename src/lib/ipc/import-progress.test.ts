/**
 * @file import-progress.test.ts
 * @description Focused coverage for Browser Direct / Takeout foreground progress events.
 * @module lib/ipc
 *
 * ## Responsibilities
 * - Verify the import progress helper subscribes to the exact desktop event channel.
 * - Verify empty payloads are ignored and transport failures degrade to a noop unsubscribe.
 *
 * ## Not responsible for
 * - Re-testing import route rendering or progress copy.
 * - Re-testing the Tauri event implementation itself.
 *
 * ## Dependencies
 * - Mocks `@tauri-apps/api/event` at the module boundary.
 *
 * ## Performance notes
 * - Pure unit test; no desktop process or file IO.
 */

import { afterEach, describe, expect, test, vi } from 'vitest'
import { subscribeToImportProgress } from './import-progress'

const listen = vi.fn()

vi.mock('@tauri-apps/api/event', () => ({
  listen,
}))

describe('subscribeToImportProgress', () => {
  afterEach(() => {
    listen.mockReset()
  })

  test('subscribes to the desktop import progress channel and forwards payloads', async () => {
    const unsubscribe = vi.fn()
    const listener = vi.fn()
    listen.mockImplementation((_event, handler) => {
      handler({
        payload: {
          phase: 'executing',
          label: 'Importing Safari history',
          detail: '240 / 587 rows',
          current: 240,
          total: 587,
          percent: 40.9,
          logLines: ['Streaming browser rows into the archive'],
        },
      })
      handler({ payload: null })
      return Promise.resolve(unsubscribe)
    })

    const result = await subscribeToImportProgress(listener)

    expect(listen).toHaveBeenCalledWith(
      'pathkeep://import-progress',
      expect.any(Function),
    )
    expect(listener).toHaveBeenCalledTimes(1)
    expect(listener).toHaveBeenCalledWith(
      expect.objectContaining({
        phase: 'executing',
        current: 240,
        total: 587,
      }),
    )
    expect(result).toBe(unsubscribe)
  })

  test('returns a noop unsubscribe when the desktop event bridge is unavailable', async () => {
    const listener = vi.fn()
    listen.mockRejectedValueOnce(new Error('event bridge unavailable'))

    const result = await subscribeToImportProgress(listener)

    expect(result()).toBeUndefined()
    expect(listener).not.toHaveBeenCalled()
  })
})
