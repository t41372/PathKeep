/**
 * @file model-download.test.ts
 * @description Focused coverage for the in-app model download subscription + in-flight latch.
 * @module lib/ipc
 *
 * ## Responsibilities
 * - Verify the helper subscribes to the exact desktop progress channel and forwards payloads
 *   (ignoring empty ones), and degrades to a no-op unsubscribe when the bridge is unavailable.
 * - Verify the process-global in-flight latch flips with started/settled and reads back true only
 *   while a download is in flight.
 *
 * ## Not responsible for
 * - Re-testing the static download panel UI (covered in ai-providers-section.test).
 * - Re-testing the Tauri event implementation itself.
 *
 * ## Dependencies
 * - Mocks `@tauri-apps/api/event` at the module boundary.
 */

import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import {
  MODEL_DOWNLOAD_PROGRESS_EVENT,
  isModelDownloadInFlight,
  markModelDownloadSettled,
  markModelDownloadStarted,
  subscribeToModelDownloadProgress,
} from './model-download'
import type { ModelDownloadProgressEvent } from '../types'

const listen = vi.fn()

vi.mock('@tauri-apps/api/event', () => ({
  listen,
}))

describe('subscribeToModelDownloadProgress', () => {
  afterEach(() => {
    listen.mockReset()
  })

  test('subscribes to the desktop progress channel and forwards non-empty payloads', async () => {
    const unsubscribe = vi.fn()
    const received: ModelDownloadProgressEvent[] = []
    listen.mockImplementation((_event, handler) => {
      handler({ payload: { kind: 'fileStarted', file: 'a', totalBytes: 0 } })
      // An empty payload must be ignored, not forwarded.
      handler({ payload: null })
      handler({ payload: { kind: 'done' } })
      return Promise.resolve(unsubscribe)
    })

    const result = await subscribeToModelDownloadProgress((event) => {
      received.push(event)
    })

    expect(listen).toHaveBeenCalledWith(
      MODEL_DOWNLOAD_PROGRESS_EVENT,
      expect.any(Function),
    )
    expect(received).toEqual([
      { kind: 'fileStarted', file: 'a', totalBytes: 0 },
      { kind: 'done' },
    ])
    expect(result).toBe(unsubscribe)
  })

  test('returns a noop unsubscribe when the desktop event bridge is unavailable', async () => {
    const listener = vi.fn()
    listen.mockRejectedValueOnce(new Error('event bridge unavailable'))

    const result = await subscribeToModelDownloadProgress(listener)

    expect(result()).toBeUndefined()
    expect(listener).not.toHaveBeenCalled()
  })
})

describe('model download in-flight latch', () => {
  beforeEach(() => {
    // The latch is module-scoped (it survives a remount by design); normalize it before each test.
    markModelDownloadSettled()
  })

  test('reads false initially, true after started, false after settled', () => {
    expect(isModelDownloadInFlight()).toBe(false)
    markModelDownloadStarted()
    expect(isModelDownloadInFlight()).toBe(true)
    markModelDownloadSettled()
    expect(isModelDownloadInFlight()).toBe(false)
  })
})
