/**
 * PaperContactSheet — the composed Browse view shell.
 *
 * Wires the navigation chrome (toolbar + day-nav + calendar + year rail) to
 * the contact-sheet body (target banner + sticky day headers + sessions +
 * card/list rendering). Pure presentation — data + handlers come from props.
 *
 * ## Responsibilities
 * - Render the sticky toolbar with the day-nav pill (anchored CalendarPopover
 *   slot) and the view-mode toggle.
 * - Render the optional target banner when the user landed from Search /
 *   On-This-Day / Intelligence.
 * - For each PaperDay, emit a sticky PaperDayHeader followed by each
 *   PaperSession (header + either grid of ContactFrame/DomainStack or list
 *   rows depending on viewMode).
 * - Mount the YearRail on the right edge.
 *
 * ## Not responsible for
 * - Data fetching, paging, prefetch — the route owns useExplorerData/url.
 * - Grouping entries into days/sessions/stacks — caller pre-groups via
 *   `src/pages/explorer/paper/group-entries.ts`.
 * - Resolving domain colours — caller can override or rely on the deterministic
 *   helper in `src/pages/explorer/paper/domain-color.ts`.
 *
 * ## Dependencies
 * - Paper Browse primitives + DayNavControl + CalendarPopover + YearRail.
 */

import { useEffect, useMemo, useRef, type ReactNode } from 'react'
import { cn } from '@/lib/cn'
import type { HistoryEntry } from '@/lib/types/archive'
import type { PaperBlock, PaperDay } from '@/pages/explorer/paper/group-entries'
import {
  getDomainAbbr,
  getDomainColor,
} from '@/pages/explorer/paper/domain-color'
import { PaperContactFrame } from './paper-contact-frame'
import { PaperDayHeader } from './paper-day-header'
import {
  PaperDayInsights,
  type PaperDayInsightsCopy,
} from './paper-day-insights'
import { aggregateDayInsights } from './paper-day-insights-helpers'
import { PaperDomainStack } from './paper-domain-stack'
import { PaperListRow } from './paper-list-row'
import { PaperSessionHeader } from './paper-session-header'
import { PaperTargetBanner } from './paper-target-banner'
import {
  PaperViewToggle,
  type PaperViewToggleOption,
} from './paper-view-toggle'
import {
  PaperDayNavControl,
  type PaperDayNavControlCopy,
} from './paper-day-nav-control'
import { PaperYearRail } from './paper-year-rail'

export type PaperViewMode = 'cards' | 'list'

export interface PaperContactSheetTarget {
  source: 'on-this-day' | 'search' | 'intelligence'
  date: string
  /** Pretty kicker line, e.g. "From 'On this day'" or "From search · 'rust'". */
  kicker: ReactNode
  /** Pretty date, e.g. "Saturday, May 17, 2025". */
  prettyDate: string
  /** Status sentence, e.g. "3 pages archived" / "Scrolled to record". */
  status: string
  /** Optional id of the specific entry to highlight inside the target day. */
  entryId?: number | string | null
}

export interface PaperContactSheetDayNav {
  dow: string
  monthDay: string
  year: string
  densityTier: 0 | 1 | 2 | 3 | 4
  countLabel: string
  relativeAgo: string
  isToday: boolean
  prevDisabled?: boolean
  nextDisabled?: boolean
  onPrev: () => void
  onNext: () => void
  onToday: () => void
  onToggleCal: () => void
  calOpen: boolean
  copy: PaperDayNavControlCopy
  /** Mounted calendar popover (PaperCalendarPopover) when calOpen is true. */
  calendarSlot?: ReactNode
}

export interface PaperContactSheetYearRail {
  densityByYear: ReadonlyMap<number, number>
  bounds: { firstYear: number; lastYear: number; lastIso: string }
  currentDate: string
  onJump: (iso: string) => void
  ariaLabel?: string
  /** Localised "now" caption under the newest-year footer. */
  nowLabel?: string
  /** Localised "first" caption under the oldest-year footer. */
  firstLabel?: string
}

export interface PaperContactSheetCopy {
  view: string
  cards: string
  list: string
  /** Template, e.g. "{count} pages · {sessions} sessions". */
  dayMeta: string
  /** Template, e.g. "Day {n}". */
  dayIndex: string
  clearTarget: string
  expandStack: string
  moreInStack: string
  pagesLabel: string
  empty: string
}

