/**
 * The first cards on the Insights screen: KPIs, daily activity, top sites,
 * rhythm, searches and pages reopened. Each takes the range and owns its
 * query; each links into a drill-in.
 */
import { Repeat } from 'lucide-react'
import { useMemo } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import type { MouseHandlerDataParam } from 'recharts'
import { EvilBarChart } from '@/components/evilcharts/charts/recharts-bar-chart'
import { Favicon } from '@/components/app/favicon'
import { heatLevel } from '@/components/app/heat-level'
import { Heatmap, type HeatCell } from '@/components/app/heatmap'
import { CardError, SectionCard } from '@/components/app/section-card'
import { Skeleton } from '@/components/ui/skeleton'
import { useFormat, useI18n } from '@/lib/i18n'
import { dayPath, pagePath, searchPath, sitePath } from './links'
import { formatDuration, rowLink, shareBar } from './display'
import { KpiTile, ListSkeleton, QueryBody, SectionStatus } from './parts'
import {
  rangeDays,
  useDailyActivity,
  useDigest,
  useFrequentSearches,
  useRefindPages,
  useRhythm,
  useTopSites,
  type RangeId,
} from './queries'

export function KpiRow({ range }: { range: RangeId }) {
  const { t } = useI18n()
  const format = useFormat()
  const digest = useDigest(range)
  const data = digest.data?.data
  const hint = t('insights.kpi.vsPrevious', { days: rangeDays[range] })
  const loading = data === undefined && !digest.isError

  if (digest.isError && data === undefined) {
    return (
      <div className="rounded-xl border bg-card px-4 py-3.5 shadow-card">
        <CardError error={digest.error} onRetry={() => void digest.refetch()} />
      </div>
    )
  }
  return (
    <div className="grid grid-cols-[repeat(auto-fit,minmax(170px,1fr))] gap-3">
      <KpiTile
        label={t('insights.kpi.visits')}
        metric={data?.totalVisits}
        value={format.compact(data?.totalVisits.value ?? 0)}
        hint={hint}
        loading={loading}
      />
      <KpiTile
        label={t('insights.kpi.domains')}
        metric={data?.distinctDomains}
        value={format.number(data?.distinctDomains?.value ?? 0)}
        hint={hint}
        loading={loading}
      />
      <KpiTile
        label={t('insights.kpi.searches')}
        metric={data?.totalSearches}
        value={format.compact(data?.totalSearches.value ?? 0)}
        hint={hint}
        loading={loading}
      />
      <KpiTile
        label={t('insights.kpi.activeTime')}
        metric={data?.activeTimeMs}
        value={formatDuration(data?.activeTimeMs?.value ?? 0, t, format)}
        hint={hint}
        loading={loading}
      />
    </div>
  )
}

export function DailyActivityCard({ range }: { range: RangeId }) {
  const { t } = useI18n()
  const format = useFormat()
  const navigate = useNavigate()
  const daily = useDailyActivity(range)
  const weekly = range === 'y1'
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
  const openDay = (state: MouseHandlerDataParam) => {
    if (!weekly && typeof state.activeLabel === 'string') {
      void navigate(dayPath(state.activeLabel))
    }
  }

  return (
    <SectionCard
      title={t('insights.daily.title')}
      subtitle={weekly ? undefined : t('insights.daily.hint')}
      action={
        <span className="flex items-center gap-3 text-xs text-muted-foreground">
          <span className="flex items-center gap-1.5">
            <span className="size-2 rounded-[2px] bg-brand" />
            {t('insights.daily.pages')}
          </span>
          <span className="flex items-center gap-1.5">
            <span className="size-2 rounded-[2px] bg-blue" />
            {t('insights.daily.searches')}
          </span>
          <SectionStatus meta={daily.data?.meta} />
        </span>
      }
    >
      <div className="h-[220px]">
        {daily.isError && daily.data === undefined ? (
          <CardError error={daily.error} onRetry={() => void daily.refetch()} />
        ) : (
          <EvilBarChart
            data={points}
            config={config}
            xDataKey="dateKey"
            isLoading={daily.data === undefined}
            className="aspect-auto h-full"
            barRadius={3}
            barGap={2}
            chartProps={{
              onClick: openDay,
              style: weekly ? undefined : { cursor: 'pointer' },
            }}
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
        )}
      </div>
    </SectionCard>
  )
}

