/**
 * Paper Explorer view — the v0.3 Browse experience, wired to a stream of
 * HistoryEntry rows. Owns the day-nav state, calendar popover toggle, and
 * density-map derivation so the route file just feeds entries + handlers.
 *
 * ## Responsibilities
 * - Group HistoryEntry rows into days/sessions/blocks via group-entries.ts.
 * - Derive density maps (per-day count, per-year aggregate) from the loaded
 *   entries plus an optional `additionalDensity` override the route can
 *   supply when richer aggregates are available (e.g. core-intelligence).
 * - Drive the day-nav pill state (current date, ago label, density tier,
 *   prev/next within loaded range).
 * - Toggle the CalendarPopover via the day-nav pill click + Esc.
 * - Render `PaperContactSheet` with all the chrome composed.
 *
 * ## Not responsible for
 * - Data fetching — caller hands us `entries` + handlers.
 * - URL state — the route owns useExplorerUrlState and feeds `targetDate`.
 * - Semantic / regex panels — those keep their own panels for now.
 *
 * ## Dependencies
 * - `src/components/explorer-paper/*` for the visual layer.
 * - `src/pages/explorer/paper/{group-entries,date-helpers,domain-color}.ts`
 *   for the data-shaping helpers.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import {
  PaperCalendarPopover,
  PaperContactSheet,
  type PaperContactSheetCopy,
  type PaperContactSheetDayNav,
  type PaperContactSheetTarget,
  type PaperContactSheetYearRail,
  type PaperViewMode,
} from '@/components/explorer-paper'
import type { HistoryEntry } from '@/lib/types/archive'
import {
  groupEntriesByDay,
  type PaperDay,
} from '@/pages/explorer/paper/group-entries'
import {
  addDaysIso,
  dateFromIso,
  dayDensityTier,
  isoFromDate,
  prettyDay,
  relativeDayLabel,
  type RelativeDayCopy,
} from '@/pages/explorer/paper/date-helpers'
import {
  buildPerDayDensity,
  buildPerYearDensity,
  inferBounds,
  pickInitialDate,
} from './paper-view-helpers'

export interface PaperExplorerCopy {
  contactSheet: PaperContactSheetCopy
  dayNav: PaperContactSheetDayNav['copy']
  relative: RelativeDayCopy
  calendar: {
    prevMonth: string
    nextMonth: string
    months: [
      string,
      string,
      string,
      string,
      string,
      string,
      string,
      string,
      string,
      string,
      string,
      string,
    ]
    dowLabels: [string, string, string, string, string, string, string]
    today: string
    oneYearAgo: string
    pagesArchived: string
    monthSummary: string
    boundsMeta: string
    dialogLabel: string
  }
  /** Template for the year-rail tooltip: "{year} · {count} pages". */
  yearRailTitle: string
  /** ARIA label for the year-scrubber rail. */
  yearRailAria: string
  /** Caption under the newest-year footer of the year rail (e.g. "now"). */
  yearRailNowLabel: string
  /** Caption under the oldest-year footer of the year rail (e.g. "first"). */
  yearRailFirstLabel: string
  /** Target-banner kicker copy for the four sources. */
  target: {
    fromOnThisDay: string
    fromSearch: string
    /** Supports a `{query}` substitution. */
    fromSearchWithQuery: string
    fromIntelligence: string
    /** Supports a `{count}` substitution. */
    pagesArchived: string
    noArchive: string
  }
  pagination: {
    /** Label on the "older / next page" button (paper goes newest→oldest, so next is older). */
    older: string
    /** Label on the "newer / previous page" button. */
    newer: string
    /** Caption template, e.g. "Page {page} of {pageCount} · {total} rows". */
    summary: string
    /** Caption when total/pageCount aren't known yet (initial load). */
    summaryPending: string
    /** Page-size selector label, e.g. "Rows per page". */
    pageSizeLabel: string
  }
}

/**
 * Page-level pagination handles forwarded from the Explorer route.
 *
 * The paper Browse surface defaults to a day-grouped contact sheet, but a
 * 1440 M-row archive can't fit in any single response page. The route owns
 * the cursor / page state in `useExplorerUrlState`; the view receives only
 * the read-out state + the prev/next/page-size handlers so the footer
 * affordances stay decoupled from URL grammar.
 */
export interface PaperExplorerPagination {
  /** 1-indexed current page; `null` means the implicit "page 1". */
  page: number | null
  pageSize: number
  total: number
  /** 0 when the archive is empty or while the initial query loads. */
  pageCount: number
  hasPrevious: boolean
  hasNext: boolean
  onPrevious: () => void
  onNext: () => void
  onChangePageSize?: (next: number) => void
}

