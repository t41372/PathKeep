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

  test('deltas smaller than `deltaThresholdPx` are dropped without resetting the count', () => {
    const { result } = renderHook(() =>
      useScrollDirection({ hysteresisFrames: 3, deltaThresholdPx: 5 }),
    )
    // Two large-enough down samples.
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
    // Sub-threshold wobble — still below 5px from 100 → 102.
    act(() => {
      setScrollY(102)
      dispatchScroll()
      vi.runAllTimers()
    })
    // Idle still because hysteresis counter was reset by the
    // sub-threshold sample.
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
})
