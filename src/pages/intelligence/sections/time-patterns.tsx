/**
 * Weekday-vs-Weekend and Peak Hours strips for the Intelligence hub Time axis.
 *
 * ## Responsibilities
 * - Derive weekday vs weekend aggregate visit counts from the browsing rhythm
 *   heatmap data (RhythmHeatmapCell already carries dow 0-6).
 * - Derive the peak 3-hour window from the same hourly distribution.
 * - Render compact inline strips suitable for embedding inside the Time axis card.
 *
 * ## Not responsible for
 * - Fetching the rhythm heatmap (caller passes pre-loaded data).
 * - Rendering the full calendar heatmap (that stays in BrowsingRhythmSection).
 *
 * ## Dependencies
 * - RhythmHeatmapCell type from `lib/core-intelligence`.
 * - i18n `t` translator from the parent coordinator.
 *
 * ## Performance notes
 * - Aggregation runs over the heatmap cells array (max 7*24 = 168 items) on
 *   each render; negligible cost even on the target 4-core machine.
 */

import type { RhythmHeatmapCell } from '../../../lib/core-intelligence'
import {
  computePeakHours,
  computeWeekdayWeekend,
} from './time-patterns-helpers'
import type { T } from './shared'

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

interface WeekdayWeekendStripProps {
  cells: RhythmHeatmapCell[]
  t: T
}

/**
 * Inline strip showing weekday vs weekend visit split with proportional bars.
 */
export function WeekdayWeekendStrip({ cells, t }: WeekdayWeekendStripProps) {
  const { weekdayVisits, weekendVisits } = computeWeekdayWeekend(cells)
  const total = weekdayVisits + weekendVisits

  if (total === 0) {
    return (
      <div className="time-pattern-strip" data-testid="weekday-weekend-strip">
        <span className="time-pattern-strip__empty">
          {t('hubWeekdayWeekendEmpty')}
        </span>
      </div>
    )
  }

  const weekdayPercent = Math.round((weekdayVisits / total) * 100)
  const weekendPercent = 100 - weekdayPercent

  return (
    <div className="time-pattern-strip" data-testid="weekday-weekend-strip">
      <div className="time-pattern-strip__row">
        <span className="time-pattern-strip__label">
          {t('hubWeekdayLabel')}
        </span>
        <span className="time-pattern-strip__bar-track">
          <span
            className="time-pattern-strip__bar-fill"
            style={{ width: `${weekdayPercent}%` }}
          />
        </span>
        <span className="time-pattern-strip__value">{weekdayPercent}%</span>
      </div>
      <div className="time-pattern-strip__row">
        <span className="time-pattern-strip__label">
          {t('hubWeekendLabel')}
        </span>
        <span className="time-pattern-strip__bar-track">
          <span
            className="time-pattern-strip__bar-fill time-pattern-strip__bar-fill--alt"
            style={{ width: `${weekendPercent}%` }}
          />
        </span>
        <span className="time-pattern-strip__value">{weekendPercent}%</span>
      </div>
    </div>
  )
}

interface PeakHoursStripProps {
  cells: RhythmHeatmapCell[]
  t: T
}

/**
 * Inline strip showing the most active 3-hour window.
 */
export function PeakHoursStrip({ cells, t }: PeakHoursStripProps) {
  const peak = computePeakHours(cells)

  if (!peak) {
    return (
      <div className="time-pattern-strip" data-testid="peak-hours-strip">
        <span className="time-pattern-strip__empty">
          {t('hubPeakHoursEmpty')}
        </span>
      </div>
    )
  }

  const startLabel = String(peak.startHour).padStart(2, '0') + ':00'
  const endLabel = String((peak.startHour + 3) % 24).padStart(2, '0') + ':00'

  return (
    <div className="time-pattern-strip" data-testid="peak-hours-strip">
      <div className="time-pattern-strip__peak">
        <span className="time-pattern-strip__peak-label">
          {t('hubPeakHoursTitle')}
        </span>
        <span className="time-pattern-strip__peak-value">
          {startLabel} &ndash; {endLabel}
        </span>
      </div>
    </div>
  )
}
