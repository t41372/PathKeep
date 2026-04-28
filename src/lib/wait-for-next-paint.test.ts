/**
 * This test file protects the paint-first scheduling contract shared by heavy workflows.
 *
 * Why this file exists:
 * - Busy overlays are only trustworthy if they can paint before desktop work starts.
 * - The helper must keep working in browser preview, jsdom, and throttled desktop shells.
 *
 * Main declarations:
 * - This file is mostly internal implementation detail.
 *
 * Source-of-truth notes:
 * - Prefer scheduler-order assertions over sleeping in tests.
 */

import { afterEach, describe, expect, test, vi } from 'vitest'
import { waitForNextPaint } from './wait-for-next-paint'

describe('waitForNextPaint', () => {
  afterEach(() => {
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
    vi.useRealTimers()
  })

  test('resolves immediately when no browser frame scheduler is available', async () => {
    vi.stubGlobal('window', undefined)

    await expect(waitForNextPaint()).resolves.toBeUndefined()
  })

  test('waits for an animation frame before resolving', async () => {
    vi.useFakeTimers()
    let frameScheduled = false
    let frameCallback: FrameRequestCallback = () => {
      throw new Error('requestAnimationFrame callback was not captured')
    }
    const requestAnimationFrameSpy = vi
      .spyOn(window, 'requestAnimationFrame')
      .mockImplementation((callback) => {
        frameCallback = callback
        frameScheduled = true
        return 1
      })
    const setTimeoutSpy = vi.spyOn(window, 'setTimeout')
    let settled = false

    const promise = waitForNextPaint().then(() => {
      settled = true
    })
    await Promise.resolve()

    expect(settled).toBe(false)
    expect(frameScheduled).toBe(true)
    expect(requestAnimationFrameSpy).toHaveBeenCalledTimes(1)
    expect(setTimeoutSpy).toHaveBeenCalledWith(expect.any(Function), 16)

    frameCallback(0)
    await promise

    expect(settled).toBe(true)
  })

  test('uses the timeout fallback when the frame callback never arrives', async () => {
    vi.useFakeTimers()
    vi.spyOn(window, 'requestAnimationFrame').mockImplementation(() => 1)

    const promise = waitForNextPaint()
    await vi.advanceTimersByTimeAsync(16)

    await expect(promise).resolves.toBeUndefined()
  })
})