/**
 * Pagination footer descriptor. Optional — when omitted, the contact sheet
 * renders without page navigation (which is the right call for a single-day
 * insight surface). The Browse route supplies it so a 1440 M-row archive
 * can step backwards through history pages.
 */
export interface PaperContactSheetPagination {
  page: number | null
  pageSize: number
  total: number
  pageCount: number
  hasPrevious: boolean
  hasNext: boolean
  onPrevious: () => void
  onNext: () => void
  onChangePageSize?: (next: number) => void
  copy: {
    older: string
    newer: string
    summary: string
    summaryPending: string
    pageSizeLabel: string
  }
}

/**
 * Infinite-scroll descriptor — supplied by the Browse route when there's
 * no date filter so the sheet renders a single continuous timeline that
 * lazy-loads older pages as the user scrolls toward the bottom.
 */
export interface PaperContactSheetInfiniteScroll {
  loadingMore: boolean
  canLoadMore: boolean
  onLoadMore: () => void
  loadedPageCount: number
  totalPages: number
  totalRows: number
  copy: {
    loadingMore: string
    endOfArchive: string
    loadedSummary: string
  }
}

export interface PaperContactSheetProps {
  /** Pre-grouped days, newest → oldest. */
  days: PaperDay[]
  viewMode: PaperViewMode
  onViewModeChange: (next: PaperViewMode) => void
  dayNav: PaperContactSheetDayNav
  yearRail?: PaperContactSheetYearRail
  /** Active target landed-on from another route; null when browsing freely. */
  target?: PaperContactSheetTarget | null
  onClearTarget?: () => void
  selectedEntryId?: number | string | null
  onSelectEntry?: (entry: HistoryEntry) => void
  /** Optional page-level navigation footer. Null disables the footer. */
  pagination?: PaperContactSheetPagination | null
  /** Optional infinite-scroll descriptor — mutually exclusive with pagination. */
  infiniteScroll?: PaperContactSheetInfiniteScroll | null
  /**
   * Localised copy for the per-day insights strip. When omitted the strip
   * is hidden — the design tool shows it under every day separator, so
   * routes that want the editorial Browse layout must supply this.
   */
  dayInsightsCopy?: PaperDayInsightsCopy
  /** Language tag used for time/labels in headers. */
  language?: string
  copy: PaperContactSheetCopy
  className?: string
  testId?: string
}

