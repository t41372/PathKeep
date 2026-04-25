/**
 * @file browsing-rhythm-card.tsx
 * @description Shared calendar-based browsing rhythm card used by Dashboard and `/intelligence`.
 * @module components/intelligence
 *
 * ## Responsibilities
 * - Compose the extracted year controls, calendar grid, and day-detail renderers.
 * - Keep Dashboard and `/intelligence` wired to the same browsing-rhythm interaction surface.
 * - Delegate data loading and selected-year/day state to the sibling state hook.
 *
 * ## Not responsible for
 * - Implementing the shared day-detail primitives, discovery-trend math helpers, or state hook.
 * - Defining route-level navigation destinations.
 * - Owning the `/intelligence` or dashboard section chrome around the card.
 *
 * ## Dependencies
 * - Depends on the shared browsing-rhythm subcomponents and state hook.
 * - Receives route-specific copy, href builders, profile scope, and date range from consumers.
 *
 * ## Performance notes
 * - Keeps this render tree prop-driven; the O(days-in-range) derivation stays in the memoized state hook.
 */

import './browsing-rhythm-card.css'

import {
  BrowsingRhythmCalendarGrid,
  BrowsingRhythmYearControls,
} from './browsing-rhythm-calendar'
import { type BrowsingRhythmTranslator as BrowsingRhythmTranslatorType } from './browsing-rhythm-card-helpers'
import { useBrowsingRhythmCardState } from './browsing-rhythm-card-state'
import { BrowsingRhythmDayDetail } from './browsing-rhythm-day-detail'
import {
  type CoreIntelligenceSectionMeta,
  type DateRange,
  type TimeRangePreset,
} from '../../lib/core-intelligence'

/**
 * Shared translator signature for the browsing-rhythm card and its extracted subviews.
 */
export type BrowsingRhythmTranslator = BrowsingRhythmTranslatorType

interface BrowsingRhythmCardProps {
  dateRange?: DateRange
  dayDomainHref?: (domain: string, date: string) => string
  dayHref: (date: string) => string
  language: string
  mode: 'range' | 'year'
  onTrendMetaChange?: (meta: CoreIntelligenceSectionMeta | null) => void
  profileId?: string | null
  refreshToken?: number | string | null
  showCurrentYearShortcut?: boolean
  summaryPreset?: TimeRangePreset | 'calendar-year'
  t: BrowsingRhythmTranslator
  yearNavigation?: 'select' | 'pager'
}

/**
 * Reusable calendar-based browsing rhythm card shared by Dashboard and
 * `/intelligence`.
 *
 * The accepted contract is a real-date calendar heatmap backed by
 * `getDiscoveryTrend(..., 'day')`. Selecting a day keeps the user in context
 * and lazy-loads a compact inline preview; navigation to the first-class day
 * insights route only happens through the explicit detail CTA.
 */
export function BrowsingRhythmCard({
  dateRange,
  dayDomainHref,
  dayHref,
  language,
  mode,
  onTrendMetaChange,
  profileId,
  refreshToken,
  showCurrentYearShortcut = false,
  summaryPreset,
  t,
  yearNavigation = 'select',
}: BrowsingRhythmCardProps) {
  const {
    calendarDays,
    calendarWeeks,
    canResetToCurrentYear,
    hasCalendarVisits,
    maxVisits,
    monthLabels,
    newerYear,
    olderYear,
    resetToCurrentYear,
    selectDay,
    selectYear,
    selectedDay,
    selectedDayDetail,
    selectedDayError,
    selectedDayLoading,
    selectedYear,
    trendError,
    trendLoading,
    visibleRangeHint,
    visitSummary,
    waitingForYearRealignment,
    weekdayLabels,
    yearOptions,
  } = useBrowsingRhythmCardState({
    dateRange,
    language,
    mode,
    onTrendMetaChange,
    profileId,
    refreshToken,
    showCurrentYearShortcut,
    summaryPreset,
    t,
  })

  return (
    <div className="browsing-rhythm-card">
      <BrowsingRhythmYearControls
        canResetToCurrentYear={canResetToCurrentYear}
        mode={mode}
        newerYear={newerYear}
        olderYear={olderYear}
        onResetToCurrentYear={resetToCurrentYear}
        onSelectYear={selectYear}
        selectedYear={selectedYear}
        t={t}
        yearNavigation={yearNavigation}
        yearOptions={yearOptions}
      />

      {trendLoading || waitingForYearRealignment ? (
        <div className="browsing-rhythm-card__skeleton" />
      ) : trendError ? (
        <div className="browsing-rhythm-card__empty">
          <p className="browsing-rhythm-card__empty-text">{trendError}</p>
        </div>
      ) : mode !== 'year' && !hasCalendarVisits ? (
        <div className="browsing-rhythm-card__empty">
          <p className="browsing-rhythm-card__empty-text">{t('rhythmEmpty')}</p>
        </div>
      ) : (
        <>
          <p
            className="browsing-rhythm-card__summary"
            data-testid="browsing-rhythm-summary"
          >
            {visitSummary}
          </p>
          {visibleRangeHint ? (
            <p className="browsing-rhythm-card__range-hint">
              {visibleRangeHint}
            </p>
          ) : null}
          <BrowsingRhythmCalendarGrid
            calendarWeeks={calendarWeeks}
            maxVisits={maxVisits}
            monthLabels={monthLabels}
            onSelectDay={selectDay}
            selectedDateKey={selectedDay?.dateKey ?? null}
            t={t}
            weekdayLabels={weekdayLabels}
          />

          {selectedDay ? (
            <BrowsingRhythmDayDetail
              dateKey={selectedDay.dateKey}
              dayDomainHref={dayDomainHref}
              dayHref={dayHref}
              detail={selectedDayDetail}
              error={selectedDayError}
              language={language}
              loading={selectedDayLoading}
              t={t}
            />
          ) : calendarDays.some((cell) => cell.totalVisits > 0) ? (
            <div className="browsing-rhythm-card__empty browsing-rhythm-card__empty--prompt">
              <p className="browsing-rhythm-card__empty-text">
                {t('rhythmSelectDayPrompt')}
              </p>
            </div>
          ) : null}
        </>
      )}
    </div>
  )
}
