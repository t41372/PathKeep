/**
 * Tests for PaperContactSheet — the composed Browse view shell.
 *
 * These tests cover the orchestration contract that the Explorer route will
 * lean on: pre-grouped days render with sticky headers, sessions render with
 * session headers, cards vs list switches the inner block rendering, the
 * target banner shows when present, the year rail mounts when supplied, and
 * the view-toggle wires back to its handler.
 */

import { fireEvent, render, screen, within } from '@testing-library/react'
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
  expandStack: 'Toggle stack',
  moreInStack: '+',
  pagesLabel: 'pages',
  empty: 'Nothing here yet.',
  sessionGapLabel: '{duration} away',
}

const NAV_COPY = {
  prev: 'Previous day',
  next: 'Next day',
  today: 'Today',
  openCalendar: 'Open calendar',
}

function makeEntry(overrides: Partial<HistoryEntry>): HistoryEntry {
  return {
    id: 1,
    profileId: 'default',
    url: 'https://example.com',
    title: 'Example',
    domain: 'example.com',
    favicon: null,
    visitedAt: '2026-05-16T20:15:00Z',
    visitTime: new Date('2026-05-16T20:15:00Z').getTime() / 1000,
    durationMs: null,
    transition: null,
    sourceVisitId: 0,
    appId: null,
    ...overrides,
  }
}

function baseDays(): PaperDay[] {
  return [
    {
      date: '2026-05-16',
      visitCount: 5,
      domains: 2,
      sessions: [
        {
          id: 'day1-sess1',
          startMs: new Date('2026-05-16T20:15:00Z').getTime(),
          endMs: new Date('2026-05-16T21:42:00Z').getTime(),
          visitCount: 4,
          blocks: [
            {
              type: 'stack',
              domain: 'github.com',
              entries: [
                makeEntry({
                  id: 11,
                  title: 'tokio',
                  domain: 'github.com',
                  url: 'g1',
                }),
                makeEntry({
                  id: 12,
                  title: 'tokio sched',
                  domain: 'github.com',
                  url: 'g2',
                }),
                makeEntry({
                  id: 13,
                  title: 'tokio issues',
                  domain: 'github.com',
                  url: 'g3',
                }),
              ],
            },
            {
              type: 'single',
              entry: makeEntry({
                id: 14,
                title: 'Attention Is All You Need',
                domain: 'arxiv.org',
                url: 'a1',
              }),
            },
          ],
        },
      ],
    },
    {
      date: '2026-05-15',
      visitCount: 1,
      domains: 1,
      sessions: [
        {
          id: 'day2-sess1',
          startMs: new Date('2026-05-15T09:30:00Z').getTime(),
          endMs: new Date('2026-05-15T09:30:00Z').getTime(),
          visitCount: 1,
          blocks: [
            {
              type: 'single',
              entry: makeEntry({
                id: 21,
                title: 'docs.rs / sqlx',
                domain: 'docs.rs',
                url: 'd1',
                visitedAt: '2026-05-15T09:30:00Z',
                visitTime: new Date('2026-05-15T09:30:00Z').getTime() / 1000,
              }),
            },
          ],
        },
      ],
    },
  ]
}

function makeNav(
  overrides: Partial<PaperContactSheetDayNav> = {},
): PaperContactSheetDayNav {
  return {
    dow: 'FRI',
    monthDay: 'May 16',
    year: '2026',
    densityTier: 3,
    countLabel: '5p',
    relativeAgo: 'yesterday',
    isToday: false,
    onPrev: vi.fn(),
    onNext: vi.fn(),
    onToday: vi.fn(),
    onToggleCal: vi.fn(),
    calOpen: false,
    copy: NAV_COPY,
    ...overrides,
  }
}

