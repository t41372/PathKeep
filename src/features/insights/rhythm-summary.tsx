/**
 * The two lines under the rhythm heatmap: visits on an average weekday
 * against an average weekend day, and the busiest hours.
 *
 * Averages divide by how many of each kind of day the range holds (all time
 * starts at the first day with data), so a range with more weekdays than
 * weekends does not tilt the comparison.
 */
import type { RhythmHeatmap } from '@/lib/core-intelligence/types'
import { useFormat, useI18n } from '@/lib/i18n'
import { chartDates } from './buckets'
import { useNewSites } from './queries'
import type { Scope } from './range'

const parse = (key: string) => new Date(`${key}T00:00:00`)

/** How many weekdays and weekend days `start..end` holds. */
function dayCounts(start: string, end: string) {
  let weekdays = 0
  let weekends = 0
  const last = parse(end)
  for (const day = parse(start); day <= last; day.setDate(day.getDate() + 1)) {
    if (day.getDay() === 0 || day.getDay() === 6) weekends += 1
    else weekdays += 1
  }
  return { weekdays, weekends }
}

export function RhythmSummary({
  scope,
  rhythm,
}: {
  scope: Scope
  rhythm: RhythmHeatmap
}) {
  const { t } = useI18n()
  const format = useFormat()
  const days = useNewSites(scope)
  const dates = chartDates(
    scope,
    (days.data?.data.points ?? []).map((point) => point.dateKey),
  )
  // All time starts at the first day with data; wait for it rather than divide by one day.
  if (scope.range === 'all' && days.data === undefined) return null
  const { weekdays, weekends } = dayCounts(dates.start, dates.end)

  let weekdayVisits = 0
  let weekendVisits = 0
  const byHour = new Array<number>(24).fill(0)
  for (const cell of rhythm.cells) {
    if (cell.dow === 0 || cell.dow === 6) weekendVisits += cell.visitCount
    else weekdayVisits += cell.visitCount
    byHour[cell.hour] += cell.visitCount
  }
  const perWeekday = weekdays ? weekdayVisits / weekdays : 0
  const perWeekendDay = weekends ? weekendVisits / weekends : 0
  const most = Math.max(perWeekday, perWeekendDay, 1)
  const busiest = byHour
    .map((visits, hour) => ({ hour, visits }))
    .filter((entry) => entry.visits > 0)
    .sort((a, b) => b.visits - a.visits)
    .slice(0, 3)

  return (
    <div className="flex flex-col gap-3 text-[13px]">
      <dl className="grid grid-cols-[auto_1fr_auto] items-center gap-x-3 gap-y-1.5">
        {[
          { label: t('insights.rhythm.weekday'), value: perWeekday },
          { label: t('insights.rhythm.weekend'), value: perWeekendDay },
        ].map((row) => (
          <div key={row.label} className="contents">
            <dt className="text-muted-foreground">{row.label}</dt>
            <dd className="h-1.5 overflow-hidden rounded-full bg-muted">
              <span
                className="block h-full rounded-full bg-brand transition-[width] duration-500"
                style={{ width: `${(row.value / most) * 100}%` }}
              />
            </dd>
            <dd className="text-xs text-muted-foreground tabular">
              {t('insights.rhythm.perDay', {
                visits: format.number(Math.round(row.value)),
              })}
            </dd>
          </div>
        ))}
      </dl>
      {busiest.length > 0 && (
        <p className="text-muted-foreground">
          {t('insights.rhythm.busiest')}{' '}
          {busiest.map((entry, index) => (
            <span key={entry.hour}>
              {index > 0 && ', '}
              <span className="font-medium text-foreground tabular">
                {format.hourSpan(entry.hour)}
              </span>
            </span>
          ))}
        </p>
      )}
    </div>
  )
}
