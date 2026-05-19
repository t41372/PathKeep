/**
 * Tests for the PaperExplorerView orchestration layer.
 *
 * The view's contract: given a stream of HistoryEntry rows + optional
 * archive bounds + target date/source, produce the correct day-grouping,
 * density maps, day-nav state, and target banner — wired through
 * PaperContactSheet.
 */

import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import type { HistoryEntry } from '@/lib/types/archive'
import { PaperExplorerView, type PaperExplorerCopy } from './paper-view'
import {
  buildPerDayDensity,
  buildPerYearDensity,
  inferBounds,
  pickInitialDate,
} from './paper-view-helpers'
import { groupEntriesByDay } from './paper/group-entries'

const COPY: PaperExplorerCopy = {
  contactSheet: {
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
  },
  dayNav: {
    prev: 'Previous day',
    next: 'Next day',
    today: 'Today',
    openCalendar: 'Open calendar',
  },
  relative: {
    today: 'today',
    yesterday: 'yesterday',
    daysAgo: '{count}d ago',
    weeksAgo: '{count}w ago',
    monthsAgo: '{count}mo ago',
    yearsAgo: '{count}y ago',
  },
  calendar: {
    prevMonth: 'Previous month',
    nextMonth: 'Next month',
    months: [
      'January',
      'February',
      'March',
      'April',
      'May',
      'June',
      'July',
      'August',
      'September',
      'October',
      'November',
      'December',
    ],
    dowLabels: ['M', 'T', 'W', 'T', 'F', 'S', 'S'],
    today: 'Today',
    oneYearAgo: '1 year ago',
    pagesArchived: '{count} pages archived',
    monthSummary: '{active} active days · {total} pages',
    boundsMeta: '{firstYear}–{lastYear} · {totalDays} days',
  },
  yearRailTitle: '{year} · {count} pages',
}

function makeEntry(overrides: Partial<HistoryEntry>): HistoryEntry {
  // Use bare ISO timestamps (no Z) so they parse as local time, keeping
  // the day buckets stable across whatever timezone the test runner uses.
  return {
    id: 1,
    profileId: 'default',
    url: 'https://example.com',
    title: 'Example',
    domain: 'example.com',
    favicon: null,
    visitedAt: '2026-05-16T14:15:00',
    visitTime: new Date('2026-05-16T14:15:00').getTime() / 1000,
    durationMs: null,
    transition: null,
    sourceVisitId: 0,
    appId: null,
    ...overrides,
  }
}

function sampleEntries(): HistoryEntry[] {
  return [
    makeEntry({
      id: 1,
      title: 'tokio',
      domain: 'github.com',
      url: 'https://github.com/tokio-rs/tokio',
      visitedAt: '2026-05-16T14:15:00',
      visitTime: new Date('2026-05-16T14:15:00').getTime() / 1000,
    }),
    makeEntry({
      id: 2,
      title: 'arxiv paper',
      domain: 'arxiv.org',
      url: 'https://arxiv.org/abs/1706.03762',
      visitedAt: '2026-05-16T14:32:00',
      visitTime: new Date('2026-05-16T14:32:00').getTime() / 1000,
    }),
    makeEntry({
      id: 3,
      title: 'docs.rs / sqlx',
      domain: 'docs.rs',
      url: 'https://docs.rs/sqlx',
      visitedAt: '2026-05-15T09:30:00',
      visitTime: new Date('2026-05-15T09:30:00').getTime() / 1000,
    }),
  ]
}