describe('PaperContactSheet', () => {
  test('renders every day with header + meta + day-index pill', () => {
    render(
      <PaperContactSheet
        days={baseDays()}
        viewMode="cards"
        onViewModeChange={() => {}}
        dayNav={makeNav()}
        copy={COPY}
        testId="cs"
      />,
    )

    expect(screen.getByText('5 pages · 1 sessions')).toBeVisible()
    expect(screen.getByText('1 pages · 1 sessions')).toBeVisible()
    expect(screen.getByText('Day 2')).toBeVisible()
    expect(screen.getByText('Day 1')).toBeVisible()
  })

  test('cards view renders DomainStack for runs and ContactFrame for singles', () => {
    render(
      <PaperContactSheet
        days={baseDays()}
        viewMode="cards"
        onViewModeChange={() => {}}
        dayNav={makeNav()}
        copy={COPY}
        testId="cs-cards"
      />,
    )

    // Stack header shows the domain
    expect(screen.getByText('github.com')).toBeVisible()
    // Single entry's title from arxiv
    expect(screen.getByText('Attention Is All You Need')).toBeVisible()
  })

  test('list view flattens all blocks into PaperListRow entries', () => {
    render(
      <PaperContactSheet
        days={baseDays()}
        viewMode="list"
        onViewModeChange={() => {}}
        dayNav={makeNav()}
        copy={COPY}
        testId="cs-list"
      />,
    )

    // Three github rows + arxiv + docs.rs = 5 titles render
    expect(screen.getByText('tokio')).toBeVisible()
    expect(screen.getByText('tokio sched')).toBeVisible()
    expect(screen.getByText('Attention Is All You Need')).toBeVisible()
    expect(screen.getByText('docs.rs / sqlx')).toBeVisible()
  })

  test('list view forwards entry.favicon.dataUrl as the row faviconDataUrl', () => {
    const dayWithFavicon: PaperDay = {
      date: '2026-05-16',
      visitCount: 1,
      domains: 1,
      sessions: [
        {
          id: 'fav-session',
          startMs: 0,
          endMs: 0,
          visitCount: 1,
          blocks: [
            {
              type: 'single',
              entry: makeEntry({
                id: 999,
                title: 'A page with a favicon',
                domain: 'icons.test',
                url: 'https://icons.test/page',
                favicon: { dataUrl: 'data:image/png;base64,iVBORw0KG' },
              }),
            },
          ],
        },
      ],
    }

    const { container } = render(
      <PaperContactSheet
        days={[dayWithFavicon]}
        viewMode="list"
        onViewModeChange={() => {}}
        dayNav={makeNav()}
        copy={COPY}
        testId="cs-list-favicon"
      />,
    )

    const icon = container.querySelector<HTMLImageElement>(
      'img[src^="data:image/png;base64"]',
    )
    expect(icon).not.toBeNull()
    expect(icon?.src).toBe('data:image/png;base64,iVBORw0KG')
  })

  test('list view falls back to og:image when favicon is missing, before the swatch', () => {
    const dayWithOg: PaperDay = {
      date: '2026-05-16',
      visitCount: 1,
      domains: 1,
      sessions: [
        {
          id: 'og-session',
          startMs: 0,
          endMs: 0,
          visitCount: 1,
          blocks: [
            {
              type: 'single',
              entry: makeEntry({
                id: 1000,
                title: 'A page with only og:image',
                domain: 'no-favicon.test',
                url: 'https://no-favicon.test/page',
                favicon: null,
                ogImage: { dataUrl: 'data:image/webp;base64,UklGR' },
              }),
            },
          ],
        },
      ],
    }
    const { container } = render(
      <PaperContactSheet
        days={[dayWithOg]}
        viewMode="list"
        onViewModeChange={() => {}}
        dayNav={makeNav()}
        copy={COPY}
        testId="cs-list-og"
      />,
    )
    const icon = container.querySelector<HTMLImageElement>(
      'img[src^="data:image/webp;base64"]',
    )
    expect(icon).not.toBeNull()
    expect(icon?.src).toBe('data:image/webp;base64,UklGR')
  })

  test('list view shows the domain-color swatch only when both favicon and og:image are missing', () => {
    const dayWithSwatch: PaperDay = {
      date: '2026-05-16',
      visitCount: 1,
      domains: 1,
      sessions: [
        {
          id: 'swatch-session',
          startMs: 0,
          endMs: 0,
          visitCount: 1,
          blocks: [
            {
              type: 'single',
              entry: makeEntry({
                id: 1001,
                title: 'A bare page',
                domain: 'no-icons.test',
                url: 'https://no-icons.test/page',
                favicon: null,
                ogImage: null,
              }),
            },
          ],
        },
      ],
    }
    const { container } = render(
      <PaperContactSheet
        days={[dayWithSwatch]}
        viewMode="list"
        onViewModeChange={() => {}}
        dayNav={makeNav()}
        copy={COPY}
        testId="cs-list-swatch"
      />,
    )
    expect(container.querySelector('img[src^="data:image"]')).toBeNull()
    // Swatch span uses the inline background style and the truncated
    // initials block from getDomainAbbr.
    const swatch = container.querySelector<HTMLSpanElement>(
      'span[aria-hidden="true"][style*="background"]',
    )
    expect(swatch).not.toBeNull()
  })

  test('forwards hour12=false through formatTimeFromVisitTime so session times are 24h', () => {
    const dayWith24h: PaperDay = {
      date: '2026-05-16',
      visitCount: 1,
      domains: 1,
      sessions: [
        {
          id: '24h-session',
          startMs: new Date('2026-05-16T13:14:00').getTime(),
          endMs: new Date('2026-05-16T14:01:00').getTime(),
          visitCount: 1,
          blocks: [
            {
              type: 'single',
              entry: makeEntry({
                id: 1002,
                title: 'A page',
                domain: 'example.com',
                url: 'https://example.com/page',
                visitTime: new Date('2026-05-16T13:14:00').getTime() / 1000,
              }),
            },
          ],
        },
      ],
    }
    render(
      <PaperContactSheet
        days={[dayWith24h]}
        viewMode="list"
        onViewModeChange={() => {}}
        dayNav={makeNav()}
        hour12={false}
        copy={COPY}
        testId="cs-list-24h"
      />,
    )
    // Session header reads the 24h range.
    expect(screen.getByText(/13:14.*14:01/)).toBeVisible()
  })

  test('view-toggle reports the new mode through onViewModeChange', () => {
    const onChange = vi.fn()
    render(
      <PaperContactSheet
        days={baseDays()}
        viewMode="cards"
        onViewModeChange={onChange}
        dayNav={makeNav()}
        copy={COPY}
        testId="cs-toggle"
      />,
    )

    fireEvent.click(screen.getByRole('tab', { name: '☰ List' }))
    expect(onChange).toHaveBeenCalledWith('list')
  })

  test('renders the target banner when target is supplied and clears via the button', () => {
    const onClear = vi.fn()
    render(
      <PaperContactSheet
        days={baseDays()}
        viewMode="cards"
        onViewModeChange={() => {}}
        dayNav={makeNav()}
        target={{
          source: 'on-this-day',
          date: '2026-05-16',
          kicker: "From 'On this day'",
          prettyDate: 'Friday, May 16, 2026',
          status: '5 pages archived',
        }}
        onClearTarget={onClear}
        copy={COPY}
        testId="cs-target"
      />,
    )

    expect(screen.getByText("From 'On this day'")).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: 'Clear' }))
    expect(onClear).toHaveBeenCalledTimes(1)
  })

  test('renders the empty state when no days are supplied', () => {
    render(
      <PaperContactSheet
        days={[]}
        viewMode="cards"
        onViewModeChange={() => {}}
        dayNav={makeNav()}
        copy={COPY}
        testId="cs-empty"
      />,
    )

    expect(screen.getByText('Nothing here yet.')).toBeVisible()
  })

  test('swaps the empty copy for a skeleton while loading with no rows yet', () => {
    const { container, rerender } = render(
      <PaperContactSheet
        days={[]}
        viewMode="cards"
        onViewModeChange={() => {}}
        dayNav={makeNav()}
        copy={COPY}
        loading
        testId="cs-loading"
      />,
    )

    expect(
      container.querySelector(
        '[data-testid="paper-contact-sheet-loading-skeleton"]',
      ),
    ).not.toBeNull()
    expect(screen.queryByText('Nothing here yet.')).toBeNull()

    rerender(
      <PaperContactSheet
        days={[]}
        viewMode="cards"
        onViewModeChange={() => {}}
        dayNav={makeNav()}
        copy={COPY}
        loading={false}
        testId="cs-loading"
      />,
    )

    expect(
      container.querySelector(
        '[data-testid="paper-contact-sheet-loading-skeleton"]',
      ),
    ).toBeNull()
    expect(screen.getByText('Nothing here yet.')).toBeVisible()
  })

  test('day nav prev/next/today buttons route through their handlers', () => {
    const onPrev = vi.fn()
    const onNext = vi.fn()
    const onToday = vi.fn()
    render(
      <PaperContactSheet
        days={baseDays()}
        viewMode="cards"
        onViewModeChange={() => {}}
        dayNav={makeNav({ onPrev, onNext, onToday })}
        copy={COPY}
        testId="cs-nav"
      />,
    )

    const nav = screen.getByTestId('paper-contact-sheet-day-nav')
    fireEvent.click(within(nav).getByRole('button', { name: 'Previous day' }))
    fireEvent.click(within(nav).getByRole('button', { name: 'Next day' }))
    fireEvent.click(within(nav).getByRole('button', { name: 'Today' }))
    expect(onPrev).toHaveBeenCalledTimes(1)
    expect(onNext).toHaveBeenCalledTimes(1)
    expect(onToday).toHaveBeenCalledTimes(1)
  })

  test('clicking a single ContactFrame surfaces the real history entry', () => {
    const onSelect = vi.fn()
    render(
      <PaperContactSheet
        days={baseDays()}
        viewMode="cards"
        onViewModeChange={() => {}}
        dayNav={makeNav()}
        onSelectEntry={onSelect}
        copy={COPY}
        testId="cs-single"
      />,
    )

    fireEvent.click(screen.getByText('Attention Is All You Need'))
    expect(onSelect).toHaveBeenCalledTimes(1)
    expect(onSelect.mock.calls[0][0].id).toBe(14)
  })

  test('selecting an entry inside a DomainStack maps back to the real history row', () => {
    const onSelect = vi.fn()
    render(
      <PaperContactSheet
        days={baseDays()}
        viewMode="cards"
        onViewModeChange={() => {}}
        dayNav={makeNav()}
        onSelectEntry={onSelect}
        copy={COPY}
        testId="cs-stack-select"
      />,
    )

    // Click a preview row inside the stack
    fireEvent.click(screen.getByText('tokio'))
    expect(onSelect).toHaveBeenCalledTimes(1)
    expect(onSelect.mock.calls[0][0].id).toBe(11)
  })

  test('fall-back labels render when the locale is a malformed BCP-47 tag', () => {
    // language="-" forces toLocaleTimeString / toLocaleDateString to throw
    // inside the time + day formatting helpers. The component must use the
    // "--:--" / raw-iso fallbacks rather than crashing.
    render(
      <PaperContactSheet
        days={baseDays()}
        viewMode="cards"
        onViewModeChange={() => {}}
        dayNav={makeNav()}
        language="-"
        copy={COPY}
        testId="cs-bad-locale"
      />,
    )
    // The raw-iso day-header fallback surfaces both day strings.
    expect(screen.getByText('2026-05-15')).toBeVisible()
    expect(screen.getByText('2026-05-16')).toBeVisible()
  })

  test('marks the active day header when target.date matches', () => {
    const { container } = render(
      <PaperContactSheet
        days={baseDays()}
        viewMode="cards"
        onViewModeChange={() => {}}
        dayNav={makeNav()}
        target={{
          source: 'search',
          date: '2026-05-15',
          kicker: 'From search',
          prettyDate: 'Thursday, May 15, 2026',
          status: 'Scrolled to record',
        }}
        copy={COPY}
        testId="cs-active-day"
      />,
    )

    const activeHeader = container.querySelector(
      '[data-day="2026-05-15"] [data-active="true"]',
    )
    expect(activeHeader).not.toBeNull()
  })

  test('renders the pagination footer and fires older/newer/pageSize handlers', () => {
    const onPrevious = vi.fn()
    const onNext = vi.fn()
    const onChangePageSize = vi.fn()
    render(
      <PaperContactSheet
        days={baseDays()}
        viewMode="list"
        onViewModeChange={() => {}}
        dayNav={makeNav()}
        copy={COPY}
        pagination={{
          page: 3,
          pageSize: 50,
          total: 1234,
          pageCount: 25,
          hasPrevious: true,
          hasNext: true,
          onPrevious,
          onNext,
          onChangePageSize,
          copy: {
            older: 'Older',
            newer: 'Newer',
            summary: 'Page {page} of {pageCount} · {total} rows',
            summaryPending: 'Loading…',
            pageSizeLabel: 'Rows per page',
          },
        }}
        testId="cs-pagination"
      />,
    )
    expect(screen.getByTestId('paper-contact-sheet-pagination')).toBeVisible()
    expect(screen.getByText(/Page 3 of 25 · 1,234 rows/)).toBeVisible()
    fireEvent.click(screen.getByTestId('paper-contact-sheet-page-prev'))
    expect(onPrevious).toHaveBeenCalled()
    fireEvent.click(screen.getByTestId('paper-contact-sheet-page-next'))
    expect(onNext).toHaveBeenCalled()
    fireEvent.change(screen.getByTestId('paper-contact-sheet-page-size'), {
      target: { value: '100' },
    })
    expect(onChangePageSize).toHaveBeenCalledWith(100)
  })

  test('infinite-scroll footer renders sentinel + summary when canLoadMore', () => {
    const onLoadMore = vi.fn()
    render(
      <PaperContactSheet
        days={baseDays()}
        viewMode="cards"
        onViewModeChange={() => {}}
        dayNav={makeNav()}
        copy={COPY}
        infiniteScroll={{
          loadingMore: false,
          canLoadMore: true,
          onLoadMore,
          loadedPageCount: 2,
          totalPages: 8,
          totalRows: 416,
          copy: {
            loadingMore: 'Loading earlier days…',
            endOfArchive: 'End of archive',
            loadedSummary:
              'Loaded {loaded} of {total} pages · {rows} rows in view',
          },
        }}
        testId="cs-infinite"
      />,
    )
    expect(
      screen.getByTestId('paper-contact-sheet-infinite-footer'),
    ).toBeVisible()
    expect(
      screen.getByTestId('paper-contact-sheet-infinite-sentinel'),
    ).toBeInTheDocument()
    expect(
      screen.getByText(/Loaded 2 of 8 pages · 416 rows in view/),
    ).toBeVisible()
  })

  test('infinite-scroll footer renders the lazy skeleton while loading more', () => {
    render(
      <PaperContactSheet
        days={baseDays()}
        viewMode="cards"
        onViewModeChange={() => {}}
        dayNav={makeNav()}
        copy={COPY}
        infiniteScroll={{
          loadingMore: true,
          canLoadMore: true,
          onLoadMore: () => {},
          loadedPageCount: 1,
          totalPages: 4,
          totalRows: 200,
          copy: {
            loadingMore: 'Loading earlier days…',
            endOfArchive: 'End of archive',
            loadedSummary:
              'Loaded {loaded} of {total} pages · {rows} rows in view',
          },
        }}
        testId="cs-infinite-loading"
      />,
    )
    expect(
      screen.getByTestId('paper-contact-sheet-infinite-skeleton'),
    ).toBeVisible()
    expect(screen.getByText('Loading earlier days…')).toBeVisible()
    // Sentinel hidden while a load is inflight to prevent re-triggering.
    expect(
      screen.queryByTestId('paper-contact-sheet-infinite-sentinel'),
    ).toBeNull()
  })

  test('infinite-scroll footer renders end-of-archive caption when canLoadMore is false', () => {
    render(
      <PaperContactSheet
        days={baseDays()}
        viewMode="cards"
        onViewModeChange={() => {}}
        dayNav={makeNav()}
        copy={COPY}
        infiniteScroll={{
          loadingMore: false,
          canLoadMore: false,
          onLoadMore: () => {},
          loadedPageCount: 4,
          totalPages: 4,
          totalRows: 200,
          copy: {
            loadingMore: 'Loading earlier days…',
            endOfArchive: 'You’ve reached the start of the archive.',
            loadedSummary:
              'Loaded {loaded} of {total} pages · {rows} rows in view',
          },
        }}
        testId="cs-infinite-end"
      />,
    )
    expect(
      screen.getByText('You’ve reached the start of the archive.'),
    ).toBeVisible()
    expect(
      screen.queryByTestId('paper-contact-sheet-infinite-sentinel'),
    ).toBeNull()
  })

  test('pagination footer renders pending summary when pageCount is 0', () => {
    render(
      <PaperContactSheet
        days={baseDays()}
        viewMode="list"
        onViewModeChange={() => {}}
        dayNav={makeNav()}
        copy={COPY}
        pagination={{
          page: null,
          pageSize: 25,
          total: 0,
          pageCount: 0,
          hasPrevious: false,
          hasNext: false,
          onPrevious: () => {},
          onNext: () => {},
          copy: {
            older: 'Older',
            newer: 'Newer',
            summary: 'Page {page} of {pageCount} · {total} rows',
            summaryPending: 'Loading more pages…',
            pageSizeLabel: 'Rows per page',
          },
        }}
        testId="cs-pagination-pending"
      />,
    )
    expect(screen.getByText('Loading more pages…')).toBeVisible()
    // pageSize selector omitted when onChangePageSize is undefined.
    expect(screen.queryByTestId('paper-contact-sheet-page-size')).toBeNull()
  })

  test('renders a session-gap indicator between two same-day sessions and skips it for the first', () => {
    // Two sessions on the same day. Session A (newer) ran 21:00 → 22:00,
    // session B (older) ran 18:00 → 18:30, so the gap is 2.5 hours of no
    // activity. The component must render the gap above session B (the
    // older one) and not above session A.
    const sessionA: PaperDay['sessions'][number] = {
      id: 'a',
      startMs: new Date('2026-05-16T21:00:00Z').getTime(),
      endMs: new Date('2026-05-16T22:00:00Z').getTime(),
      visitCount: 1,
      blocks: [
        {
          type: 'single',
          entry: makeEntry({ id: 91, title: 'A', url: 'a' }),
        },
      ],
    }
    const sessionB: PaperDay['sessions'][number] = {
      id: 'b',
      startMs: new Date('2026-05-16T18:00:00Z').getTime(),
      endMs: new Date('2026-05-16T18:30:00Z').getTime(),
      visitCount: 1,
      blocks: [
        {
          type: 'single',
          entry: makeEntry({ id: 92, title: 'B', url: 'b' }),
        },
      ],
    }
    const days: PaperDay[] = [
      {
        date: '2026-05-16',
        visitCount: 2,
        domains: 1,
        sessions: [sessionA, sessionB],
      },
    ]
    render(<PaperContactSheet days={days} copy={COPY} dayNav={makeNav()} />)
    // The gap label is rendered above sessionB only.
    expect(
      screen.getByTestId('paper-session-gap-2026-05-16-1'),
    ).toBeInTheDocument()
    expect(
      screen.queryByTestId('paper-session-gap-2026-05-16-0'),
    ).not.toBeInTheDocument()
    // Label substitutes the formatted duration into the template.
    expect(screen.getByText(/away$/)).toBeInTheDocument()
  })
})
