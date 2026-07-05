/**
 * This test file protects the front-end helper and contract logic in Archive Upgrade Progress.
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
import { subscribeToArchiveUpgradeProgress } from './archive-upgrade-progress'

const listen = vi.fn()

vi.mock('@tauri-apps/api/event', () => ({
  listen,
}))

describe('subscribeToArchiveUpgradeProgress', () => {
  afterEach(() => {
    listen.mockReset()
  })

  test('subscribes to the archive-upgrade channel and forwards payloads', async () => {
    const unsubscribe = vi.fn()
    const listener = vi.fn()
    listen.mockImplementation((_event, handler) => {
      handler({
        payload: {
          phase: 'registrableDomainBackfill',
          phaseLabel: 'archiveUpgrade.phase.registrableDomainBackfill',
          processed: 500,
          total: 12000,
          done: false,
        },
      })
      return Promise.resolve(unsubscribe)
    })

    const result = await subscribeToArchiveUpgradeProgress(listener)

    expect(listen).toHaveBeenCalledWith(
      'pathkeep://archive-upgrade',
      expect.any(Function),
    )
    expect(listener).toHaveBeenCalledWith(
      expect.objectContaining({
        phase: 'registrableDomainBackfill',
        processed: 500,
      }),
    )
    expect(result).toBe(unsubscribe)
  })

  test('ignores empty payloads and degrades to a noop unsubscribe', async () => {
    const listener = vi.fn()
    listen.mockImplementationOnce((_event, handler) => {
      handler({ payload: null })
      return Promise.resolve(vi.fn())
    })

    await subscribeToArchiveUpgradeProgress(listener)
    expect(listener).not.toHaveBeenCalled()

    listen.mockReset()
    listen.mockRejectedValueOnce(new Error('event bridge unavailable'))
    const result = await subscribeToArchiveUpgradeProgress(listener)
    expect(result()).toBeUndefined()
  })
})
