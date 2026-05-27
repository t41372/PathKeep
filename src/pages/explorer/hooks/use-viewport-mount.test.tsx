/**
 * Tests for useViewportMount — IO-driven render gating used by
 * PaperContactSheet to recycle off-screen day blocks.
 */

import { useEffect } from 'react'
import { act, render, renderHook, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, test } from 'vitest'
import { useViewportMount } from './use-viewport-mount'

type TriggerFn = (node: Element, intersecting: boolean) => void
type TriggerEmptyFn = (node: Element) => void

function installObserverMock(): {
  trigger: TriggerFn
  triggerEmpty: TriggerEmptyFn
  restore: () => void
} {
  type Callback = (entries: IntersectionObserverEntry[]) => void
  const subscribers = new Map<Element, Callback>()
  function MockIO(this: Record<string, unknown>, callback: Callback) {
    this.callback = callback
    this.root = null
    this.rootMargin = ''
    this.thresholds = [0]
    this.observe = (node: Element) => {
      subscribers.set(node, callback)
    }
    this.unobserve = (node: Element) => {
      subscribers.delete(node)
    }
    this.disconnect = () => {
      for (const node of [...subscribers.keys()]) {
        if (subscribers.get(node) === callback) subscribers.delete(node)
      }
    }
    this.takeRecords = () => []
  }
  const previous = globalThis.IntersectionObserver
  ;(
    globalThis as { IntersectionObserver: typeof IntersectionObserver }
  ).IntersectionObserver = MockIO as unknown as typeof IntersectionObserver
  return {
    trigger(node, isIntersecting) {
      const callback = subscribers.get(node)
      if (!callback) return
      callback([
        {
          isIntersecting,
          target: node,
          boundingClientRect: node.getBoundingClientRect(),
          intersectionRatio: isIntersecting ? 1 : 0,
          intersectionRect: node.getBoundingClientRect(),
          rootBounds: null,
          time: 0,
        } as IntersectionObserverEntry,
      ])
    },
    triggerEmpty(node) {
      const callback = subscribers.get(node)
      if (!callback) return
      callback([])
    },
    restore() {
      ;(
        globalThis as { IntersectionObserver: typeof IntersectionObserver }
      ).IntersectionObserver = previous
    },
  }
}

