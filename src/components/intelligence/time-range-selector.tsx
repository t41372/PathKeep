/**
 * Shared Core Intelligence time-range selector used by route shells and deep-linkable domain drilldowns.
 *
 * Why this file exists:
 * - Core Intelligence surfaces should share one time-range control instead of inlining slightly different query behavior per route.
 * - Keeping the selector separate makes the route shell easier to read and gives tests one place to verify range-to-URL behavior.
 *
 * Main declarations:
 * - `TimeRangeSelector`
 *
 * Source-of-truth notes:
 * - Keep this aligned with `docs/features/core-intelligence-ultimate-design.md` §4.B.
 * - Keep copy, loading, and scope grammar aligned with `docs/design/screens-and-nav.md` and `docs/design/ux-principles.md`.
 */

import { useState } from 'react'
import type {
  DateRange,
  TimeRangePreset,
} from '../../lib/core-intelligence/types'

type Translate = (key: string, vars?: Record<string, string | number>) => string

interface TimeRangeSelectorProps {
  dateRange: DateRange
  preset: TimeRangePreset
  onCustomRange: (range: DateRange) => void
  onPresetChange: (preset: TimeRangePreset) => void
  t: Translate
}

/**
 * Renders the shared Core Intelligence time-range selector.
 */
export function TimeRangeSelector({
  dateRange,
  preset,
  onCustomRange,
  onPresetChange,
  t,
}: TimeRangeSelectorProps) {
  const presets: { key: TimeRangePreset; label: string }[] = [
    { key: 'day', label: t('rangeDay') },
    { key: 'week', label: t('rangeWeek') },
    { key: 'month', label: t('rangeMonth') },
    { key: 'quarter', label: t('rangeQuarter') },
    { key: 'year', label: t('rangeYear') },
    { key: 'custom', label: t('rangeCustom') },
  ]
  const [customStart, setCustomStart] = useState(dateRange.start)
  const [customEnd, setCustomEnd] = useState(dateRange.end)
  const showCustom = preset === 'custom'

  return (
    <div
      className="time-range-bar"
      role="toolbar"
      aria-label={t('timeRangeLabel')}
    >
      <div className="time-range-bar__presets">
        {presets.map(({ key, label }) => (
          <button
            key={key}
            className={`time-range-bar__btn${preset === key ? ' time-range-bar__btn--active' : ''}`}
            type="button"
            onClick={() => {
              if (key === 'custom') {
                onPresetChange(key)
                return
              }

              onPresetChange(key)
            }}
            aria-pressed={preset === key}
          >
            {label}
          </button>
        ))}
      </div>
      {showCustom ? (
        <div className="time-range-bar__custom">
          <input
            type="date"
            value={customStart}
            onChange={(event) => setCustomStart(event.target.value)}
            aria-label={t('customStart')}
          />
          <span className="time-range-bar__separator">-</span>
          <input
            type="date"
            value={customEnd}
            onChange={(event) => setCustomEnd(event.target.value)}
            aria-label={t('customEnd')}
          />
          <button
            className="time-range-bar__apply"
            type="button"
            onClick={() =>
              onCustomRange({ start: customStart, end: customEnd })
            }
          >
            {t('applyRange')}
          </button>
        </div>
      ) : null}
    </div>
  )
}
