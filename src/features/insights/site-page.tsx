/**
 * One site over the selected range: visits, the days it was visited, its
 * most visited pages and the searches that ended on it. Reached from Top
 * sites, Habits and the search drill-in; links onward to the day drill-in
 * and History.
 *
 * Reads: `get_domain_deep_dive` (rollups for totals and the trend, so the
 * numbers match Top sites; index seeks for pages and searches).
 */
import { Search } from 'lucide-react'
import { useMemo } from 'react'
import { Link, useNavigate, useParams } from 'react-router-dom'
import type { MouseHandlerDataParam } from 'recharts'
import { EvilBarChart } from '@/components/evilcharts/charts/recharts-bar-chart'
import { Favicon } from '@/components/app/favicon'
import { PageScroll, SectionCard } from '@/components/app/section-card'
import { Button } from '@/components/ui/button'
import type { DomainTrendPoint } from '@/lib/core-intelligence/types'
import { useFormat, useI18n } from '@/lib/i18n'
import { useSite } from './drill-queries'
import {
  dayPath,
  historyDateParam,
  historyPath,
  insightsPath,
  searchPath,
} from './links'
import { rowLink, shareBar } from './display'
import {
  DrillHeader,
  KpiTile,
  ListSkeleton,
  QueryBody,
  SectionStatus,
  RangeToggle,
} from './parts'
import { rangeDates, rangeDays, useRange, type RangeId } from './range'
import { localDateKey } from '@/lib/backend-client/insights'

/** Every day of the range, zero where the site was not visited. */
function fillDays(points: DomainTrendPoint[], range: RangeId) {
  const counts = new Map(
    points.map((point) => [point.dateKey, point.visitCount]),
  )
  const { start } = rangeDates(range)
  const [year, month, day] = start.split('-').map(Number)
  const cursor = new Date(year, month - 1, day)
  return Array.from({ length: rangeDays[range] }, () => {
    const key = localDateKey(cursor)
    cursor.setDate(cursor.getDate() + 1)
    return { dateKey: key, visits: counts.get(key) ?? 0 }
  })
}

/** Monday-start weeks, for the one-year range. */
function toWeeks(days: { dateKey: string; visits: number }[]) {
  const weeks: { dateKey: string; visits: number }[] = []
  for (const day of days) {
    const [year, month, date] = day.dateKey.split('-').map(Number)
    const weekday = (new Date(year, month - 1, date).getDay() + 6) % 7
    if (weekday === 0 || weeks.length === 0) {
      weeks.push({ dateKey: day.dateKey, visits: 0 })
    }
    weeks[weeks.length - 1].visits += day.visits
  }
  return weeks
}