export interface PaperExplorerViewProps {
  entries: HistoryEntry[]
  /** Optional override for the archive bounds; defaults to inferred from entries. */
  archiveBounds?: {
    firstIso: string
    lastIso: string
    firstYear: number
    lastYear: number
    totalDays: number
  }
  /** Extra density data that overlays the per-entry-derived counts. */
  additionalDensity?: {
    perDay?: ReadonlyMap<string, number>
    perYear?: ReadonlyMap<number, number>
  }
  targetDate?: string | null
  targetSource?: 'on-this-day' | 'search' | 'intelligence' | null
  targetQuery?: string | null
  targetEntryId?: number | string | null
  selectedEntryId?: number | string | null
  onSelectEntry?: (entry: HistoryEntry) => void
  onJumpToDate?: (iso: string) => void
  onClearTarget?: () => void
  pagination?: PaperExplorerPagination
  language?: string
  /** Today's anchor in local time. Tests inject; routes default to new Date(). */
  todayIso?: string
  initialViewMode?: PaperViewMode
  copy: PaperExplorerCopy
  className?: string
  testId?: string
}

export function PaperExplorerView({
  entries,
  archiveBounds,
  additionalDensity,
  targetDate = null,
  targetSource = null,
  targetQuery = null,
  targetEntryId = null,
  selectedEntryId = null,
  onSelectEntry,
  onJumpToDate,
  onClearTarget,
  pagination,
  language = 'en',
  todayIso,
  initialViewMode = 'cards',
  copy,
  className,
  testId,
}: PaperExplorerViewProps) {
  const [viewMode, setViewMode] = useState<PaperViewMode>(initialViewMode)
  const [calOpen, setCalOpen] = useState(false)
  const calAnchorRef = useRef<HTMLDivElement | null>(null)

  const days = useMemo(() => groupEntriesByDay(entries), [entries])
  const today = useMemo(() => todayIso ?? isoFromDate(new Date()), [todayIso])

  const perDayDensity = useMemo(
    () => buildPerDayDensity(days, additionalDensity?.perDay),
    [days, additionalDensity?.perDay],
  )
  const perYearDensity = useMemo(
    () => buildPerYearDensity(days, additionalDensity?.perYear),
    [days, additionalDensity?.perYear],
  )

  const bounds = useMemo(
    () => archiveBounds ?? inferBounds(days, today),
    [archiveBounds, days, today],
  )

  // Active date — prefer the explicit target, else the newest loaded day,
  // else fall back to today.
  const [activeDate, setActiveDate] = useState<string>(() =>
    pickInitialDate(targetDate, days, today),
  )

  useEffect(() => {
    if (targetDate) setActiveDate(targetDate)
  }, [targetDate])

  // Sync activeDate to the newest loaded day if it drifts out of range.
  useEffect(() => {
    if (days.length === 0) return
    const loaded = new Set(days.map((day) => day.date))
    if (!loaded.has(activeDate) && !targetDate) {
      setActiveDate(days[0].date)
    }
  }, [days, activeDate, targetDate])

  const peakDailyCount = useMemo(() => {
    let peak = 1
    for (const value of perDayDensity.values()) {
      if (value > peak) peak = value
    }
    return peak
  }, [perDayDensity])

  const dayNav = useMemo<PaperContactSheetDayNav>(
    () =>
      buildDayNav({
        activeDate,
        today,
        perDayDensity,
        bounds,
        relative: copy.relative,
        navCopy: copy.dayNav,
        calOpen,
        onPrev: () => stepDay(-1),
        onNext: () => stepDay(1),
        onToday: () => {
          setActiveDate(today)
          onJumpToDate?.(today)
          setCalOpen(false)
        },
        onToggleCal: () => setCalOpen((value) => !value),
      }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [
      activeDate,
      today,
      perDayDensity,
      bounds,
      calOpen,
      copy.dayNav,
      copy.relative,
    ],
  )

  function stepDay(delta: number) {
    const next = addDaysIso(activeDate, delta)
    if (next < bounds.firstIso || next > bounds.lastIso) return
    setActiveDate(next)
    onJumpToDate?.(next)
  }

  const handleCalendarSelect = useCallback(
    (iso: string) => {
      setActiveDate(iso)
      setCalOpen(false)
      onJumpToDate?.(iso)
    },
    [onJumpToDate],
  )

  // Esc closes the calendar.
  useEffect(() => {
    if (!calOpen) return
    function handleKey(event: KeyboardEvent) {
      if (event.key === 'Escape') setCalOpen(false)
    }
    window.addEventListener('keydown', handleKey)
    return () => window.removeEventListener('keydown', handleKey)
  }, [calOpen])

  const yearRail = useMemo<PaperContactSheetYearRail>(
    () => ({
      densityByYear: perYearDensity,
      bounds: {
        firstYear: bounds.firstYear,
        lastYear: bounds.lastYear,
        lastIso: bounds.lastIso,
      },
      currentDate: activeDate,
      onJump: handleCalendarSelect,
      ariaLabel: copy.yearRailAria,
      nowLabel: copy.yearRailNowLabel,
      firstLabel: copy.yearRailFirstLabel,
    }),
    [
      perYearDensity,
      bounds,
      activeDate,
      handleCalendarSelect,
      copy.yearRailAria,
      copy.yearRailNowLabel,
      copy.yearRailFirstLabel,
    ],
  )

  const target = useMemo<PaperContactSheetTarget | null>(
    () =>
      buildTarget(
        targetDate,
        targetSource,
        targetQuery,
        targetEntryId,
        days,
        language,
        copy.target,
      ),
    [
      targetDate,
      targetSource,
      targetQuery,
      targetEntryId,
      days,
      language,
      copy.target,
    ],
  )

  const calendarSlot = calOpen ? (
    <div ref={calAnchorRef}>
      <PaperCalendarPopover
        value={activeDate}
        todayIso={today}
        densityByDate={perDayDensity}
        densityByYear={perYearDensity}
        loadedDates={new Set(days.map((day) => day.date))}
        bounds={{
          firstIso: bounds.firstIso,
          lastIso: bounds.lastIso,
          firstYear: bounds.firstYear,
          lastYear: bounds.lastYear,
          totalDays: bounds.totalDays,
        }}
        peakDailyCount={peakDailyCount}
        onSelect={handleCalendarSelect}
        copy={copy.calendar}
        testId="paper-explorer-calendar"
      />
    </div>
  ) : null

  return (
    <PaperContactSheet
      className={className}
      testId={testId}
      days={days}
      viewMode={viewMode}
      onViewModeChange={setViewMode}
      dayNav={{ ...dayNav, calendarSlot }}
      yearRail={yearRail}
      target={target}
      onClearTarget={onClearTarget}
      selectedEntryId={selectedEntryId}
      onSelectEntry={onSelectEntry}
      pagination={
        pagination
          ? {
              ...pagination,
              copy: copy.pagination,
            }
          : null
      }
      language={language}
      copy={copy.contactSheet}
    />
  )
}

function buildDayNav({
  activeDate,
  today,
  perDayDensity,
  bounds,
  relative,
  navCopy,
  calOpen,
  onPrev,
  onNext,
  onToday,
  onToggleCal,
}: {
  activeDate: string
  today: string
  perDayDensity: ReadonlyMap<string, number>
  bounds: { firstIso: string; lastIso: string }
  relative: RelativeDayCopy
  navCopy: PaperContactSheetDayNav['copy']
  calOpen: boolean
  onPrev: () => void
  onNext: () => void
  onToday: () => void
  onToggleCal: () => void
}): PaperContactSheetDayNav {
  const count = perDayDensity.get(activeDate) ?? 0
  const tier = dayDensityTier(count)
  const ago = relativeDayLabel(activeDate, today, relative)
  const date = dateFromIso(activeDate)
  const valid = !Number.isNaN(date.getTime())
  const dow = valid
    ? date.toLocaleDateString('en-US', { weekday: 'short' }).toUpperCase()
    : ''
  const monthDay = valid
    ? date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
    : activeDate
  const year = valid ? String(date.getFullYear()) : ''
  return {
    dow,
    monthDay,
    year,
    densityTier: tier,
    countLabel: count > 0 ? `${count.toLocaleString()}p` : '—',
    relativeAgo: ago,
    isToday: activeDate === today,
    prevDisabled: addDaysIso(activeDate, -1) < bounds.firstIso,
    nextDisabled: addDaysIso(activeDate, 1) > bounds.lastIso,
    onPrev,
    onNext,
    onToday,
    onToggleCal,
    calOpen,
    copy: navCopy,
  }
}

function buildTarget(
  targetDate: string | null,
  source: 'on-this-day' | 'search' | 'intelligence' | null,
  query: string | null,
  entryId: number | string | null,
  days: PaperDay[],
  language: string,
  targetCopy: PaperExplorerCopy['target'],
): PaperContactSheetTarget | null {
  if (!targetDate) return null
  const targetDay = days.find((day) => day.date === targetDate)
  const safeSource = source ?? 'on-this-day'
  const kicker =
    safeSource === 'search'
      ? query
        ? targetCopy.fromSearchWithQuery.replace('{query}', query)
        : targetCopy.fromSearch
      : safeSource === 'intelligence'
        ? targetCopy.fromIntelligence
        : targetCopy.fromOnThisDay
  const status = targetDay
    ? targetCopy.pagesArchived.replace('{count}', String(targetDay.visitCount))
    : targetCopy.noArchive
  return {
    source: safeSource,
    date: targetDate,
    kicker,
    prettyDate: prettyDay(targetDate, { language }),
    status,
    entryId: entryId ?? undefined,
  }
}