describe('PaperExplorerView', () => {
  test('renders the empty state when entries[] is empty', () => {
    render(
      <PaperExplorerView
        entries={[]}
        copy={COPY}
        todayIso="2026-05-17"
        testId="px"
      />,
    )

    expect(screen.getByText('Nothing here yet.')).toBeVisible()
  })

  test('groups entries by day and renders sticky day headers', () => {
    render(
      <PaperExplorerView
        entries={sampleEntries()}
        copy={COPY}
        todayIso="2026-05-17"
        testId="px-days"
      />,
    )

    // Two days surfaced — 16th has 2 pages / 1 session, 15th has 1 / 1.
    expect(screen.getByText('2 pages · 1 sessions')).toBeVisible()
    expect(screen.getByText('1 pages · 1 sessions')).toBeVisible()
    expect(screen.getByText('Day 2')).toBeVisible()
    expect(screen.getByText('Day 1')).toBeVisible()
  })

  test('day-nav pill reflects the most recent loaded day by default', () => {
    render(
      <PaperExplorerView
        entries={sampleEntries()}
        copy={COPY}
        todayIso="2026-05-17"
        testId="px-pill"
      />,
    )

    const nav = screen.getByTestId('paper-contact-sheet-day-nav')
    // SAT · May 16 · 2026 · 2p · yesterday  (May 16 2026 is a Saturday)
    expect(within(nav).getByText('SAT')).toBeVisible()
    expect(within(nav).getByText('May 16')).toBeVisible()
    expect(within(nav).getByText('2026')).toBeVisible()
    expect(within(nav).getByText('2p')).toBeVisible()
    expect(within(nav).getByText('yesterday')).toBeVisible()
  })

  test('clicking Previous day walks back inside the loaded archive bounds', () => {
    const onJump = vi.fn()
    render(
      <PaperExplorerView
        entries={sampleEntries()}
        copy={COPY}
        todayIso="2026-05-17"
        onJumpToDate={onJump}
        testId="px-prev"
      />,
    )

    const nav = screen.getByTestId('paper-contact-sheet-day-nav')
    fireEvent.click(within(nav).getByRole('button', { name: 'Previous day' }))
    expect(onJump).toHaveBeenCalledWith('2026-05-15')
  })

  test('Previous day clamps at the first archive day', () => {
    const onJump = vi.fn()
    render(
      <PaperExplorerView
        entries={sampleEntries()}
        archiveBounds={{
          firstIso: '2026-05-15',
          lastIso: '2026-05-16',
          firstYear: 2026,
          lastYear: 2026,
          totalDays: 2,
        }}
        copy={COPY}
        todayIso="2026-05-17"
        onJumpToDate={onJump}
        testId="px-prev-clamp"
      />,
    )

    const nav = screen.getByTestId('paper-contact-sheet-day-nav')
    // Step from 5/16 back to 5/15 — at the floor.
    fireEvent.click(within(nav).getByRole('button', { name: 'Previous day' }))
    expect(onJump).toHaveBeenCalledWith('2026-05-15')
    // Now 5/15. Prev should be disabled at the floor.
    const prevButton = within(nav).getByRole<HTMLButtonElement>('button', {
      name: 'Previous day',
    })
    expect(prevButton.disabled).toBe(true)
  })

  test('Today button jumps to the configured todayIso anchor', () => {
    const onJump = vi.fn()
    render(
      <PaperExplorerView
        entries={sampleEntries()}
        archiveBounds={{
          firstIso: '2026-05-15',
          lastIso: '2026-05-17',
          firstYear: 2026,
          lastYear: 2026,
          totalDays: 3,
        }}
        copy={COPY}
        todayIso="2026-05-17"
        onJumpToDate={onJump}
        testId="px-today"
      />,
    )

    const nav = screen.getByTestId('paper-contact-sheet-day-nav')
    fireEvent.click(within(nav).getByRole('button', { name: 'Today' }))
    expect(onJump).toHaveBeenCalledWith('2026-05-17')
  })

  test('opening the calendar surfaces the popover with the current month', () => {
    render(
      <PaperExplorerView
        entries={sampleEntries()}
        copy={COPY}
        todayIso="2026-05-17"
        testId="px-cal"
      />,
    )

    const nav = screen.getByTestId('paper-contact-sheet-day-nav')
    fireEvent.click(within(nav).getByRole('button', { name: 'Open calendar' }))
    expect(screen.getByTestId('paper-explorer-calendar')).toBeVisible()
    expect(screen.getByText('May')).toBeVisible()
  })

  test('Escape closes the calendar', () => {
    render(
      <PaperExplorerView
        entries={sampleEntries()}
        copy={COPY}
        todayIso="2026-05-17"
        testId="px-esc"
      />,
    )

    const nav = screen.getByTestId('paper-contact-sheet-day-nav')
    fireEvent.click(within(nav).getByRole('button', { name: 'Open calendar' }))
    expect(screen.getByTestId('paper-explorer-calendar')).toBeVisible()
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(screen.queryByTestId('paper-explorer-calendar')).toBeNull()
  })

  test('renders the target banner when targetDate is supplied', () => {
    render(
      <PaperExplorerView
        entries={sampleEntries()}
        targetDate="2026-05-16"
        targetSource="search"
        targetQuery="rust"
        copy={COPY}
        todayIso="2026-05-17"
        testId="px-target"
      />,
    )

    expect(screen.getByText('From search · "rust"')).toBeVisible()
    expect(screen.getByText('2 pages archived')).toBeVisible()
  })

  test('target banner falls back to a generic kicker when source is null', () => {
    render(
      <PaperExplorerView
        entries={sampleEntries()}
        targetDate="2026-05-16"
        copy={COPY}
        todayIso="2026-05-17"
        testId="px-fallback-source"
      />,
    )

    expect(screen.getByText("From 'On this day'")).toBeVisible()
  })

  test('target banner status notes the missing day when archive has no record', () => {
    render(
      <PaperExplorerView
        entries={sampleEntries()}
        targetDate="2026-05-01"
        targetSource="on-this-day"
        copy={COPY}
        todayIso="2026-05-17"
        testId="px-missing-target"
      />,
    )

    expect(screen.getByText('No archive for this exact day yet')).toBeVisible()
  })

  test('clicking a year on the rail jumps via onJumpToDate', () => {
    const onJump = vi.fn()
    render(
      <PaperExplorerView
        entries={sampleEntries()}
        archiveBounds={{
          firstIso: '2024-01-01',
          lastIso: '2026-05-16',
          firstYear: 2024,
          lastYear: 2026,
          totalDays: 900,
        }}
        copy={COPY}
        todayIso="2026-05-17"
        onJumpToDate={onJump}
        testId="px-rail"
      />,
    )

    const rail = screen.getByTestId('paper-contact-sheet-year-rail')
    fireEvent.click(rail.querySelector('[data-year="2024"]') as HTMLElement)
    expect(onJump).toHaveBeenCalledWith('2024-06-15')
  })

  test('selecting an entry surfaces the canonical HistoryEntry', () => {
    const onSelect = vi.fn()
    render(
      <PaperExplorerView
        entries={sampleEntries()}
        copy={COPY}
        todayIso="2026-05-17"
        onSelectEntry={onSelect}
        testId="px-select"
      />,
    )

    fireEvent.click(screen.getByText('docs.rs / sqlx'))
    expect(onSelect).toHaveBeenCalledTimes(1)
    expect(onSelect.mock.calls[0][0].id).toBe(3)
  })

  test('cards / list toggle changes the inner layout without losing data', () => {
    render(
      <PaperExplorerView
        entries={sampleEntries()}
        copy={COPY}
        todayIso="2026-05-17"
        initialViewMode="cards"
        testId="px-toggle"
      />,
    )

    fireEvent.click(screen.getByRole('tab', { name: '☰ List' }))
    expect(screen.getByText('docs.rs / sqlx')).toBeVisible()
    expect(screen.getByText('arxiv paper')).toBeVisible()
  })

  test('respects an external targetDate change and rebuilds the banner', () => {
    const { rerender } = render(
      <PaperExplorerView
        entries={sampleEntries()}
        targetDate="2026-05-16"
        targetSource="search"
        copy={COPY}
        todayIso="2026-05-17"
        testId="px-target-change"
      />,
    )

    expect(screen.getByText('From search')).toBeVisible()

    rerender(
      <PaperExplorerView
        entries={sampleEntries()}
        targetDate="2026-05-15"
        targetSource="intelligence"
        copy={COPY}
        todayIso="2026-05-17"
        testId="px-target-change"
      />,
    )

    expect(screen.getByText('From intelligence')).toBeVisible()
  })
})

