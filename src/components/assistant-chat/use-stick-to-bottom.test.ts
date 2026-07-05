/**
 * @file use-stick-to-bottom.test.ts
 * @description Coverage for the stick-to-bottom scroll model.
 *
 * Proves: starts sticky (auto-follow on); a scroll-up past the threshold turns following off; a
 * scroll back to the bottom turns it back on; `scrollToBottom()` jumps to the bottom and re-arms;
 * the boundary at exactly the threshold counts as sticky while one pixel past does not; the listener
 * re-binds when `attachKey` changes (conditionally-mounted node) and detaches on unmount; the rAF
 * dedup coalesces a burst of scroll events into a single sample.
 */

import { act, renderHook } from '@testing-library/react'
import { afterEach, describe, expect, test, vi } from 'vitest'
import {
  STICK_TO_BOTTOM_THRESHOLD_PX,
  useStickToBottom,
} from './use-stick-to-bottom'

/** A scroll element whose layout metrics are fully controllable (jsdom reports 0 for all). */
function makeNode(metrics: {
  scrollTop: number
  scrollHeight: number
  clientHeight: number
}): HTMLDivElement {
  const node = document.createElement('div')
  Object.defineProperty(node, 'scrollHeight', {
    value: metrics.scrollHeight,
    writable: true,
    configurable: true,
  })
  Object.defineProperty(node, 'clientHeight', {
    value: metrics.clientHeight,
    writable: true,
    configurable: true,
  })
  node.scrollTop = metrics.scrollTop
  document.body.appendChild(node)
  return node
}

/**
 * Fire a scroll event, then flush the hook's rAF-deduped sample. rAF is captured (not run inline) so
 * the hook finishes assigning `rafRef.current` BEFORE the frame fires — mirroring a real browser,
 * where rAF is always asynchronous — then the captured frame is flushed inside `act`.
 */
function fireScroll(node: HTMLElement) {
  const frames: FrameRequestCallback[] = []
  const raf = vi
    .spyOn(globalThis, 'requestAnimationFrame')
    .mockImplementation((cb: FrameRequestCallback) => {
      frames.push(cb)
      return frames.length
    })
  act(() => {
    node.dispatchEvent(new Event('scroll'))
  })
  act(() => frames.forEach((cb) => cb(0)))
  raf.mockRestore()
}

afterEach(() => {
  vi.restoreAllMocks()
  document.body.innerHTML = ''
})