function Harness({
  onState,
  options,
}: {
  onState: (state: ReturnType<typeof useViewportMount>) => void
  options?: Parameters<typeof useViewportMount>[0]
}) {
  const state = useViewportMount<HTMLDivElement>(options)
  // Defer the side effect to commit time so the render itself stays
  // pure (and the React lint rule banning ref reads + side effects
  // during render stays satisfied).
  useEffect(() => {
    onState(state)
    // The hook returns a fresh object every render, so we listen on
    // its observable fields instead of the wrapper identity.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state.inView, state.measuredHeight])
  return (
    <div ref={state.ref} data-testid="harness">
      {state.inView ? 'mounted' : 'recycled'}
    </div>
  )
}

describe('useViewportMount', () => {
  let io: ReturnType<typeof installObserverMock>
  beforeEach(() => {
    io = installObserverMock()
  })
  afterEach(() => {
    io.restore()
  })

  test('starts inView=true so first paint renders everything', () => {
    const states: ReturnType<typeof useViewportMount>[] = []
    render(<Harness onState={(s) => states.push(s)} />)
    expect(states[states.length - 1].inView).toBe(true)
  })

  test('no-ops before a consumer assigns the observed ref', () => {
    const { result } = renderHook(() => useViewportMount<HTMLDivElement>())

    expect(result.current.inView).toBe(true)
    expect(result.current.measuredHeight).toBeNull()
  })

  test('flips inView=false when IntersectionObserver reports out-of-view, capturing measured height', () => {
    const states: ReturnType<typeof useViewportMount>[] = []
    const { getByTestId } = render(<Harness onState={(s) => states.push(s)} />)
    const node = getByTestId('harness')
    // Pretend the node was 480px tall before we recycled it.
    node.getBoundingClientRect = () =>
      ({
        height: 480,
        width: 0,
        top: 0,
        left: 0,
        bottom: 480,
        right: 0,
        x: 0,
        y: 0,
        toJSON: () => ({}),
      }) as DOMRect
    act(() => {
      io.trigger(node, false)
    })
    const last = states[states.length - 1]
    expect(last.inView).toBe(false)
    expect(last.measuredHeight).toBe(480)
  })

  test('does not record a placeholder height when the recycled node has no height', () => {
    const states: ReturnType<typeof useViewportMount>[] = []
    const { getByTestId } = render(<Harness onState={(s) => states.push(s)} />)
    const node = getByTestId('harness')
    node.getBoundingClientRect = () =>
      ({
        height: 0,
        width: 0,
        top: 0,
        left: 0,
        bottom: 0,
        right: 0,
        x: 0,
        y: 0,
        toJSON: () => ({}),
      }) as DOMRect

    act(() => {
      io.trigger(node, false)
    })

    expect(states[states.length - 1].inView).toBe(false)
    expect(states[states.length - 1].measuredHeight).toBeNull()
  })

  test('ignores observer callbacks that arrive without entries', () => {
    const states: ReturnType<typeof useViewportMount>[] = []
    const { getByTestId } = render(<Harness onState={(s) => states.push(s)} />)
    const node = getByTestId('harness')

    act(() => {
      io.triggerEmpty(node)
    })

    expect(states[states.length - 1].inView).toBe(true)
  })

  test('flips inView back to true on re-entry without changing a matching measured height', async () => {
    const states: ReturnType<typeof useViewportMount>[] = []
    const { getByTestId } = render(<Harness onState={(s) => states.push(s)} />)
    const node = getByTestId('harness')
    node.getBoundingClientRect = () =>
      ({
        height: 240,
        width: 0,
        top: 0,
        left: 0,
        bottom: 240,
        right: 0,
        x: 0,
        y: 0,
        toJSON: () => ({}),
      }) as DOMRect
    act(() => {
      io.trigger(node, false)
    })
    act(() => {
      io.trigger(node, true)
    })
    await waitFor(() => {
      expect(states[states.length - 1].inView).toBe(true)
    })
    expect(states[states.length - 1].measuredHeight).toBe(240)
  })

  test('respects initialInView=false (e.g. server-rendered placeholder)', () => {
    const states: ReturnType<typeof useViewportMount>[] = []
    render(
      <Harness
        onState={(s) => states.push(s)}
        options={{ initialInView: false }}
      />,
    )
    expect(states[0].inView).toBe(false)
  })

  test('skip=true bypasses the IntersectionObserver entirely and always reports inView=true (review §11)', () => {
    let observerCount = 0
    const previousIO = globalThis.IntersectionObserver
    ;(
      globalThis as { IntersectionObserver: typeof IntersectionObserver }
    ).IntersectionObserver = function MockCountingIO() {
      observerCount += 1
      // Return a stub that satisfies the interface but isn't actually
      // used — we're only counting whether it was constructed.
      return {
        observe: () => {},
        unobserve: () => {},
        disconnect: () => {},
        takeRecords: () => [],
        root: null,
        rootMargin: '',
        thresholds: [0],
      } as unknown as IntersectionObserver
    } as unknown as typeof IntersectionObserver

    const states: ReturnType<typeof useViewportMount>[] = []
    render(
      <Harness
        onState={(s) => states.push(s)}
        options={{ skip: true, initialInView: false }}
      />,
    )
    expect(observerCount).toBe(0)
    // Even with `initialInView: false`, skip forces `inView=true` so
    // the consumer renders content.
    expect(states[states.length - 1].inView).toBe(true)
    ;(
      globalThis as { IntersectionObserver: typeof IntersectionObserver }
    ).IntersectionObserver = previousIO
  })

  test('no-ops when IntersectionObserver is unavailable, staying at initialInView', () => {
    io.restore()
    const previous = globalThis.IntersectionObserver
    ;(
      globalThis as {
        IntersectionObserver: typeof IntersectionObserver | undefined
      }
    ).IntersectionObserver = undefined as unknown as typeof IntersectionObserver
    const states: ReturnType<typeof useViewportMount>[] = []
    render(<Harness onState={(s) => states.push(s)} />)
    expect(states[states.length - 1].inView).toBe(true)
    ;(
      globalThis as { IntersectionObserver: typeof IntersectionObserver }
    ).IntersectionObserver = previous
  })
})
