/**
 * Tests for useViewportMount — IO-driven render gating used by
 * PaperContactSheet to recycle off-screen day blocks.
 */

import { act, render } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, test } from 'vitest'
import { useViewportMount } from './use-viewport-mount'

type TriggerFn = (node: Element, intersecting: boolean) => void

function installObserverMock(): { trigger: TriggerFn; restore: () => void } {
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
  ;(globalThis as { IntersectionObserver: typeof IntersectionObserver }).IntersectionObserver =
    MockIO as unknown as typeof IntersectionObserver
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
    restore() {
      ;(globalThis as { IntersectionObserver: typeof IntersectionObserver }).IntersectionObserver =
        previous
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
  onState(state)
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

  test('flips inView=false when IntersectionObserver reports out-of-view, capturing measured height', () => {
    const states: ReturnType<typeof useViewportMount>[] = []
    const { getByTestId } = render(<Harness onState={(s) => states.push(s)} />)
    const node = getByTestId('harness')
    // Pretend the node was 480px tall before we recycled it.
    node.getBoundingClientRect = () =>
      ({ height: 480, width: 0, top: 0, left: 0, bottom: 480, right: 0, x: 0, y: 0, toJSON: () => ({}) }) as DOMRect
    act(() => {
      io.trigger(node, false)
    })
    const last = states[states.length - 1]
    expect(last.inView).toBe(false)
    expect(last.measuredHeight).toBe(480)
  })

  test('flips inView back to true on re-entry', () => {
    const states: ReturnType<typeof useViewportMount>[] = []
    const { getByTestId } = render(<Harness onState={(s) => states.push(s)} />)
    const node = getByTestId('harness')
    node.getBoundingClientRect = () =>
      ({ height: 240, width: 0, top: 0, left: 0, bottom: 240, right: 0, x: 0, y: 0, toJSON: () => ({}) }) as DOMRect
    act(() => {
      io.trigger(node, false)
    })
    act(() => {
      io.trigger(node, true)
    })
    expect(states[states.length - 1].inView).toBe(true)
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

  test('no-ops when IntersectionObserver is unavailable, staying at initialInView', () => {
    io.restore()
    const previous = globalThis.IntersectionObserver
    ;(globalThis as { IntersectionObserver: typeof IntersectionObserver | undefined }).IntersectionObserver =
      undefined as unknown as typeof IntersectionObserver
    const states: ReturnType<typeof useViewportMount>[] = []
    render(<Harness onState={(s) => states.push(s)} />)
    expect(states[states.length - 1].inView).toBe(true)
    ;(globalThis as { IntersectionObserver: typeof IntersectionObserver }).IntersectionObserver =
      previous
  })
})
