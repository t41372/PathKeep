/** Cards on the Insights screen. Each takes the range and owns its query. */
import { Repeat } from 'lucide-react'
import { useMemo } from 'react'
import { Link } from 'react-router-dom'
import { EvilBarChart } from '@/components/evilcharts/charts/recharts-bar-chart'
import { Favicon } from '@/components/app/favicon'
import { heatLevel } from '@/components/app/heat-level'
import { Heatmap, type HeatCell } from '@/components/app/heatmap'
import { SectionCard } from '@/components/app/section-card'
import { Skeleton } from '@/components/ui/skeleton'
import { cn } from '@/lib/cn'
import type { KpiMetric } from '@/lib/core-intelligence/types'
import {
  useFormat,
  useI18n,
  type Formatters,
  type Translator,
} from '@/lib/i18n'
import {
  rangeDays,
  useDailyActivity,
  useDigest,
  useRefindPages,
  useRhythm,
  useFrequentSearches,
  useTopSites,
  type RangeId,
} from './queries'

function formatActiveTime(ms: number, t: Translator, format: Formatters) {
  const minutes = ms / 60_000
  if (minutes < 90)
    return t('insights.kpi.minutes', {
      value: format.number(Math.round(minutes)),
    })
  return t('insights.kpi.hours', {
    value: format.number(Math.round(minutes / 60)),
  })
}

function KpiCard({
  label,
  metric,
  value,
  hint,
  loading,
}: {
  label: string
  metric?: KpiMetric
  value: string
  hint: string
  loading: boolean
}) {
  const format = useFormat()
  const change = metric?.changePercent
  return (
    <div className="flex flex-col gap-1 rounded-xl border bg-card px-4 py-3.5 shadow-card">
      <span className="text-[13px] text-muted-foreground">{label}</span>
      {loading ? (
        <Skeleton className="my-1 h-6 w-24" />
      ) : (
        <span className="flex items-baseline gap-2">
          <span className="text-[22px] font-semibold tabular">{value}</span>
          {change != null && Number.isFinite(change) && (
            <span
              title={hint}
              className={cn(
                'text-xs tabular',
                change >= 0 ? 'text-green' : 'text-muted-foreground',
              )}
            >
              {format.percent(change / 100, true)}
            </span>
          )}
        </span>
      )}
    </div>
  )
}

export function KpiRow({ range }: { range: RangeId }) {
  const { t } = useI18n()
  const format = useFormat()
  const digest = useDigest(range)
  const data = digest.data?.data
  const hint = t('insights.kpi.vsPrevious', { days: rangeDays[range] })
  const loading = digest.isPending

  return (
    <div className="grid grid-cols-[repeat(auto-fit,minmax(170px,1fr))] gap-3">
      <KpiCard
        label={t('insights.kpi.visits')}
        metric={data?.totalVisits}
        value={format.compact(data?.totalVisits.value ?? 0)}
        hint={hint}
        loading={loading}
      />
      <KpiCard
        label={t('insights.kpi.domains')}
        metric={data?.distinctDomains}
        value={format.number(data?.distinctDomains?.value ?? 0)}
        hint={hint}
        loading={loading}
      />
      <KpiCard
        label={t('insights.kpi.searches')}
        metric={data?.totalSearches}
        value={format.compact(data?.totalSearches.value ?? 0)}
        hint={hint}
        loading={loading}
      />
      <KpiCard
        label={t('insights.kpi.activeTime')}
        metric={data?.activeTimeMs}
        value={formatActiveTime(data?.activeTimeMs?.value ?? 0, t, format)}
        hint={hint}
        loading={loading}
      />
    </div>
  )
}

export function DailyActivityCard({ range }: { range: RangeId }) {
  const { t } = useI18n()
  const format = useFormat()
  const daily = useDailyActivity(range)
  const config = useMemo(
    () => ({
      pages: {
        label: t('insights.daily.pages'),
        colors: { light: ['var(--brand)'], dark: ['var(--brand)'] },
      },
      searches: {
        label: t('insights.daily.searches'),
        colors: { light: ['var(--blue)'], dark: ['var(--blue)'] },
      },
    }),
    [t],
  )
  const points = daily.data?.points ?? []

  return (
    <SectionCard
      title={t('insights.daily.title')}
      action={
        <span className="flex gap-3 text-xs text-muted-foreground">
          <span className="flex items-center gap-1.5">
            <span className="size-2 rounded-[2px] bg-brand" />
            {t('insights.daily.pages')}
          </span>
          <span className="flex items-center gap-1.5">
            <span className="size-2 rounded-[2px] bg-blue" />
            {t('insights.daily.searches')}
          </span>
        </span>
      }
    >
      <div className="h-[220px]">
        <EvilBarChart
          data={points}
          config={config}
          xDataKey="dateKey"
          isLoading={daily.isPending}
          className="aspect-auto h-full"
          barRadius={3}
          barGap={2}
        >
          <EvilBarChart.Grid vertical={false} strokeDasharray="3 4" />
          <EvilBarChart.XAxis
            dataKey="dateKey"
            tickLine={false}
            axisLine={false}
            minTickGap={32}
            tickFormatter={(value: string) =>
              format.monthDay(`${value}T00:00:00`)
            }
            className="font-mono"
          />
          <EvilBarChart.Tooltip />
          <EvilBarChart.Bar dataKey="pages" />
          <EvilBarChart.Bar dataKey="searches" />
        </EvilBarChart>
      </div>
    </SectionCard>
  )
}

