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

  test('mounts the year rail when supplied and routes jumps to its handler', () => {
    const onJump = vi.fn()
    render(
      <PaperContactSheet
        days={baseDays()}
        viewMode="cards"
        onViewModeChange={() => {}}
        dayNav={makeNav()}
        yearRail={{
          densityByYear: new Map([[2026, 100_000]]),
          bounds: { firstYear: 2024, lastYear: 2026, lastIso: '2026-05-17' },
          currentDate: '2026-05-16',
          onJump,
        }}
        copy={COPY}
        testId="cs-rail"
      />,
    )

    const rail = screen.getByTestId('paper-contact-sheet-year-rail')
    fireEvent.click(rail.querySelector('[data-year="2024"]') as HTMLElement)
    expect(onJump).toHaveBeenCalledWith('2024-06-15')
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
})