describe('useStickToBottom', () => {
  test('starts sticky', () => {
    const node = makeNode({
      scrollTop: 0,
      scrollHeight: 1000,
      clientHeight: 400,
    })
    const ref = { current: node }
    const { result } = renderHook(() => useStickToBottom(ref))
    expect(result.current.stickToBottom).toBe(true)
  })

  test('turns following off when the user scrolls up past the threshold', () => {
    const node = makeNode({
      scrollTop: 0,
      scrollHeight: 1000,
      clientHeight: 400,
    })
    const ref = { current: node }
    const { result } = renderHook(() => useStickToBottom(ref))
    // distanceFromBottom = 1000 - 100 - 400 = 500 > threshold → not sticky.
    node.scrollTop = 100
    fireScroll(node)
    expect(result.current.stickToBottom).toBe(false)
  })

  test('treats exactly-at-the-threshold as sticky and one past as not', () => {
    const node = makeNode({
      scrollTop: 0,
      scrollHeight: 1000,
      clientHeight: 400,
    })
    const ref = { current: node }
    const { result } = renderHook(() => useStickToBottom(ref))
    // distance == threshold → still sticky (inclusive `<=`).
    node.scrollTop = 600 - STICK_TO_BOTTOM_THRESHOLD_PX
    fireScroll(node)
    expect(result.current.stickToBottom).toBe(true)
    // one pixel further up → not sticky.
    node.scrollTop = 600 - STICK_TO_BOTTOM_THRESHOLD_PX - 1
    fireScroll(node)
    expect(result.current.stickToBottom).toBe(false)
  })

  test('re-arms following when the user scrolls back to the bottom', () => {
    const node = makeNode({
      scrollTop: 0,
      scrollHeight: 1000,
      clientHeight: 400,
    })
    const ref = { current: node }
    const { result } = renderHook(() => useStickToBottom(ref))
    node.scrollTop = 100 // up → off
    fireScroll(node)
    expect(result.current.stickToBottom).toBe(false)
    node.scrollTop = 600 // back to bottom (distance 0) → on
    fireScroll(node)
    expect(result.current.stickToBottom).toBe(true)
  })

  test('scrollToBottom jumps to the bottom and re-arms following', () => {
    const node = makeNode({
      scrollTop: 0,
      scrollHeight: 1000,
      clientHeight: 400,
    })
    const ref = { current: node }
    const { result } = renderHook(() => useStickToBottom(ref))
    node.scrollTop = 100 // scrolled up → off
    fireScroll(node)
    expect(result.current.stickToBottom).toBe(false)
    act(() => result.current.scrollToBottom())
    expect(node.scrollTop).toBe(node.scrollHeight)
    expect(result.current.stickToBottom).toBe(true)
  })

  test('scrollToBottom is a no-op when the ref is detached', () => {
    const ref = { current: null as HTMLElement | null }
    const { result } = renderHook(() => useStickToBottom(ref))
    // Must not throw with no node attached.
    act(() => result.current.scrollToBottom())
    expect(result.current.stickToBottom).toBe(true)
  })

  test('coalesces a burst of scroll events into one rAF sample', () => {
    const node = makeNode({
      scrollTop: 0,
      scrollHeight: 1000,
      clientHeight: 400,
    })
    const ref = { current: node }
    const { result } = renderHook(() => useStickToBottom(ref))
    const pending: FrameRequestCallback[] = []
    const raf = vi
      .spyOn(globalThis, 'requestAnimationFrame')
      .mockImplementation((cb: FrameRequestCallback) => {
        pending.push(cb)
        return pending.length
      })
    node.scrollTop = 100
    act(() => {
      node.dispatchEvent(new Event('scroll'))
      node.dispatchEvent(new Event('scroll'))
      node.dispatchEvent(new Event('scroll'))
    })
    // Three scrolls, one scheduled frame (dedup).
    expect(pending).toHaveLength(1)
    act(() => pending.forEach((cb) => cb(0)))
    expect(result.current.stickToBottom).toBe(false)
    raf.mockRestore()
  })

  test('re-binds the listener when attachKey changes (conditionally-mounted node)', () => {
    // Two distinct nodes behind one ref object: the second only "mounts" after attachKey flips.
    const first = makeNode({
      scrollTop: 0,
      scrollHeight: 1000,
      clientHeight: 400,
    })
    const ref = { current: first as HTMLElement | null }
    const { result, rerender } = renderHook(
      ({ key }: { key: boolean }) => useStickToBottom(ref, key),
      { initialProps: { key: false } },
    )
    // Swap in a fresh node (the body re-mounted on open) and bump the key.
    const second = makeNode({
      scrollTop: 100,
      scrollHeight: 1000,
      clientHeight: 400,
    })
    ref.current = second
    rerender({ key: true })
    // The listener now lives on `second`: a scroll there updates the flag.
    fireScroll(second)
    expect(result.current.stickToBottom).toBe(false)
  })

  test('cleans up the scroll listener on unmount', () => {
    const node = makeNode({
      scrollTop: 0,
      scrollHeight: 1000,
      clientHeight: 400,
    })
    const remove = vi.spyOn(node, 'removeEventListener')
    const ref = { current: node as HTMLElement | null }
    const { unmount } = renderHook(() => useStickToBottom(ref))
    unmount()
    expect(remove).toHaveBeenCalledWith('scroll', expect.any(Function))
  })

  test('cancels a pending rAF sample on unmount', () => {
    const node = makeNode({
      scrollTop: 0,
      scrollHeight: 1000,
      clientHeight: 400,
    })
    const ref = { current: node as HTMLElement | null }
    const raf = vi
      .spyOn(globalThis, 'requestAnimationFrame')
      .mockImplementation(() => 42)
    const cancel = vi.spyOn(globalThis, 'cancelAnimationFrame')
    const { unmount } = renderHook(() => useStickToBottom(ref))
    // Schedule a frame but never flush it.
    act(() => {
      node.dispatchEvent(new Event('scroll'))
    })
    unmount()
    expect(cancel).toHaveBeenCalledWith(42)
    raf.mockRestore()
  })

  test('does nothing when the ref has no node at mount', () => {
    const ref = { current: null as HTMLElement | null }
    const { result } = renderHook(() => useStickToBottom(ref))
    expect(result.current.stickToBottom).toBe(true)
  })
})