export function PaperContactSheet({
  days,
  viewMode,
  onViewModeChange,
  dayNav,
  yearRail,
  target,
  onClearTarget,
  selectedEntryId = null,
  onSelectEntry,
  pagination,
  infiniteScroll,
  dayInsightsCopy,
  language = 'en',
  copy,
  className,
  testId,
}: PaperContactSheetProps) {
  const toolbarRef = useRef<HTMLDivElement | null>(null)
  const viewOptions = useMemo<PaperViewToggleOption<PaperViewMode>[]>(
    () => [
      { value: 'cards', label: `⊞ ${copy.cards}` },
      { value: 'list', label: `☰ ${copy.list}` },
    ],
    [copy.cards, copy.list],
  )

  let globalFrameIndex = 0

  return (
    <section
      data-testid={testId}
      className={cn('relative flex w-full flex-col', className)}
    >
      <div
        ref={toolbarRef}
        className={cn(
          'sticky top-0 z-[11] -mx-7 flex h-[44px] items-center justify-between gap-4 px-7',
          'bg-paper border-b border-transparent',
        )}
      >
        <PaperDayNavControl
          {...dayNav}
          calendarSlot={dayNav.calendarSlot}
          testId="paper-contact-sheet-day-nav"
        />
        <div className="flex items-center gap-2">
          <span className="text-ink-faint mr-1 font-mono text-[9.5px] uppercase tracking-[0.08em]">
            {copy.view}
          </span>
          <PaperViewToggle
            value={viewMode}
            options={viewOptions}
            onChange={onViewModeChange}
            ariaLabel={copy.view}
          />
        </div>
      </div>

      {target ? (
        <PaperTargetBanner
          source={target.source}
          kicker={target.kicker}
          date={target.prettyDate}
          status={target.status}
          onClear={onClearTarget ?? (() => {})}
          clearLabel={copy.clearTarget}
          className="mt-4"
          testId="paper-contact-sheet-target-banner"
        />
      ) : null}

      {days.length === 0 ? (
        <div className="text-ink-faint mt-10 text-center font-serif text-[16px] italic">
          {copy.empty}
        </div>
      ) : (
        days.map((day, dayIndex) => (
          <div
            key={day.date}
            data-day={day.date}
            className={cn('flex flex-col', dayIndex > 0 && 'mt-8')}
          >
            <PaperDayHeader
              label={describeDayInLanguage(day.date, language)}
              meta={copy.dayMeta
                .replace('{count}', String(day.visitCount))
                .replace('{sessions}', String(day.sessions.length))}
              rightIndex={copy.dayIndex.replace(
                '{n}',
                String(days.length - dayIndex),
              )}
              active={target?.date === day.date}
            />

            {dayInsightsCopy ? (
              <PaperDayInsights
                insights={aggregateDayInsights(day)}
                copy={dayInsightsCopy}
                language={language}
                testId={`paper-day-insights-${day.date}`}
              />
            ) : null}

            {day.sessions.map((session) => (
              <div key={session.id} className="mt-4">
                <PaperSessionHeader
                  timeRange={formatRange(
                    session.startMs,
                    session.endMs,
                    language,
                  )}
                  label={`${session.visitCount} ${copy.pagesLabel}`}
                />

                {viewMode === 'cards' ? (
                  <div className="grid grid-cols-[repeat(auto-fill,minmax(195px,1fr))] gap-4">
                    {session.blocks.map((block, blockIdx) =>
                      renderCardBlock({
                        block,
                        blockIdx,
                        baseIndex: globalFrameIndex,
                        increment: (n) => {
                          globalFrameIndex += n
                        },
                        copy,
                        target: target ?? null,
                        selectedEntryId,
                        onSelectEntry,
                      }),
                    )}
                  </div>
                ) : (
                  <div className="flex flex-col">
                    {flattenBlocks(session.blocks).map((entry) => (
                      <PaperListRow
                        key={entry.id}
                        entry={toListEntry(entry, language)}
                        domainColor={getDomainColor(entry.domain)}
                        domainAbbr={getDomainAbbr(entry.domain)}
                        selected={selectedEntryId === entry.id}
                        onClick={() => onSelectEntry?.(entry)}
                      />
                    ))}
                  </div>
                )}
              </div>
            ))}
          </div>
        ))
      )}

      {yearRail ? (
        <PaperYearRail
          densityByYear={yearRail.densityByYear}
          bounds={yearRail.bounds}
          currentDate={yearRail.currentDate}
          onJump={yearRail.onJump}
          ariaLabel={yearRail.ariaLabel}
          nowLabel={yearRail.nowLabel}
          firstLabel={yearRail.firstLabel}
          testId="paper-contact-sheet-year-rail"
        />
      ) : null}

      {pagination ? <PaginationFooter pagination={pagination} /> : null}
      {infiniteScroll ? <InfiniteScrollFooter scroll={infiniteScroll} /> : null}
    </section>
  )
}

