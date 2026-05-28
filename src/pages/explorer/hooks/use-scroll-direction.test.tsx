/**
 * Tests the scroll-direction signal — RAF-deduped scroll listener
 * with hysteresis. Drives `window.scrollY` directly and flushes RAF
 * to simulate sample steps.
 */

import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import { useScrollDirection } from './use-scroll-direction'

function setScrollY(y: number) {
  Object.defineProperty(window, 'scrollY', { value: y, configurable: true })
}

function dispatchScroll() {
  window.dispatchEvent(new Event('scroll'))
}

describe('useScrollDirection', () => {
  let originalScrollY: number

  beforeEach(() => {
    originalScrollY = window.scrollY
    setScrollY(0)
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
    setScrollY(originalScrollY)
  })

  test('returns "idle" until enough consecutive same-direction samples accumulate', () => {
    const { result } = renderHook(() =>
      useScrollDirection({ hysteresisFrames: 4, deltaThresholdPx: 1 }),
    )
    expect(result.current).toBe('idle')

    // Single down-scroll sample — not enough to flip direction yet.
    act(() => {
      setScrollY(100)
      dispatchScroll()
      vi.runAllTimers()
    })
    expect(result.current).toBe('idle')
  })

  test('flips to "down" after `hysteresisFrames` consecutive downward samples', () => {
    const { result } = renderHook(() =>
      useScrollDirection({ hysteresisFrames: 3, deltaThresholdPx: 1 }),
    )
    for (let i = 1; i <= 3; i += 1) {
      act(() => {
        setScrollY(i * 50)
        dispatchScroll()
        vi.runAllTimers()
      })
    }
    expect(result.current).toBe('down')
  })

  test('keeps a stable direction once the sampled direction already matches', () => {
    const { result } = renderHook(() =>
      useScrollDirection({ hysteresisFrames: 1, deltaThresholdPx: 1 }),
    )

    act(() => {
      setScrollY(50)
      dispatchScroll()
      vi.runAllTimers()
    })
    expect(result.current).toBe('down')

    act(() => {
      setScrollY(100)
      dispatchScroll()
      vi.runAllTimers()
    })
    expect(result.current).toBe('down')
  })

  test('dedupes multiple scroll events into one pending animation frame', () => {
    const rafSpy = vi.spyOn(window, 'requestAnimationFrame')
    renderHook(() =>
      useScrollDirection({ hysteresisFrames: 3, deltaThresholdPx: 1 }),
    )

    act(() => {
      setScrollY(50)
      dispatchScroll()
      dispatchScroll()
    })
    expect(rafSpy).toHaveBeenCalledTimes(1)

    act(() => {
      vi.runAllTimers()
    })
    rafSpy.mockRestore()
  })

  test('flips to "up" after `hysteresisFrames` consecutive upward samples; a reversed sample resets the count', () => {
    const { result } = renderHook(() =>
      useScrollDirection({ hysteresisFrames: 3, deltaThresholdPx: 1 }),
    )
    // Three down samples → direction = down.
    for (let i = 1; i <= 3; i += 1) {
      act(() => {
        setScrollY(i * 50)
        dispatchScroll()
        vi.runAllTimers()
      })
    }
    expect(result.current).toBe('down')

    // Two up samples — not enough to flip yet (still says "down").
    act(() => {
      setScrollY(100)
      dispatchScroll()
      vi.runAllTimers()
    })
    act(() => {
      setScrollY(50)
      dispatchScroll()
      vi.runAllTimers()
    })
    expect(result.current).toBe('down')

    // Third up sample flips the signal.
    act(() => {
      setScrollY(10)
      dispatchScroll()
      vi.runAllTimers()
    })
    expect(result.current).toBe('up')
  })

  test('a sub-threshold delta resets the hysteresis count so the direction stays idle until a fresh same-direction streak rebuilds it', () => {
    const { result } = renderHook(() =>
      useScrollDirection({ hysteresisFrames: 3, deltaThresholdPx: 5 }),
    )
    // Two large-enough down samples → count = 2 (not yet >=
    // hysteresis, so direction is still 'idle').
    act(() => {
      setScrollY(50)
      dispatchScroll()
      vi.runAllTimers()
    })
    act(() => {
      setScrollY(100)
      dispatchScroll()
      vi.runAllTimers()
    })
    // Sub-threshold wobble — below 5 px from 100 → 102. Per the
    // hook contract (see use-scroll-direction.ts §sample), a
    // sub-threshold sample resets `sameDirCountRef` to 0 AND
    // `lastSampledDirRef` to 'idle' so the hysteresis has to
    // re-charge from scratch. Reset matches a user pausing
    // mid-scroll: their next directional intent should be treated
    // as a fresh decision, not a continuation of the prior streak.
    act(() => {
      setScrollY(102)
      dispatchScroll()
      vi.runAllTimers()
    })
    // The reset means we never reached the 3-frame threshold, so
    // direction is still 'idle'.
    expect(result.current).toBe('idle')
  })

  test('cleans up the scroll listener on unmount', () => {
    const removeSpy = vi.spyOn(window, 'removeEventListener')
    const { unmount } = renderHook(() => useScrollDirection())
    unmount()
    expect(removeSpy.mock.calls.some(([event]) => event === 'scroll')).toBe(
      true,
    )
    removeSpy.mockRestore()
  })

  test('cancels a pending animation-frame sample on unmount', () => {
    const cancelSpy = vi.spyOn(window, 'cancelAnimationFrame')
    const { unmount } = renderHook(() => useScrollDirection())

    act(() => {
      setScrollY(50)
      dispatchScroll()
    })
    unmount()

    expect(cancelSpy).toHaveBeenCalledTimes(1)
    cancelSpy.mockRestore()
  })
})