export default function SitePage() {
  const { t } = useI18n()
  const format = useFormat()
  const { domain = '' } = useParams()
  const [range, setRange] = useRange()
  const site = useSite(domain, range)
  const data = site.data?.data
  const loading = data === undefined && !site.isError

  return (
    <PageScroll wide>
      <DrillHeader
        back={insightsPath(range)}
        eyebrow={t('insights.site.eyebrow')}
        title={domain}
        icon={
          <Favicon domain={domain} className="size-10 rounded-xl text-base" />
        }
        actions={
          <>
            <RangeToggle value={range} onChange={setRange} />
            <Button variant="outline" size="sm" asChild>
              <Link to={historyPath({ domain, date: historyDateParam(range) })}>
                {t('insights.openInHistory')}
              </Link>
            </Button>
          </>
        }
      />
      <div className="grid grid-cols-[repeat(auto-fit,minmax(170px,1fr))] gap-3">
        <KpiTile
          label={t('insights.kpi.visits')}
          value={format.number(data?.totalVisits ?? 0)}
          loading={loading}
        />
        <KpiTile
          label={t('insights.site.activeDays')}
          value={format.number(data?.activeDays ?? 0)}
          detail={t('insights.site.ofDays', { days: rangeDays[range] })}
          loading={loading}
        />
        <KpiTile
          label={t('insights.site.pages')}
          value={format.number(data?.pageCount ?? 0)}
          loading={loading}
        />
        <KpiTile
          label={t('insights.site.landings')}
          value={format.number(data?.landingSearchCount ?? 0)}
          detail={t('insights.site.landingsHint')}
          loading={loading}
        />
      </div>
      <SiteTrendCard domain={domain} range={range} />
      <div className="grid grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)] gap-4 max-[900px]:grid-cols-1">
        <SectionCard title={t('insights.site.topPages')}>
          <QueryBody
            query={site}
            skeleton={<ListSkeleton rows={6} />}
            isEmpty={(result) => result.data.topPages.length === 0}
            empty={t('insights.site.empty')}
          >
            {(result) => {
              const max = Math.max(
                1,
                ...result.data.topPages.map((page) => page.visitCount),
              )
              return (
                <ul className="flex flex-col gap-1.5">
                  {result.data.topPages.map((page) => (
                    <li key={page.url}>
                      <Link
                        to={historyPath({ q: page.title || page.path, domain })}
                        title={page.url}
                        className="relative flex h-9 items-center gap-2.5 rounded-md px-2 text-[13px] hover:no-underline"
                      >
                        <span
                          aria-hidden
                          className="absolute inset-y-0 left-0 rounded-md bg-brand-soft"
                          style={shareBar(page.visitCount, max)}
                        />
                        <span className="relative flex min-w-0 flex-1 flex-col">
                          <span className="truncate font-medium">
                            {page.title || page.path}
                          </span>
                          {page.title && (
                            <span className="truncate font-mono text-[11px] text-muted-foreground">
                              {page.path}
                            </span>
                          )}
                        </span>
                        <span className="relative font-mono text-xs text-muted-foreground">
                          {format.number(page.visitCount)}
                        </span>
                      </Link>
                    </li>
                  ))}
                </ul>
              )
            }}
          </QueryBody>
        </SectionCard>
        <SectionCard
          title={t('insights.site.searches')}
          subtitle={t('insights.site.searchesHint')}
        >
          <QueryBody
            query={site}
            skeleton={<ListSkeleton rows={4} />}
            isEmpty={(result) => result.data.landingSearches.length === 0}
            empty={t('insights.site.noSearches')}
          >
            {(result) => (
              <ul className="-mx-2 flex flex-col">
                {result.data.landingSearches.map((search) => (
                  <li key={search.query}>
                    <Link
                      to={searchPath(search.query, range)}
                      className={rowLink}
                    >
                      <Search
                        className="size-3.5 shrink-0 text-muted-foreground"
                        aria-hidden
                      />
                      <span className="flex-1 truncate">{search.query}</span>
                      <span className="font-mono text-xs text-muted-foreground">
                        {t('insights.times', {
                          count: format.number(search.count),
                        })}
                      </span>
                    </Link>
                  </li>
                ))}
              </ul>
            )}
          </QueryBody>
        </SectionCard>
      </div>
    </PageScroll>
  )
}

function SiteTrendCard({ domain, range }: { domain: string; range: RangeId }) {
  const { t } = useI18n()
  const format = useFormat()
  const navigate = useNavigate()
  const site = useSite(domain, range)
  const weekly = range === 'y1'
  const points = useMemo(() => {
    const days = fillDays(site.data?.data.visitTrend ?? [], range)
    return weekly ? toWeeks(days) : days
  }, [site.data, range, weekly])
  const config = useMemo(
    () => ({
      visits: {
        label: t('insights.day.visits'),
        colors: { light: ['var(--brand)'], dark: ['var(--brand)'] },
      },
    }),
    [t],
  )
  const openDay = (state: MouseHandlerDataParam) => {
    if (!weekly && typeof state.activeLabel === 'string') {
      void navigate(dayPath(state.activeLabel))
    }
  }

  return (
    <SectionCard
      title={t(weekly ? 'insights.site.trendWeekly' : 'insights.site.trend')}
      subtitle={weekly ? undefined : t('insights.site.trendHint')}
      action={<SectionStatus meta={site.data?.meta} />}
    >
      <div className="h-[200px]">
        {site.isError && site.data === undefined ? (
          <QueryBody
            query={site}
            skeleton={null}
            isEmpty={() => false}
            empty=""
          >
            {() => null}
          </QueryBody>
        ) : (
          <EvilBarChart
            data={points}
            config={config}
            xDataKey="dateKey"
            isLoading={site.data === undefined}
            className="aspect-auto h-full"
            barRadius={3}
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
            <EvilBarChart.Bar dataKey="visits" />
          </EvilBarChart>
        )}
      </div>
    </SectionCard>
  )
}