function InfiniteScrollFooter({
  scroll,
}: {
  scroll: PaperContactSheetInfiniteScroll
}) {
  const sentinelRef = useRef<HTMLDivElement | null>(null)
  const { canLoadMore, loadingMore, onLoadMore } = scroll
  // IntersectionObserver-driven lazy load. Triggers `onLoadMore` whenever
  // the sentinel scrolls into view AND there's more archive below.
  // `rootMargin: 400px` pre-loads slightly before the bottom so the user
  // never sees the skeleton pop in.
  useEffect(() => {
    const node = sentinelRef.current
    if (!node) return
    if (!canLoadMore) return
    if (typeof IntersectionObserver === 'undefined') {
      // jsdom + extremely old browsers: fall back to firing immediately so
      // the auto-load still works (slower, but never gets stuck).
      onLoadMore()
      return
    }
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          onLoadMore()
        }
      },
      { root: null, rootMargin: '400px', threshold: 0 },
    )
    observer.observe(node)
    return () => observer.disconnect()
  }, [canLoadMore, onLoadMore])

  const rowsInView = scroll.totalRows
  const loadedSummary = scroll.copy.loadedSummary
    .replace('{loaded}', scroll.loadedPageCount.toLocaleString())
    .replace('{total}', Math.max(1, scroll.totalPages).toLocaleString())
    .replace('{rows}', rowsInView.toLocaleString())

  return (
    <footer
      data-testid="paper-contact-sheet-infinite-footer"
      className="mt-6 flex flex-col items-center gap-3 border-t border-border-light pt-4 font-mono text-[10.5px] tracking-[0.04em] text-ink-muted"
    >
      {loadingMore ? (
        <div
          data-testid="paper-contact-sheet-infinite-skeleton"
          className="flex w-full max-w-[640px] flex-col gap-3"
          aria-hidden="true"
        >
          {/* Lazy-load skeleton — three placeholder day cards that pulse while
              the next page is in flight. Matches the design tool's lazy state. */}
          {[0, 1, 2].map((index) => (
            <div
              key={index}
              className="flex animate-pulse items-stretch gap-3 rounded-paper border border-border-light bg-card-paper px-3 py-3"
            >
              <div className="h-14 w-14 shrink-0 rounded-paper bg-page" />
              <div className="flex flex-1 flex-col gap-2 py-1">
                <div className="h-3 w-2/3 rounded-[2px] bg-page" />
                <div className="h-2.5 w-1/2 rounded-[2px] bg-page opacity-70" />
                <div className="h-2 w-1/3 rounded-[2px] bg-page opacity-50" />
              </div>
            </div>
          ))}
          <span className="text-center text-ink-faint">
            {scroll.copy.loadingMore}
          </span>
        </div>
      ) : null}
      {!loadingMore && canLoadMore ? (
        <div
          ref={sentinelRef}
          data-testid="paper-contact-sheet-infinite-sentinel"
          aria-hidden="true"
          className="h-px w-full"
        />
      ) : null}
      <span className="text-ink-faint">
        {canLoadMore ? loadedSummary : scroll.copy.endOfArchive}
      </span>
    </footer>
  )
}

function PaginationFooter({
  pagination,
}: {
  pagination: PaperContactSheetPagination
}) {
  const {
    page,
    pageSize,
    total,
    pageCount,
    hasPrevious,
    hasNext,
    onPrevious,
    onNext,
    onChangePageSize,
    copy,
  } = pagination
  const currentPage = page ?? 1
  const summary =
    pageCount > 0
      ? copy.summary
          .replace('{page}', currentPage.toLocaleString())
          .replace('{pageCount}', pageCount.toLocaleString())
          .replace('{total}', total.toLocaleString())
      : copy.summaryPending
  return (
    <footer
      data-testid="paper-contact-sheet-pagination"
      className="border-border-light mt-6 flex flex-wrap items-center justify-between gap-3 border-t pt-4 font-mono text-[10.5px] tracking-[0.04em] text-ink-muted"
    >
      <span>{summary}</span>
      <div className="flex items-center gap-2">
        {onChangePageSize ? (
          <label className="flex items-center gap-2">
            <span className="text-ink-faint">{copy.pageSizeLabel}</span>
            <select
              value={pageSize}
              onChange={(event) =>
                onChangePageSize(Number.parseInt(event.target.value, 10))
              }
              className="border-border-default bg-card-paper text-ink rounded-paper border px-2 py-1 font-mono text-[10.5px]"
              data-testid="paper-contact-sheet-page-size"
            >
              {[25, 50, 100, 200].map((option) => (
                <option key={option} value={option}>
                  {option}
                </option>
              ))}
            </select>
          </label>
        ) : null}
        <button
          type="button"
          disabled={!hasPrevious}
          onClick={onPrevious}
          data-testid="paper-contact-sheet-page-prev"
          className="border-border-default text-ink-muted hover:border-ink-muted hover:text-ink rounded-paper border px-3 py-1 transition-colors duration-150 disabled:cursor-not-allowed disabled:opacity-50"
        >
          ← {copy.newer}
        </button>
        <button
          type="button"
          disabled={!hasNext}
          onClick={onNext}
          data-testid="paper-contact-sheet-page-next"
          className="border-border-default text-ink-muted hover:border-ink-muted hover:text-ink rounded-paper border px-3 py-1 transition-colors duration-150 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {copy.older} →
        </button>
      </div>
    </footer>
  )
}

