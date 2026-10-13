/** The cards that make up Home. Each one owns its query and loading state. */
import { ChevronLeft, ChevronRight } from 'lucide-react'
import { useMemo, useState } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { EvilAreaChart } from '@/components/evilcharts/charts/recharts-area-chart'
import type { ChartConfig } from '@/components/evilcharts/ui/recharts-chart'
import { heatLevel } from '@/components/app/heat-level'
import { Heatmap, HeatLegend, type HeatCell } from '@/components/app/heatmap'
import { CardError, SectionCard } from '@/components/app/section-card'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { BrowserIcon } from '@/lib/browser-icons'
import { cn } from '@/lib/cn'
import { useFormat, useI18n } from '@/lib/i18n'
import { dayPath, pagePath, searchPath } from '@/features/insights/links'
import { presetScope } from '@/features/insights/range'
import { TopicIcon } from '@/features/insights/topic-icon'
import { BuildingNote } from './building'
import {
  dailySeries,
  noVisits,
  stillBuilding,
  useBrowserSummaries,
} from './home-data'
import {
  useOnThisDay,
  useThirtyDayDigest,
  useThirtyDayTrend,
  useThreads,
  useYearTrend,
} from './queries'
import { useInsightsFormat, useInsightsZone } from '@/features/insights/zone'
import { addDays, dayAsUtcDate, todayIn } from '@/lib/time-zone'

const trendConfig = {
  visits: {
    label: 'Visits',
    colors: { light: ['var(--brand)'], dark: ['var(--brand)'] },
  },
} satisfies ChartConfig

export function TrendCard() {
  const { t } = useI18n()
  const format = useInsightsFormat()
  const today = todayIn(useInsightsZone().shown)
  const trend = useThirtyDayTrend()
  const digest = useThirtyDayDigest()
  const building = stillBuilding(trend.data, noVisits)
  const series = useMemo(
    () => dailySeries(trend.data?.data.points ?? [], 30, today),
    [trend.data, today],
  )
  const change = digest.data?.data.totalVisits.changePercent
  const config = useMemo(
    () => ({
      visits: { ...trendConfig.visits, label: t('home.trend.visits') },
    }),
    [t],
  )

  return (
    <SectionCard
      title={t('home.trend.title')}
      subtitle={t('home.trend.subtitle')}
      action={
        change != null && Number.isFinite(change) ? (
          <span
            title={t('home.trend.vsPrevious')}
            className="rounded-md bg-brand-soft px-2 py-0.5 text-xs font-medium text-brand tabular"
          >
            {format.percent(change / 100, true)}
          </span>
        ) : null
      }
    >
      <div className="h-[210px]">
        {trend.isError ? (
          <CardError error={trend.error} onRetry={() => void trend.refetch()} />
        ) : (
          <EvilAreaChart
            data={series}
            config={config}
            xDataKey="dateKey"
            isLoading={trend.isPending || building}
            className="h-full aspect-auto"
            curveType="monotone"
          >
            {/* The prototype's fill is stronger than evilcharts' built-in fade. */}
            <defs>
              <linearGradient id="home-trend-fill" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stopColor="var(--brand)" stopOpacity={0.35} />
                <stop offset="100%" stopColor="var(--brand)" stopOpacity={0} />
              </linearGradient>
            </defs>
            <EvilAreaChart.Grid vertical={false} strokeDasharray="3 4" />
            <EvilAreaChart.XAxis
              dataKey="dateKey"
              tickLine={false}
              axisLine={false}
              interval={6}
              padding={{ left: 14, right: 14 }}
              tickFormatter={(value: string) =>
                format.day(value, { month: 'short', day: 'numeric' })
              }
              className="font-mono"
            />
            <EvilAreaChart.Tooltip />
            <EvilAreaChart.Area
              dataKey="visits"
              variant="gradient"
              strokeVariant="solid"
              strokeWidth={2}
              areaProps={{ dataKey: 'visits', fill: 'url(#home-trend-fill)' }}
            >
              <EvilAreaChart.ActiveDot variant="colored-border" />
            </EvilAreaChart.Area>
          </EvilAreaChart>
        )}
      </div>
      {building && <BuildingNote result={trend.data} />}
    </SectionCard>
  )
}

