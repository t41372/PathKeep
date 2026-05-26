/**
 * Tests that PaperContactSheet's viewport-driven mounting actually
 * unmounts off-screen day content when the IntersectionObserver
 * reports them as out-of-view, and remounts them when they scroll
 * back in. jsdom doesn't fire IntersectionObserver entries
 * automatically, so we install a controllable mock and step through
 * the lifecycle manually.
 */

import { act, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, test } from 'vitest'
import type { HistoryEntry } from '@/lib/types/archive'
import type { PaperDay } from '@/pages/explorer/paper/group-entries'
import {
  PaperContactSheet,
  type PaperContactSheetCopy,
  type PaperContactSheetDayNav,
} from './paper-contact-sheet'

const COPY: PaperContactSheetCopy = {
  view: 'View',
  cards: 'Cards',
  list: 'List',
  dayMeta: '{count} pages · {sessions} sessions',
  dayIndex: 'Day {n}',
  clearTarget: 'Clear',
  pagesLabel: 'pages',
  empty: 'Nothing here yet.',
  sessionGapLabel: '{duration} away',
}

const NAV: PaperContactSheetDayNav = {
  dow: 'FRI',
  monthDay: 'May 16',
  year: '2026',
  densityTier: 3,
  countLabel: '5p',
  relativeAgo: 'yesterday',
  isToday: false,
  onPrev: () => {},
  onNext: () => {},
  onToday: () => {},
  onToggleCal: () => {},
  calOpen: false,
  copy: {
    prev: 'Previous day',
    next: 'Next day',
    today: 'Today',
    openCalendar: 'Open calendar',
  },
}

function makeEntry(id: number, overrides: Partial<HistoryEntry> = {}): HistoryEntry {
  return {
    id,
    profileId: 'virt-test',
    url: `https://example.com/${id}`,
    title: `Entry ${id}`,
    domain: 'example.com',
    favicon: null,
    visitedAt: '2026-05-16T20:15:00Z',
    visitTime: new Date('2026-05-16T20:15:00Z').getTime(),
    durationMs: null,
    transition: null,
    sourceVisitId: 0,
    appId: null,
    ...overrides,
  }
}

function makeDay(date: string, rowCount: number): PaperDay {
  const sessionStart = new Date(`${date}T20:00:00Z`).getTime()
  return {
    date,
    visitCount: rowCount,
    domains: 1,
    sessions: [
      {
        id: `${date}-sess`,
        startMs: sessionStart,
        endMs: sessionStart + rowCount * 60_000,
        visitCount: rowCount,
        blocks: Array.from({ length: rowCount }, (_, i) => ({
          type: 'single' as const,
          entry: makeEntry(Number(`${date.replace(/-/g, '')}${i}`)),
        })),
      },
    ],
  }
}

/**
 * Controllable IntersectionObserver mock. Records every observed
 * node and exposes `trigger(node, isIntersecting)` so tests can
 * simulate viewport entry / exit. Mounted on `window` /
 * `globalThis` for the duration of each test.
 */
function installObserverMock() {
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
    trigger(node: Element, isIntersecting: boolean) {
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

describe('PaperContactSheet — viewport-driven mounting', () => {
  let io: ReturnType<typeof installObserverMock>

  beforeEach(() => {
    io = installObserverMock()
  })
  afterEach(() => {
    io.restore()
  })

  test('day content unmounts when IntersectionObserver reports out-of-view, remounts on re-entry', () => {
    const days: PaperDay[] = [
      makeDay('2026-05-16', 2),
      makeDay('2026-05-15', 2),
      makeDay('2026-05-14', 2),
    ]

    render(
      <PaperContactSheet
        days={days}
        viewMode="list"
        onViewModeChange={() => {}}
        dayNav={NAV}
        copy={COPY}
      />,
    )

    // Every day starts mounted (initialInView=true) so the first
    // paint of the contact sheet doesn't flash empty placeholders.
    expect(screen.getAllByText(/Entry/)).toHaveLength(6)

    // Find the wrapper div for the bottom day and report it as
    // off-screen. Its entry content should disappear, but the
    // wrapper itself stays in the DOM with a placeholder min-height.
    const bottomDay = document.querySelector('[data-day="2026-05-14"]')
    if (!(bottomDay instanceof HTMLElement)) {
      throw new Error('bottom day wrapper missing')
    }
    // Pretend the bottom day rendered a 600px-tall block before we
    // recycled it. jsdom returns 0 for getBoundingClientRect by
    // default, so we stub it for this assertion.
    bottomDay.getBoundingClientRect = () =>
      ({ height: 600, width: 0, top: 0, left: 0, bottom: 600, right: 0, x: 0, y: 0, toJSON: () => ({}) }) as DOMRect

    act(() => {
      io.trigger(bottomDay, false)
    })

    // Bottom day's entries are unmounted; the wrapper now carries a
    // placeholder min-height and the data-virt-state attribute flips
    // to "recycled" so it's observable in tests.
    expect(bottomDay.getAttribute('data-virt-state')).toBe('recycled')
    expect(bottomDay.style.minHeight).toBe('600px')
    // The two visible days still have their 2 entries each (4 total).
    expect(screen.getAllByText(/Entry/)).toHaveLength(4)

    // Re-entering re-mounts the content.
    act(() => {
      io.trigger(bottomDay, true)
    })
    expect(bottomDay.getAttribute('data-virt-state')).toBe('mounted')
    expect(screen.getAllByText(/Entry/)).toHaveLength(6)
  })

  test('disableVirtualization keeps every day mounted regardless of IO callbacks', () => {
    const days: PaperDay[] = [
      makeDay('2026-05-16', 1),
      makeDay('2026-05-15', 1),
    ]

    render(
      <PaperContactSheet
        days={days}
        viewMode="list"
        onViewModeChange={() => {}}
        dayNav={NAV}
        copy={COPY}
        disableVirtualization
      />,
    )

    const bottomDay = document.querySelector('[data-day="2026-05-15"]')
    if (!(bottomDay instanceof HTMLElement)) {
      throw new Error('bottom day wrapper missing')
    }

    // Even when the observer reports the day as off-screen, the
    // disable flag keeps the content mounted — the spike harness
    // depends on this so it can measure the un-recycled DOM cost.
    act(() => {
      io.trigger(bottomDay, false)
    })
    expect(bottomDay.getAttribute('data-virt-state')).toBe('mounted')
    expect(screen.getAllByText(/Entry/)).toHaveLength(2)
  })
})
