/**
 * The year (`agent-history.md` §2.3): every day's Agent work time as a
 * calendar, and that year's six figures with a bar per month. It is the
 * one part the time range does not change (the tool and project filters
 * do), and the calendar keeps its own width, so it sits apart from the
 * wall. A day opens its conversations in History.
 */
import { ChevronLeft, ChevronRight } from 'lucide-react'
import { useMemo, type ReactNode } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { heatLevel } from '@/components/app/heat-level'
import { HeatLegend, Heatmap, type HeatCell } from '@/components/app/heatmap'
import { CardError, SectionCard } from '@/components/app/section-card'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import type { InsightsCalendar } from '@/lib/backend-client/agent-insights'
import { cn } from '@/lib/cn'
import { useI18n, type Formatters, type Translator } from '@/lib/i18n'
import { dayAsUtcDate } from '@/lib/time-zone'
import { useInsightsFormat } from '@/features/insights/zone'
import { historyOnDay, type Scope } from './scope'
import type { Shown } from './use-report'
import { hours } from './words'

type Filters = Pick<Scope, 'harnesses' | 'projectId'>
type Day = InsightsCalendar['days'][number]

const NONE = '—'

function dayLabel(t: Translator, f: Formatters, day: Day) {
  if (day.active && day.busyMs === null)
    return t('agentInsights.year.cellUnknown', { day: day.day })
  if (!day.busyMs) return t('agentInsights.year.cellNone', { day: day.day })
  return t('agentInsights.year.cell', {
    day: day.day,
    time: hours(t, f, day.busyMs),
  })
}

/** Monday-first weeks as columns; days before 1 January and after today are blank. */
function yearColumns(days: Day[], label: (day: Day) => string): HeatCell[][] {
  const max = Math.max(0, ...days.map((day) => day.busyMs ?? 0))
  const blank = (key: string): HeatCell => ({
    key,
    level: 0,
    label: '',
    empty: true,
  })
  const columns: HeatCell[][] = []
  const offset = days.length
    ? (dayAsUtcDate(days[0].day).getUTCDay() + 6) % 7
    : 0
  let column = Array.from({ length: offset }, (_, i) => blank(`lead-${i}`))
  for (const day of days) {
    column.push(
      day.future
        ? blank(day.day)
        : {
            key: day.day,
            level: heatLevel(day.busyMs ?? 0, max),
            label: label(day),
          },
    )
    if (column.length === 7) {
      columns.push(column)
      column = []
    }
  }
  if (column.length) {
    while (column.length < 7) column.push(blank(`tail-${column.length}`))
    columns.push(column)
  }
  return columns.filter((week) => week.some((cell) => !cell.empty))
}

function Stepper({
  year,
  first,
  last,
  onYear,
}: {
  year: number
  first: number
  last: number
  onYear: (year: number) => void
}) {
  const { t } = useI18n()
  return (
    <div className="flex items-center gap-1">
      <Button
        variant="outline"
        size="icon"
        className="size-7"
        aria-label={t('agentInsights.year.previous')}
        disabled={year <= first}
        onClick={() => onYear(year - 1)}
      >
        <ChevronLeft />
      </Button>
      <span className="px-2 font-mono text-[13px] font-medium tabular">
        {year}
      </span>
      <Button
        variant="outline"
        size="icon"
        className="size-7"
        aria-label={t('agentInsights.year.next')}
        disabled={year >= last}
        onClick={() => onYear(year + 1)}
      >
        <ChevronRight />
      </Button>
    </div>
  )
}

function Fact({
  label,
  value,
  sub,
}: {
  label: string
  value: ReactNode
  sub?: ReactNode
}) {
  return (
    <div className="flex min-w-0 flex-col gap-0.5">
      <dt className="text-xs text-muted-foreground">{label}</dt>
      <dd className="text-[17px] font-semibold tabular">{value}</dd>
      {sub && (
        <dd className="truncate text-xs text-muted-foreground tabular">
          {sub}
        </dd>
      )}
    </div>
  )
}

