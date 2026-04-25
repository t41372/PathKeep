/**
 * @file browsing-rhythm-calendar.tsx
 * @description Render-only year controls and calendar grid for the browsing-rhythm card.
 * @module components/intelligence
 *
 * ## Responsibilities
 * - Render the year picker / pager chrome for the shared browsing-rhythm card.
 * - Render the calendar heatmap grid from memoized week and month models.
 *
 * ## Not responsible for
 * - Fetching discovery-trend data or deriving calendar/date summaries.
 * - Rendering the selected-day detail rail.
 * - Managing route navigation or data prewarm decisions.
 *
 * ## Dependencies
 * - Depends on the shared browsing-rhythm helper models and translator contract.
 * - Uses the shared `rhythm-calendar` CSS classes for the shipped layout.
 *
 * ## Performance notes
 * - The render paths stay flat and prop-driven so the parent can memoize the heavy date math once.
 */

import type {
  BrowsingRhythmCalendarCell,
  BrowsingRhythmTranslator,
} from './browsing-rhythm-card-helpers'

interface BrowsingRhythmYearControlsProps {
  canResetToCurrentYear: boolean
  mode: 'range' | 'year'
  newerYear: number | null
  olderYear: number | null
  onResetToCurrentYear: () => void
  onSelectYear: (year: number) => void
  selectedYear: number
  t: BrowsingRhythmTranslator
  yearNavigation: 'select' | 'pager'
  yearOptions: number[]
}

/**
 * Renders the year-navigation chrome so the card can keep the selector logic isolated from the calendar body.
 */
export function BrowsingRhythmYearControls({
  canResetToCurrentYear,
  mode,
  newerYear,
  olderYear,
  onResetToCurrentYear,
  onSelectYear,
  selectedYear,
  t,
  yearNavigation,
  yearOptions,
}: BrowsingRhythmYearControlsProps) {
  return (
    <div className="browsing-rhythm-card__controls">
      <span className="browsing-rhythm-card__legend">{t('rhythmLegend')}</span>
      {mode === 'year' ? (
        yearNavigation === 'pager' ? (
          <div className="browsing-rhythm-card__year-actions">
            {canResetToCurrentYear ? (
              <button
                className="browsing-rhythm-card__current-year-button"
                data-testid="browsing-rhythm-current-year-shortcut"
                type="button"
                onClick={onResetToCurrentYear}
              >
                {t('rhythmCurrentYearAction')}
              </button>
            ) : null}
            <div
              className="browsing-rhythm-card__year-pager"
              data-testid="browsing-rhythm-year-pager"
            >
              <button
                aria-label={t('rhythmPreviousYearAria', {
                  year: olderYear ?? selectedYear,
                })}
                className="browsing-rhythm-card__year-button"
                data-testid="browsing-rhythm-year-previous"
                disabled={olderYear === null}
                type="button"
                onClick={() => {
                  if (olderYear !== null) {
                    onSelectYear(olderYear)
                  }
                }}
              >
                {'<'}
              </button>
              <div
                aria-live="polite"
                className="browsing-rhythm-card__year-current"
                data-testid="browsing-rhythm-year-label"
              >
                <span className="browsing-rhythm-card__year-caption">
                  {t('rhythmYearLabel')}
                </span>
                <strong className="browsing-rhythm-card__year-value">
                  {selectedYear}
                </strong>
              </div>
              <button
                aria-label={t('rhythmNextYearAria', {
                  year: newerYear ?? selectedYear,
                })}
                className="browsing-rhythm-card__year-button"
                data-testid="browsing-rhythm-year-next"
                disabled={newerYear === null}
                type="button"
                onClick={() => {
                  if (newerYear !== null) {
                    onSelectYear(newerYear)
                  }
                }}
              >
                {'>'}
              </button>
            </div>
          </div>
        ) : (
          <label className="browsing-rhythm-card__selector">
            <span>{t('rhythmYearLabel')}</span>
            <select
              aria-label={t('rhythmYearAria', {
                year: selectedYear,
              })}
              className="browsing-rhythm-card__select"
              data-testid="browsing-rhythm-year-select"
              value={selectedYear}
              onChange={(event) => {
                onSelectYear(Number(event.target.value))
              }}
            >
              {yearOptions.map((year) => (
                <option key={year} value={year}>
                  {year}
                </option>
              ))}
            </select>
          </label>
        )
      ) : null}
    </div>
  )
}

interface BrowsingRhythmCalendarGridProps {
  calendarWeeks: BrowsingRhythmCalendarCell[][]
  maxVisits: number
  monthLabels: string[]
  onSelectDay: (dateKey: string) => void
  selectedDateKey: string | null
  t: BrowsingRhythmTranslator
  weekdayLabels: string[]
}

/**
 * Renders the calendar body so the parent can keep the date-grid math separate from the JSX tree.
 */
export function BrowsingRhythmCalendarGrid({
  calendarWeeks,
  maxVisits,
  monthLabels,
  onSelectDay,
  selectedDateKey,
  t,
  weekdayLabels,
}: BrowsingRhythmCalendarGridProps) {
  return (
    <div className="rhythm-calendar-shell">
      <div className="rhythm-calendar__months">
        <span className="rhythm-calendar__months-spacer" />
        <div className="rhythm-calendar__months-track">
          {monthLabels.map((label, index) => (
            <span key={`${label}:${index}`} className="rhythm-calendar__month">
              {label}
            </span>
          ))}
        </div>
      </div>
      <div
        className="rhythm-calendar"
        role="grid"
        aria-label={t('rhythmLabel')}
      >
        <div className="rhythm-calendar__weekday-rail" aria-hidden="true">
          {weekdayLabels.map((label) => (
            <span key={label} className="rhythm-calendar__weekday">
              {label}
            </span>
          ))}
        </div>
        <div className="rhythm-calendar__weeks">
          {calendarWeeks.map((week, weekIndex) => (
            <div key={weekIndex} className="rhythm-calendar__week">
              {week.map((cell) => {
                const level = heatLevel(cell.totalVisits, maxVisits)
                const isSelected = selectedDateKey === cell.dateKey

                return (
                  <button
                    key={cell.dateKey}
                    type="button"
                    disabled={!cell.inRange}
                    className={`rhythm-calendar__day${
                      cell.inRange ? '' : ' rhythm-calendar__day--outside'
                    }${isSelected ? ' rhythm-calendar__day--active' : ''}`}
                    data-level={level}
                    aria-label={t('rhythmDayTooltip', {
                      date: cell.dateKey,
                      count: cell.totalVisits,
                      newDomains: cell.newDomainCount,
                    })}
                    title={t('rhythmDayTooltip', {
                      date: cell.dateKey,
                      count: cell.totalVisits,
                      newDomains: cell.newDomainCount,
                    })}
                    onClick={() => {
                      onSelectDay(cell.dateKey)
                    }}
                  />
                )
              })}
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}

function heatLevel(count: number, maxCount: number) {
  if (count <= 0 || maxCount <= 0) {
    return 0
  }

  const ratio = count / maxCount
  if (ratio >= 0.75) return 4
  if (ratio >= 0.5) return 3
  if (ratio >= 0.25) return 2
  return 1
}