describe('helpers', () => {
  test('buildPerDayDensity merges per-day visit counts with overrides', () => {
    const days = groupEntriesByDay(sampleEntries())
    const overrides = new Map<string, number>([
      ['2026-05-16', 500], // override wins over loaded 2
      ['2026-01-01', 99], // unloaded day gets the override
    ])
    const map = buildPerDayDensity(days, overrides)
    expect(map.get('2026-05-16')).toBe(500)
    expect(map.get('2026-05-15')).toBe(1)
    expect(map.get('2026-01-01')).toBe(99)
  })

  test('buildPerDayDensity returns loaded counts when no overrides are supplied', () => {
    const days = groupEntriesByDay(sampleEntries())
    const map = buildPerDayDensity(days, undefined)
    expect(map.get('2026-05-16')).toBe(2)
    expect(map.get('2026-05-15')).toBe(1)
  })

  test('buildPerYearDensity aggregates loaded days and merges overrides', () => {
    const days = groupEntriesByDay(sampleEntries())
    const overrides = new Map<number, number>([
      [2026, 100_000], // override beats the loaded 3 pages
      [1990, 42],
    ])
    const map = buildPerYearDensity(days, overrides)
    expect(map.get(2026)).toBe(100_000)
    expect(map.get(1990)).toBe(42)
  })

  test('inferBounds derives bounds from the days array', () => {
    const days = groupEntriesByDay(sampleEntries())
    const bounds = inferBounds(days, '2026-05-17')
    expect(bounds.firstIso).toBe('2026-05-15')
    expect(bounds.lastIso).toBe('2026-05-16')
    expect(bounds.firstYear).toBe(2026)
    expect(bounds.lastYear).toBe(2026)
    expect(bounds.totalDays).toBe(2)
  })

  test('inferBounds falls back to today when days is empty', () => {
    const bounds = inferBounds([], '2026-05-17')
    expect(bounds.firstIso).toBe('2026-05-17')
    expect(bounds.lastIso).toBe('2026-05-17')
    expect(bounds.totalDays).toBe(1)
  })

  test('pickInitialDate prefers explicit target, then newest day, then today', () => {
    const days = groupEntriesByDay(sampleEntries())
    expect(pickInitialDate('2025-01-01', days, '2026-05-17')).toBe('2025-01-01')
    expect(pickInitialDate(null, days, '2026-05-17')).toBe('2026-05-16')
    expect(pickInitialDate(null, [], '2026-05-17')).toBe('2026-05-17')
  })
})