function ListSkeleton({ rows }: { rows: number }) {
  return (
    <div className="flex flex-col gap-2">
      {Array.from({ length: rows }, (_, index) => (
        <Skeleton key={index} className="h-8" />
      ))}
    </div>
  )
}

export function TopSitesCard({ range }: { range: RangeId }) {
  const { t } = useI18n()
  const format = useFormat()
  const sites = useTopSites(range)
  const items = sites.data?.data ?? []
  const max = Math.max(1, ...items.map((site) => site.visitCount))

  return (
    <SectionCard title={t('insights.topSites.title')}>
      {sites.isPending ? (
        <ListSkeleton rows={6} />
      ) : items.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          {t('insights.topSites.empty')}
        </p>
      ) : (
        <ul className="flex flex-col gap-1.5">
          {items.map((site) => (
            <li key={site.registrableDomain}>
              <Link
                to={`/history?domain=${encodeURIComponent(site.registrableDomain)}`}
                className="relative flex h-8 items-center gap-2.5 rounded-md px-2 text-[13px] hover:no-underline"
              >
                <span
                  aria-hidden
                  className="absolute inset-y-0 left-0 rounded-md bg-brand-soft transition-[width] duration-500"
                  style={{ width: `${(site.visitCount / max) * 100}%` }}
                />
                <Favicon domain={site.registrableDomain} className="relative" />
                <span className="relative flex-1 truncate font-medium">
                  {site.registrableDomain}
                </span>
                <span className="relative font-mono text-xs text-muted-foreground">
                  {format.compact(site.visitCount)}
                </span>
              </Link>
            </li>
          ))}
        </ul>
      )}
    </SectionCard>
  )
}

export function RhythmCard({ range }: { range: RangeId }) {
  const { t } = useI18n()
  const rhythm = useRhythm(range)
  const weekdayList = t('insights.rhythm.weekdays')
  const weekdays = useMemo(() => weekdayList.split(','), [weekdayList])

  const columns = useMemo(() => {
    const cells = rhythm.data?.data.cells ?? []
    const counts = new Map(
      cells.map((cell) => [`${cell.dow}-${cell.hour}`, cell.visitCount]),
    )
    const max = rhythm.data?.data.maxCount ?? 0
    // Rows run Monday..Sunday; the backend's dow is 0 = Sunday.
    const dows = [1, 2, 3, 4, 5, 6, 0]
    return Array.from({ length: 24 }, (_, hour): HeatCell[] =>
      dows.map((dow, row) => {
        const visits = counts.get(`${dow}-${hour}`) ?? 0
        return {
          key: `${dow}-${hour}`,
          level: heatLevel(visits, max),
          label: t('insights.rhythm.cell', {
            day: weekdays[row] ?? '',
            hour: String(hour).padStart(2, '0'),
            visits: t('common.visits', { count: visits }),
          }),
        }
      }),
    )
  }, [rhythm.data, t, weekdays])

  return (
    <SectionCard
      title={t('insights.rhythm.title')}
      subtitle={t('insights.rhythm.subtitle')}
    >
      {rhythm.isPending ? (
        <Skeleton className="h-[150px]" />
      ) : (
        <div className="overflow-x-auto">
          <Heatmap
            columns={columns}
            cellSize={16}
            gap={3}
            rowLabels={weekdays}
            columnLabels={Array.from({ length: 24 }, (_, hour) =>
              hour % 6 === 0 || hour === 23
                ? String(hour).padStart(2, '0')
                : null,
            )}
          />
        </div>
      )}
    </SectionCard>
  )
}

export function SearchesAndRefindCard({ range }: { range: RangeId }) {
  const { t } = useI18n()
  const format = useFormat()
  const searches = useFrequentSearches(range)
  const refind = useRefindPages(range)
  const concepts = searches.data ?? []
  const pages = refind.data?.data ?? []

  return (
    <SectionCard title={t('insights.searches.title')}>
      {searches.isPending ? (
        <ListSkeleton rows={2} />
      ) : concepts.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          {t('insights.searches.empty')}
        </p>
      ) : (
        <div className="flex flex-wrap gap-1.5">
          {concepts.map((concept) => (
            <Link
              key={concept.query}
              to={`/history?q=${encodeURIComponent(concept.query)}`}
              className="flex h-7 items-center gap-1.5 rounded-full border bg-card px-3 text-[13px] transition-colors hover:bg-muted hover:no-underline"
            >
              {concept.query}
              <span className="font-mono text-[11px] text-muted-foreground">
                {concept.count}
              </span>
            </Link>
          ))}
        </div>
      )}
      <h3 className="mt-3 text-sm font-semibold">
        {t('insights.refind.title')}
      </h3>
      {refind.isPending ? (
        <ListSkeleton rows={3} />
      ) : pages.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          {t('insights.refind.empty')}
        </p>
      ) : (
        <ul className="-mx-2 flex flex-col">
          {pages.map((page, index) => (
            <li key={`${page.canonicalUrl}-${index}`}>
              <Link
                to={`/history?q=${encodeURIComponent(page.title || page.url)}`}
                title={t('insights.refind.days', { count: page.crossDayCount })}
                className="flex items-center gap-2.5 rounded-md px-2 py-1.5 text-[13px] hover:bg-muted hover:no-underline"
              >
                <Repeat
                  className="size-3.5 shrink-0 text-muted-foreground"
                  aria-hidden
                />
                <span className="flex-1 truncate">
                  {page.title || page.url}
                </span>
                <span className="font-mono text-xs text-muted-foreground">
                  {t('insights.refind.times', {
                    count: format.number(page.crossDayCount),
                  })}
                </span>
              </Link>
            </li>
          ))}
        </ul>
      )}
    </SectionCard>
  )
}