export function OnThisDayCard() {
  const { t } = useI18n()
  const format = useInsightsFormat()
  const today = todayIn(useInsightsZone().shown)
  const onThisDay = useOnThisDay()
  const building = stillBuilding(onThisDay.data, (days) =>
    days.every((day) => day.totalVisits === 0),
  )
  const entries = (onThisDay.data?.data ?? [])
    .filter((entry) => entry.totalVisits > 0)
    .slice(0, 4)

  return (
    <SectionCard
      title={t('home.onThisDay.title')}
      subtitle={t('home.onThisDay.subtitle', {
        date: format.day(today, { month: 'short', day: 'numeric' }),
      })}
    >
      {onThisDay.isError ? (
        <CardError
          error={onThisDay.error}
          onRetry={() => void onThisDay.refetch()}
        />
      ) : onThisDay.isPending || building ? (
        <div className="flex flex-col gap-3">
          {[0, 1, 2].map((index) => (
            <Skeleton key={index} className="h-10" />
          ))}
          {building && <BuildingNote result={onThisDay.data} />}
        </div>
      ) : entries.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          {t('home.onThisDay.empty')}
        </p>
      ) : (
        <ul className="-mx-2 flex flex-col">
          {entries.map((entry) => (
            <li key={entry.year}>
              <Link
                to={`/history?date=${entry.date}`}
                className="flex items-start gap-3 rounded-lg p-2 transition-colors hover:bg-muted"
              >
                <span className="min-w-[38px] pt-0.5 font-mono text-xs font-medium text-brand">
                  {entry.year}
                </span>
                <span className="flex min-w-0 flex-col gap-0.5">
                  <span className="leading-snug font-medium">
                    {entry.summary ||
                      entry.topDomains.slice(0, 2).join('、') ||
                      entry.date}
                  </span>
                  <span className="truncate text-xs text-muted-foreground">
                    {t('home.onThisDay.meta', {
                      visits: t('common.visits', { count: entry.totalVisits }),
                      domains: entry.topDomains.slice(0, 3).join(', '),
                    })}
                  </span>
                </span>
              </Link>
            </li>
          ))}
        </ul>
      )}
    </SectionCard>
  )
}

/** Monday-first week columns covering one calendar year, up to `today`. */
function yearColumns(
  year: number,
  today: string,
  counts: Map<string, number>,
  label: (dateKey: string, visits: number) => string,
) {
  const max = Math.max(1, ...counts.values())
  const first = `${year}-01-01`
  const offset = (dayAsUtcDate(first).getUTCDay() + 6) % 7
  const columns: HeatCell[][] = []
  for (let week = 0; week < 54; week += 1) {
    const column: HeatCell[] = []
    for (let row = 0; row < 7; row += 1) {
      const dateKey = addDays(first, week * 7 + row - offset)
      const outside = !dateKey.startsWith(`${year}-`) || dateKey > today
      const visits = counts.get(dateKey) ?? 0
      column.push(
        outside
          ? { key: `${week}-${row}`, level: 0, label: '', empty: true }
          : {
              key: dateKey,
              level: heatLevel(visits, max),
              label: label(dateKey, visits),
            },
      )
    }
    if (column.some((cell) => !cell.empty)) columns.push(column)
  }
  return columns
}

export function YearCard() {
  const { t } = useI18n()
  const format = useInsightsFormat()
  const navigate = useNavigate()
  const today = todayIn(useInsightsZone().shown)
  const currentYear = Number(today.slice(0, 4))
  const [year, setYear] = useState(currentYear)
  const trend = useYearTrend(year)
  const building = stillBuilding(trend.data, noVisits)
  const years = trend.data?.data.availableYears ?? [currentYear]
  const earliest = Math.min(currentYear, ...years)

  const points = trend.data?.data.points
  const { columns, total, busiest } = useMemo(() => {
    const counts = new Map(
      (points ?? []).map((point) => [point.dateKey, point.totalVisits]),
    )
    let best: { dateKey: string; visits: number } | null = null
    let sum = 0
    for (const [dateKey, visits] of counts) {
      sum += visits
      if (!best || visits > best.visits) best = { dateKey, visits }
    }
    const label = (dateKey: string, visits: number) =>
      visits > 0
        ? t('home.year.cell', {
            date: format.day(dateKey),
            visits: t('common.visits', { count: visits }),
          })
        : t('home.year.cellEmpty', { date: format.day(dateKey) })
    return {
      columns: yearColumns(year, today, counts, label),
      total: sum,
      busiest: best,
    }
  }, [points, year, today, t, format])

  return (
    <SectionCard
      title={t('home.year.title')}
      subtitle={t('home.year.subtitle')}
      action={
        <div className="flex items-center gap-1">
          <Button
            variant="outline"
            size="icon"
            className="size-7"
            aria-label={t('home.year.previous')}
            disabled={year <= earliest}
            onClick={() => setYear((value) => value - 1)}
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
            aria-label={t('home.year.next')}
            disabled={year >= currentYear}
            onClick={() => setYear((value) => value + 1)}
          >
            <ChevronRight />
          </Button>
        </div>
      }
    >
      <div
        className={cn(
          'overflow-x-auto pb-1 transition-opacity',
          trend.isFetching && 'opacity-60',
        )}
      >
        {trend.isError ? (
          <CardError error={trend.error} onRetry={() => void trend.refetch()} />
        ) : trend.isPending || building ? (
          <Skeleton className="h-[110px] w-full" />
        ) : (
          <Heatmap
            columns={columns}
            label={`${t('home.year.title')} · ${year}`}
            onCellClick={(date) => void navigate(dayPath(date))}
          />
        )}
      </div>
      <div className="flex flex-wrap items-center justify-between gap-2 text-xs text-muted-foreground">
        {building ? (
          <BuildingNote result={trend.data} />
        ) : trend.isPending ? (
          <Skeleton className="h-3.5 w-48" />
        ) : (
          <span className="tabular">
            {t('home.year.summary', {
              total: format.number(total),
              year: String(year),
            })}
            {busiest &&
              busiest.visits > 0 &&
              ` · ${t('home.year.busiest', { date: format.day(busiest.dateKey, { month: 'short', day: 'numeric' }) })}`}
          </span>
        )}
        <HeatLegend less={t('common.less')} more={t('common.more')} />
      </div>
    </SectionCard>
  )
}

