/**
 * This test file protects the front-end helper and contract logic in Updater Progress.
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