function Facts({
  calendar,
  filters,
}: {
  calendar: InsightsCalendar
  filters: Filters
}) {
  const { t } = useI18n()
  const f = useInsightsFormat()
  const time = (ms: number | null) => (ms === null ? NONE : hours(t, f, ms))
  const short = (day: string) => f.day(day, { month: 'short', day: 'numeric' })
  const monthName = (month: number) =>
    f.day(`${calendar.year}-${String(month).padStart(2, '0')}-01`, {
      month: 'long',
    })
  const busiestDay = calendar.busiestDay
  const busiest = busiestDay
    ? calendar.days.find((day) => day.day === busiestDay)
    : undefined
  const peakMonth = calendar.busiestMonth
  const peak = peakMonth ? calendar.months[peakMonth - 1] : undefined
  const max = Math.max(1e-9, ...calendar.months.map((m) => m.busyMs ?? 0))
  return (
    <>
      <div className="@container">
        <dl className="grid grid-cols-2 gap-x-6 gap-y-4 @min-[400px]:grid-cols-3 @min-[820px]:grid-cols-6">
          <Fact
            label={t('agentInsights.year.work')}
            value={time(calendar.busyMs)}
            sub={t('agentInsights.year.workSub')}
          />
          <Fact
            label={t('agentInsights.year.activeDays')}
            value={t('agentInsights.year.days', { count: calendar.activeDays })}
            sub={t(
              calendar.year === calendar.currentYear
                ? 'agentInsights.year.soFar'
                : 'agentInsights.year.whole',
              { count: calendar.elapsedDays },
            )}
          />
          <Fact
            label={t('agentInsights.year.average')}
            value={time(
              calendar.averageBusyMs === null
                ? null
                : Math.round(calendar.averageBusyMs),
            )}
            sub={t('agentInsights.year.averageSub')}
          />
          <Fact
            label={t('agentInsights.year.streak')}
            value={t('agentInsights.year.days', {
              count: calendar.longestStreak,
            })}
            sub={
              calendar.longestStreak > 1 &&
              calendar.streakStart &&
              calendar.streakEnd
                ? `${short(calendar.streakStart)} – ${short(calendar.streakEnd)}`
                : undefined
            }
          />
          <Fact
            label={t('agentInsights.year.busiestDay')}
            value={time(busiest?.busyMs ?? null)}
            sub={
              busiestDay ? (
                <Link
                  to={historyOnDay(filters, busiestDay)}
                  className="hover:text-foreground hover:underline"
                >
                  {short(busiestDay)} →
                </Link>
              ) : undefined
            }
          />
          <Fact
            label={t('agentInsights.year.busiestMonth')}
            value={peakMonth ? monthName(peakMonth) : NONE}
            sub={peak ? time(peak.busyMs) : undefined}
          />
        </dl>
      </div>
      <div
        role="img"
        aria-label={t('agentInsights.year.months', { year: calendar.year })}
        className="flex min-h-[48px] flex-1 flex-col gap-1.5"
      >
        <div className="flex flex-1 items-end gap-1.5 border-b border-foreground/10">
          {calendar.months.map((month) => {
            const name = monthName(month.month)
            const title = month.future
              ? t('agentInsights.year.monthFuture', { month: name })
              : month.busyMs === null
                ? t('agentInsights.year.monthUnknown', { month: name })
                : t('agentInsights.year.month', {
                    month: name,
                    time: hours(t, f, month.busyMs),
                  })
            return (
              <span
                key={month.month}
                title={title}
                className="flex h-full min-w-0 flex-1 items-end"
              >
                <span
                  className={cn(
                    'block h-full w-full origin-bottom rounded-t-[3px] transition-transform duration-500 ease-out',
                    month.month === peakMonth ? 'bg-heat-4' : 'bg-heat-3',
                    month.future && 'bg-transparent',
                  )}
                  style={{
                    transform: `scaleY(${Math.max(0.02, (month.busyMs ?? 0) / max)})`,
                  }}
                />
              </span>
            )
          })}
        </div>
        <div className="flex gap-1.5 text-[11px] text-muted-foreground tabular">
          {calendar.months.map((month) => (
            <span key={month.month} className="flex-1 text-center">
              {month.month}
            </span>
          ))}
        </div>
      </div>
    </>
  )
}

export function YearSection({
  calendar,
  failed,
  filters,
  onYear,
  onRetry,
}: {
  calendar: Shown<InsightsCalendar>
  failed: string | null
  filters: Filters
  onYear: (year: number) => void
  onRetry: () => void
}) {
  const { t } = useI18n()
  const f = useInsightsFormat()
  const navigate = useNavigate()
  const value = calendar?.value
  const columns = useMemo(
    () => (value ? yearColumns(value.days, (day) => dayLabel(t, f, day)) : []),
    [value, t, f],
  )
  const stale = calendar?.stale ?? false
  const error = failed !== null && (
    <CardError error={new Error(failed)} onRetry={onRetry} />
  )

  return (
    <div
      aria-busy={!value || stale}
      className={cn(
        'flex flex-wrap items-stretch gap-4 transition-opacity duration-200',
        stale && 'opacity-60',
      )}
    >
      <div className="flex w-fit max-w-full flex-col">
        <SectionCard
          title={t('agentInsights.year.title')}
          subtitle={t('agentInsights.year.subtitle')}
          action={
            value && (
              <Stepper
                year={value.year}
                first={value.firstYearWithData ?? value.currentYear}
                last={value.currentYear}
                onYear={onYear}
              />
            )
          }
          className="flex-1"
        >
          {error ||
            (value ? (
              <>
                <div className="overflow-x-auto pb-1">
                  <Heatmap
                    columns={columns}
                    label={t('agentInsights.year.grid', { year: value.year })}
                    onCellClick={(day) =>
                      void navigate(historyOnDay(filters, day))
                    }
                  />
                </div>
                <div className="flex justify-end">
                  <HeatLegend
                    less={t('agentInsights.year.less')}
                    more={t('agentInsights.year.more')}
                  />
                </div>
              </>
            ) : (
              <Skeleton className="h-[112px] w-[min(100%,858px)]" />
            ))}
        </SectionCard>
      </div>
      <div className="flex min-w-[300px] flex-1 flex-col">
        <SectionCard
          title={
            value ? t('agentInsights.year.facts', { year: value.year }) : ''
          }
          className="flex-1"
        >
          {error ||
            (value ? (
              <Facts calendar={value} filters={filters} />
            ) : (
              <Skeleton className="h-[140px]" />
            ))}
        </SectionCard>
      </div>
    </div>
  )
}