export function ThreadsCard() {
  const { t } = useI18n()
  const format = useInsightsFormat()
  const zone = useInsightsZone().shown
  const threads = useThreads()
  const building = stillBuilding(
    threads.data,
    (page) => page.items.length === 0,
  )
  const items = (threads.data?.data.items ?? [])
    .slice()
    .sort((a, b) => b.lastSeenAt.localeCompare(a.lastSeenAt))
    .slice(0, 4)

  return (
    <SectionCard
      title={t('home.threads.title')}
      action={
        <Link
          to="/insights"
          className="text-[13px] text-muted-foreground hover:text-foreground"
        >
          {t('home.threads.all')} →
        </Link>
      }
    >
      {threads.isError ? (
        <CardError
          error={threads.error}
          onRetry={() => void threads.refetch()}
        />
      ) : threads.isPending || building ? (
        <div className="flex flex-col gap-3">
          {[0, 1, 2, 3].map((index) => (
            <Skeleton key={index} className="h-9" />
          ))}
          {building && <BuildingNote result={threads.data} />}
        </div>
      ) : items.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          {t('home.threads.empty')}
        </p>
      ) : (
        <ul className="-mx-2 flex flex-col">
          {items.map((thread) => (
            <li key={thread.investigationId}>
              <Link
                to={
                  thread.anchorType === 'query_family'
                    ? searchPath(thread.anchorLabel, presetScope('d90', zone))
                    : pagePath(thread.anchorId, presetScope('d90', zone))
                }
                className="flex items-center gap-3 rounded-lg p-2 transition-colors hover:bg-muted"
              >
                <TopicIcon topic={thread} />
                <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                  <span className="truncate font-medium">
                    {thread.anchorLabel}
                  </span>
                  <span className="text-xs text-muted-foreground">
                    {t('home.threads.meta', {
                      days: t('home.threads.days', {
                        count: thread.distinctDays,
                      }),
                      visits: t('common.visits', {
                        count: thread.occurrenceCount,
                      }),
                    })}
                  </span>
                </span>
                <span className="font-mono text-xs text-muted-foreground">
                  {format.shortWhen(thread.lastSeenAt)}
                </span>
              </Link>
            </li>
          ))}
        </ul>
      )}
    </SectionCard>
  )
}

export function SourcesCard() {
  const { t } = useI18n()
  const format = useFormat()
  const { list, loading, failed } = useBrowserSummaries()
  const max = Math.max(1, ...list.map((item) => item.visits))

  return (
    <SectionCard
      title={t('home.sources.title')}
      action={
        <Link
          to="/backup"
          className="text-[13px] text-muted-foreground hover:text-foreground"
        >
          {t('home.sources.manage')} →
        </Link>
      }
    >
      {list.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          {t('home.sources.empty')}
        </p>
      ) : (
        <ul className="flex flex-col gap-3">
          {list.map((item) => (
            <li
              key={item.browserName}
              className={cn(
                'flex items-center gap-3',
                !item.selected && 'opacity-50',
              )}
              title={item.selected ? undefined : t('home.sources.paused')}
            >
              <BrowserIcon
                browserName={item.browserName}
                className="size-5"
                decorative
              />
              <span className="flex-1 truncate font-medium">
                {item.browserName}
              </span>
              {loading ? (
                <Skeleton className="h-3 w-24" />
              ) : failed ? (
                <span className="font-mono text-xs text-muted-foreground">
                  —
                </span>
              ) : (
                <>
                  <span className="font-mono text-xs text-muted-foreground tabular">
                    {format.number(item.visits)}
                  </span>
                  <span className="h-1 w-16 overflow-hidden rounded-full bg-muted">
                    <span
                      className="block h-full rounded-full bg-foreground/80"
                      style={{ width: `${(item.visits / max) * 100}%` }}
                    />
                  </span>
                </>
              )}
            </li>
          ))}
        </ul>
      )}
    </SectionCard>
  )
}
