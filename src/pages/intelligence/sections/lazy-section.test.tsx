/**
 * @file lazy-section.test.tsx
 * @description Unit tests for the viewport-gated LazySection wrapper.
 * @module pages/intelligence/sections
 *
 * ## Responsibilities
 * - Verify LazySection renders the skeleton until it is near the viewport, then
 *   mounts its children once the IntersectionObserver reports intersection.
 * - Verify the no-IntersectionObserver fallback (jsdom / SSR) mounts children
 *   immediately so nothing stays stuck behind a skeleton.
 * - Verify `forceMount` bypasses the observer entirely.
 * - Verify the observer is disconnected after mounting and on unmount.
 *
 * ## Not responsible for
 * - The hub coordinator composition (covered in sections.test.tsx).
 *
 * ## Dependencies
 * - A controllable IntersectionObserver mock, since jsdom omits the real one.
 */

import { render, screen } from '@testing-library/react'
import { act } from 'react'
import { afterEach, describe, expect, test, vi } from 'vitest'
import { LazySection } from './lazy-section'

type Callback = (entries: IntersectionObserverEntry[]) => void

interface ObserverMock {
  /** Fire an intersection entry for every observed node. */
  trigger: (isIntersecting: boolean) => void
  /** How many observers are currently connected (observing, not disconnected). */
  connectedCount: () => number
  restore: () => void
}

/**
 * Installs a controllable IntersectionObserver so the lazy mount path can be
 * driven deterministically — jsdom does not fire real entries.
 */
function installObserverMock(): ObserverMock {
  const subscribers = new Set<{ callback: Callback; nodes: Set<Element> }>()

  class MockIO {
    private record: { callback: Callback; nodes: Set<Element> }
    constructor(callback: Callback) {
      this.record = { callback, nodes: new Set() }
      subscribers.add(this.record)
    }
    observe(node: Element) {
      this.record.nodes.add(node)
    }
    unobserve(node: Element) {
      this.record.nodes.delete(node)
    }
    disconnect() {
      subscribers.delete(this.record)
    }
    takeRecords(): IntersectionObserverEntry[] {
      return []
    }
  }

  const previous = globalThis.IntersectionObserver
  ;(
    globalThis as { IntersectionObserver: typeof IntersectionObserver }
  ).IntersectionObserver = MockIO as unknown as typeof IntersectionObserver

  return {
    trigger(isIntersecting) {
      // Snapshot subscribers first: a triggered callback disconnects its own
      // observer, mutating the set mid-iteration.
      for (const record of [...subscribers]) {
        const entries = [...record.nodes].map(
          (node) =>
            ({
              isIntersecting,
              target: node,
              intersectionRatio: isIntersecting ? 1 : 0,
            }) as IntersectionObserverEntry,
        )
        if (entries.length > 0) record.callback(entries)
      }
    },
    connectedCount() {
      return subscribers.size
    },
    restore() {
      ;(
        globalThis as {
          IntersectionObserver: typeof IntersectionObserver | undefined
        }
      ).IntersectionObserver = previous
    },
  }
}

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('LazySection', () => {
  test('shows the skeleton until intersection, then mounts children', () => {
    const observer = installObserverMock()
    try {
      render(
        <LazySection skeleton={<div data-testid="skeleton" />}>
          <div data-testid="real">Real content</div>
        </LazySection>,
      )

      // Before any intersection, only the skeleton is in the DOM.
      expect(screen.getByTestId('skeleton')).toBeInTheDocument()
      expect(screen.queryByTestId('real')).not.toBeInTheDocument()
      expect(observer.connectedCount()).toBe(1)

      act(() => observer.trigger(true))

      // Once near the viewport, children mount and the skeleton is gone.
      expect(screen.getByTestId('real')).toBeInTheDocument()
      expect(screen.queryByTestId('skeleton')).not.toBeInTheDocument()
      // The observer disconnects after the one-shot mount.
      expect(observer.connectedCount()).toBe(0)
    } finally {
      observer.restore()
    }
  })

  test('keeps the skeleton while the section is reported out of view', () => {
    const observer = installObserverMock()
    try {
      render(
        <LazySection skeleton={<div data-testid="skeleton" />}>
          <div data-testid="real" />
        </LazySection>,
      )

      act(() => observer.trigger(false))

      expect(screen.getByTestId('skeleton')).toBeInTheDocument()
      expect(screen.queryByTestId('real')).not.toBeInTheDocument()
      // A non-intersecting entry must not disconnect — we still need to learn
      // when the section finally scrolls near.
      expect(observer.connectedCount()).toBe(1)
    } finally {
      observer.restore()
    }
  })

  test('disconnects the observer on unmount', () => {
    const observer = installObserverMock()
    try {
      const { unmount } = render(
        <LazySection skeleton={<div data-testid="skeleton" />}>
          <div data-testid="real" />
        </LazySection>,
      )
      expect(observer.connectedCount()).toBe(1)

      unmount()

      expect(observer.connectedCount()).toBe(0)
    } finally {
      observer.restore()
    }
  })

  test('mounts immediately when IntersectionObserver is unavailable', () => {
    // jsdom default: no IntersectionObserver. The section must not get stuck
    // behind a skeleton.
    vi.stubGlobal('IntersectionObserver', undefined)

    render(
      <LazySection skeleton={<div data-testid="skeleton" />}>
        <div data-testid="real">Real content</div>
      </LazySection>,
    )

    expect(screen.getByTestId('real')).toBeInTheDocument()
    expect(screen.queryByTestId('skeleton')).not.toBeInTheDocument()
  })

  test('forceMount renders children immediately without an observer', () => {
    const observer = installObserverMock()
    try {
      render(
        <LazySection skeleton={<div data-testid="skeleton" />} forceMount>
          <div data-testid="real" />
        </LazySection>,
      )

      expect(screen.getByTestId('real')).toBeInTheDocument()
      expect(screen.queryByTestId('skeleton')).not.toBeInTheDocument()
      // No observer is installed when we already know the section must render.
      expect(observer.connectedCount()).toBe(0)
    } finally {
      observer.restore()
    }
  })
})
