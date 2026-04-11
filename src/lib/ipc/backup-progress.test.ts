/**
 * This test file protects the front-end helper and contract logic in Backup Progress.
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
