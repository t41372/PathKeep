/**
 * Focused behavior tests for PaperContactSheet.
 *
 * These pin branches that are easy to cover accidentally but hard to trust
 * without outcome assertions: cross-day card numbering, invalid timestamp/date
 * fallbacks, target-banner no-op safety, and infinite-scroll footer states.
 */

import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
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

function makeEntry(id: number, overrides: Partial<HistoryEntry> = {}) {
  const visitedMs = new Date('2026-05-16T20:15:00Z').getTime() - id * 60_000
  return {
    id,
    profileId: 'behavior:Default',
    url: `https://example.test/${id}`,
    title: `Entry ${id}`,
    domain: 'example.test',
    favicon: null,
    visitedAt: new Date(visitedMs).toISOString(),
    visitTime: visitedMs,
    durationMs: null,
    transition: null,
    sourceVisitId: 0,
    appId: null,
    ...overrides,
  } satisfies HistoryEntry
}

function makeDay(
  date: string,
  ids: number[],
  overrides: Partial<PaperDay> = {},
): PaperDay {
  const startMs = new Date('2026-05-16T20:00:00Z').getTime()
  return {
    date,
    visitCount: ids.length,
    domains: 1,
    sessions: [
      {
        id: `${date}-session`,
        startMs,
        endMs: startMs + ids.length * 60_000,
        visitCount: ids.length,
        blocks: ids.map((id) => ({
          type: 'single' as const,
          entry: makeEntry(id),
        })),
      },
    ],
    ...overrides,
  }
}

function renderSheet(
  props: Partial<Parameters<typeof PaperContactSheet>[0]> = {},
) {
  return render(
    <PaperContactSheet
      days={[makeDay('2026-05-16', [11, 12]), makeDay('2026-05-15', [21])]}
      viewMode="cards"
      onViewModeChange={() => {}}
      dayNav={NAV}
      copy={COPY}
      {...props}
    />,
  )
}