export function TopSitesCard({ range }: { range: RangeId }) {
  const { t } = useI18n()
  const format = useFormat()
  const sites = useTopSites(range)

  return (
    <SectionCard
      title={t('insights.topSites.title')}
      action={<SectionStatus meta={sites.data?.meta} />}
    >
      <QueryBody
        query={sites}
        skeleton={<ListSkeleton rows={6} />}
        isEmpty={(result) => result.data.length === 0}
        empty={t('insights.topSites.empty')}
      >
        {(result) => {
          const max = Math.max(1, ...result.data.map((site) => site.visitCount))
          return (
            <ul className="flex flex-col gap-1.5">
              {result.data.map((site) => (
                <li key={site.registrableDomain}>
                  <Link
                    to={sitePath(site.registrableDomain, range)}
                    className="relative flex h-8 items-center gap-2.5 rounded-md px-2 text-[13px] hover:no-underline"
                  >
                    <span
                      aria-hidden
                      className="absolute inset-y-0 left-0 rounded-md bg-brand-soft transition-[width] duration-500"
                      style={shareBar(site.visitCount, max)}
                    />
                    <Favicon
                      domain={site.registrableDomain}
                      className="relative"
                    />
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
          )
        }}
      </QueryBody>
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
      action={<SectionStatus meta={rhythm.data?.meta} />}
    >
      {rhythm.isError && rhythm.data === undefined ? (
        <CardError error={rhythm.error} onRetry={() => void rhythm.refetch()} />
      ) : rhythm.data === undefined ? (
        <Skeleton className="h-[150px]" />
      ) : (
        <div className="overflow-x-auto">
          <Heatmap
            columns={columns}
            label={t('insights.rhythm.subtitle')}
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

  return (
    <SectionCard
      title={t('insights.searches.title')}
      action={<SectionStatus meta={searches.data?.meta} />}
    >
      <QueryBody
        query={searches}
        skeleton={<ListSkeleton rows={2} />}
        isEmpty={(result) => result.data.length === 0}
        empty={t('insights.searches.empty')}
      >
        {(result) => (
          <div className="flex flex-wrap gap-1.5">
            {result.data.map((concept) => (
              <Link
                key={concept.query}
                to={searchPath(concept.query, range)}
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
      </QueryBody>
      <div className="mt-3 flex items-center justify-between gap-2">
        <h3 className="text-sm font-semibold">{t('insights.refind.title')}</h3>
        <SectionStatus
          meta={refind.data?.meta}
          explain={t('insights.refind.explain')}
        />
      </div>
      <QueryBody
        query={refind}
        skeleton={<ListSkeleton rows={3} />}
        isEmpty={(result) => result.data.length === 0}
        empty={t('insights.refind.empty')}
      >
        {(result) => (
          <ul className="-mx-2 flex flex-col">
            {result.data.map((page) => (
              <li key={page.canonicalUrl}>
                <Link
                  to={pagePath(page.canonicalUrl, range, page.profileId)}
                  className={rowLink}
                >
                  <Repeat
                    className="size-3.5 shrink-0 text-muted-foreground"
                    aria-hidden
                  />
                  <span className="flex-1 truncate">
                    {page.title || page.url}
                  </span>
                  <span className="text-xs text-muted-foreground tabular">
                    {t('insights.refind.days', {
                      count: page.crossDayCount,
                      days: format.number(page.crossDayCount),
                    })}
                  </span>
                </Link>
              </li>
            ))}
          </ul>
        )}
      </QueryBody>
    </SectionCard>
  )
}