interface CardBlockArgs {
  block: PaperBlock
  blockIdx: number
  baseIndex: number
  increment: (n: number) => void
  copy: PaperContactSheetCopy
  target: PaperContactSheetTarget | null
  selectedEntryId: number | string | null
  onSelectEntry?: (entry: HistoryEntry) => void
}

function renderCardBlock({
  block,
  blockIdx,
  baseIndex,
  increment,
  copy,
  target,
  selectedEntryId,
  onSelectEntry,
}: CardBlockArgs) {
  if (block.type === 'stack') {
    const targetId =
      target && target.entryId !== undefined ? target.entryId : null
    const node = (
      <PaperDomainStack
        key={`stack-${blockIdx}`}
        domain={block.domain}
        domainColor={getDomainColor(block.domain)}
        domainAbbr={getDomainAbbr(block.domain)}
        entries={block.entries.map((entry) => ({
          id: entry.id,
          title: entry.title ?? null,
          domain: entry.domain,
          url: entry.url,
          time: formatTimeFromVisitTime(entry.visitTime),
        }))}
        targetEntryId={targetId}
        onSelectEntry={(entry) => {
          const real = block.entries.find((e) => e.id === entry.id)
          if (real) onSelectEntry?.(real)
        }}
        expandLabel={copy.expandStack}
        morePrefix={copy.moreInStack}
        pagesLabel={copy.pagesLabel}
      />
    )
    increment(block.entries.length)
    return node
  }

  const idx = baseIndex
  const entry = block.entry
  increment(1)
  return (
    <PaperContactFrame
      key={entry.id}
      entry={{
        id: entry.id,
        title: entry.title ?? null,
        domain: entry.domain,
        url: entry.url,
        time: formatTimeFromVisitTime(entry.visitTime),
        faviconDataUrl: entry.favicon?.dataUrl ?? null,
        ogImageDataUrl: entry.ogImage?.dataUrl ?? null,
      }}
      domainColor={getDomainColor(entry.domain)}
      domainAbbr={getDomainAbbr(entry.domain)}
      index={idx}
      selected={selectedEntryId === entry.id}
      onClick={() => onSelectEntry?.(entry)}
    />
  )
}

function flattenBlocks(blocks: PaperBlock[]): HistoryEntry[] {
  const out: HistoryEntry[] = []
  for (const block of blocks) {
    if (block.type === 'single') out.push(block.entry)
    else out.push(...block.entries)
  }
  return out
}

function toListEntry(entry: HistoryEntry, language: string) {
  return {
    id: entry.id,
    title: entry.title ?? null,
    domain: entry.domain,
    url: entry.url,
    time: formatTimeFromVisitTime(entry.visitTime, language),
    faviconDataUrl: entry.favicon?.dataUrl ?? null,
  }
}

function formatTimeFromVisitTime(
  visitTime: number,
  language: string = 'en',
): string {
  const ms = visitTime > 1e12 ? visitTime : visitTime * 1000
  const date = new Date(ms)
  if (Number.isNaN(date.getTime())) return '--:--'
  try {
    return date.toLocaleTimeString(language, {
      hour: '2-digit',
      minute: '2-digit',
      hour12: false,
    })
  } catch {
    return '--:--'
  }
}

function formatRange(startMs: number, endMs: number, language: string): string {
  const start = new Date(startMs)
  const end = new Date(endMs)
  if (Number.isNaN(start.getTime()) || Number.isNaN(end.getTime())) {
    return '--:-- — --:--'
  }
  const opts: Intl.DateTimeFormatOptions = {
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  }
  try {
    return `${start.toLocaleTimeString(language, opts)} — ${end.toLocaleTimeString(language, opts)}`
  } catch {
    return '--:-- — --:--'
  }
}

function describeDayInLanguage(date: string, language: string): string {
  try {
    const parts = date.split('-').map((part) => Number.parseInt(part, 10))
    if (parts.length !== 3 || parts.some(Number.isNaN)) return date
    const [year, month, day] = parts
    const native = new Date(year, month - 1, day)
    if (Number.isNaN(native.getTime())) return date
    return native.toLocaleDateString(language, {
      weekday: 'long',
      month: 'long',
      day: 'numeric',
      year: 'numeric',
    })
  } catch {
    return date
  }
}