describe('PaperContactSheet focused behavior', () => {
  test('card frame numbers continue across day boundaries', () => {
    const { container } = renderSheet({
      days: [
        makeDay('2026-05-16', [11, 12, 13]),
        makeDay('2026-05-15', [21, 22]),
      ],
      viewMode: 'cards',
    })

    const lastFirstDay = container.querySelector('[data-entry-id="13"]')
    const firstSecondDay = container.querySelector('[data-entry-id="21"]')
    const lastSecondDay = container.querySelector('[data-entry-id="22"]')

    expect(lastFirstDay).not.toBeNull()
    expect(firstSecondDay).not.toBeNull()
    expect(lastSecondDay).not.toBeNull()
    expect(lastFirstDay).toHaveTextContent('03')
    expect(firstSecondDay).toHaveTextContent('04')
    expect(lastSecondDay).toHaveTextContent('05')
  })

  test('day headers use the measured sticky toolbar height when filters wrap the toolbar', async () => {
    const disconnect = vi.fn()
    class FakeResizeObserver {
      callback: ResizeObserverCallback
      constructor(callback: ResizeObserverCallback) {
        this.callback = callback
      }
      observe(target: Element) {
        this.callback(
          [] as unknown as ResizeObserverEntry[],
          this as unknown as ResizeObserver,
        )
        expect(target).toHaveTextContent('Filter chips')
      }
      disconnect() {
        disconnect()
      }
      unobserve() {}
    }
    const previousResizeObserver = globalThis.ResizeObserver
    const restoreBoundingClientRect = installFixedBoundingClientRect(88)
    ;(globalThis as { ResizeObserver: typeof ResizeObserver }).ResizeObserver =
      FakeResizeObserver as unknown as typeof ResizeObserver
    try {
      const { container, unmount } = renderSheet({
        filterStripSlot: <span>Filter chips</span>,
      })

      await waitFor(() =>
        expect(
          container.querySelector('[data-day="2026-05-16"] > div'),
        ).toHaveStyle({ top: '88px' }),
      )
      unmount()
      expect(disconnect).toHaveBeenCalled()
    } finally {
      ;(
        globalThis as { ResizeObserver: typeof ResizeObserver }
      ).ResizeObserver = previousResizeObserver
      restoreBoundingClientRect()
    }
  })

  test('day headers still use the first toolbar measurement without ResizeObserver support', async () => {
    const previousResizeObserver = globalThis.ResizeObserver
    const restoreBoundingClientRect = installFixedBoundingClientRect(64)
    ;(
      globalThis as { ResizeObserver: typeof ResizeObserver | undefined }
    ).ResizeObserver = undefined
    try {
      const { container } = renderSheet({
        filterStripSlot: <span>Filter chips</span>,
      })

      await waitFor(() =>
        expect(
          container.querySelector('[data-day="2026-05-16"] > div'),
        ).toHaveStyle({ top: '64px' }),
      )
    } finally {
      ;(
        globalThis as { ResizeObserver: typeof ResizeObserver | undefined }
      ).ResizeObserver = previousResizeObserver
      restoreBoundingClientRect()
    }
  })

  test('list rows show a neutral time fallback for invalid visit timestamps', () => {
    const { container } = renderSheet({
      days: [
        makeDay('2026-05-16', [99], {
          sessions: [
            {
              id: 'invalid-time-session',
              startMs: new Date('2026-05-16T20:00:00Z').getTime(),
              endMs: new Date('2026-05-16T20:01:00Z').getTime(),
              visitCount: 1,
              blocks: [
                {
                  type: 'single',
                  entry: makeEntry(99, {
                    title: 'Broken timestamp row',
                    visitTime: Number.NaN,
                  }),
                },
              ],
            },
          ],
        }),
      ],
      viewMode: 'list',
    })

    const row = container.querySelector('[data-entry-id="99"]')
    expect(row).not.toBeNull()
    expect(within(row as HTMLElement).getByText('--:--')).toBeVisible()
  })

  test('malformed and unrepresentable day keys render raw labels instead of crashing', () => {
    renderSheet({
      days: [makeDay('2026-05', [1]), makeDay('300000-01-01', [2])],
      viewMode: 'list',
    })

    expect(screen.getByText('2026-05')).toBeVisible()
    expect(screen.getByText('300000-01-01')).toBeVisible()
  })

  test('null titles fall back to URL text in card and list modes', () => {
    const untitledDay = makeDay('2026-05-16', [77], {
      sessions: [
        {
          id: 'untitled-session',
          startMs: new Date('2026-05-16T20:00:00Z').getTime(),
          endMs: new Date('2026-05-16T20:01:00Z').getTime(),
          visitCount: 1,
          blocks: [
            {
              type: 'single',
              entry: makeEntry(77, {
                title: null,
                url: 'https://untitled.example.test/page',
              }),
            },
          ],
        },
      ],
    })
    const { container, rerender } = renderSheet({
      days: [untitledDay],
      viewMode: 'cards',
    })

    expect(container.querySelector('[data-entry-id="77"]')).toHaveTextContent(
      'untitled.example.test/page',
    )

    rerender(
      <PaperContactSheet
        days={[untitledDay]}
        viewMode="list"
        onViewModeChange={() => {}}
        dayNav={NAV}
        copy={COPY}
      />,
    )
    expect(container.querySelector('[data-entry-id="77"]')).toHaveTextContent(
      'untitled.example.test/page',
    )
  })

  test('target clear button is safe when the caller omits a clear handler', () => {
    renderSheet({
      target: {
        source: 'search',
        date: '2026-05-16',
        kicker: 'From search',
        prettyDate: 'Saturday, May 16, 2026',
        status: '3 pages archived',
      },
    })

    fireEvent.click(screen.getByRole('button', { name: 'Clear' }))
    expect(
      screen.getByTestId('paper-contact-sheet-target-banner'),
    ).toBeVisible()
  })

  test('infinite footer shows the cap-reached guidance instead of end-of-archive copy', () => {
    renderSheet({
      infiniteScroll: {
        loadingMore: false,
        canLoadMore: false,
        capReached: true,
        onLoadMore: () => {},
        loadedPageCount: 100,
        totalPages: 400,
        totalRows: 5000,
        copy: {
          loadingMore: 'Loading',
          endOfArchive: 'End of archive',
          loadedSummary: '{loaded}/{total}',
          capReached: 'Showing {loaded} rows. Jump by date to go deeper.',
        },
      },
    })

    expect(
      screen.getByText('Showing 5,000 rows. Jump by date to go deeper.'),
    ).toBeVisible()
    expect(screen.queryByText('End of archive')).toBeNull()
  })

  test('infinite observer ignores sentinel entries outside the viewport', () => {
    type ObserverFn = (entries: { isIntersecting: boolean }[]) => void
    class FakeObserver {
      callback: ObserverFn
      constructor(callback: ObserverFn) {
        this.callback = callback
      }
      observe() {
        this.callback([{ isIntersecting: false }])
      }
      disconnect() {}
    }
    const previous = globalThis.IntersectionObserver
    ;(
      globalThis as { IntersectionObserver: typeof IntersectionObserver }
    ).IntersectionObserver =
      FakeObserver as unknown as typeof IntersectionObserver
    const onLoadMore = vi.fn()
    try {
      renderSheet({
        infiniteScroll: {
          loadingMore: false,
          canLoadMore: true,
          onLoadMore,
          loadedPageCount: 1,
          totalPages: 2,
          totalRows: 25,
          copy: {
            loadingMore: 'Loading',
            endOfArchive: 'End of archive',
            loadedSummary: '{loaded}/{total}',
          },
        },
      })
      expect(
        screen.getByTestId('paper-contact-sheet-infinite-sentinel'),
      ).toBeInTheDocument()
      expect(onLoadMore).not.toHaveBeenCalled()
    } finally {
      ;(
        globalThis as { IntersectionObserver: typeof IntersectionObserver }
      ).IntersectionObserver = previous
    }
  })

  test('infinite footer surfaces page-load errors with the supplied message', () => {
    renderSheet({
      infiniteScroll: {
        loadingMore: false,
        canLoadMore: false,
        error: 'network timeout',
        onLoadMore: () => {},
        loadedPageCount: 1,
        totalPages: 2,
        totalRows: 25,
        copy: {
          loadingMore: 'Loading',
          endOfArchive: 'End of archive',
          loadedSummary: '{loaded}/{total}',
          error: 'Could not load more pages: {message}',
        },
      },
    })

    expect(screen.getByRole('alert')).toHaveTextContent(
      'Could not load more pages: network timeout',
    )
  })
})

function installFixedBoundingClientRect(height: number) {
  const descriptor = Object.getOwnPropertyDescriptor(
    HTMLElement.prototype,
    'getBoundingClientRect',
  )
  Object.defineProperty(HTMLElement.prototype, 'getBoundingClientRect', {
    configurable: true,
    value: () =>
      ({
        height,
        width: 0,
        top: 0,
        left: 0,
        bottom: height,
        right: 0,
        x: 0,
        y: 0,
        toJSON: () => ({}),
      }) as DOMRect,
  })
  return () => {
    if (descriptor) {
      Object.defineProperty(
        HTMLElement.prototype,
        'getBoundingClientRect',
        descriptor,
      )
    }
  }
}
