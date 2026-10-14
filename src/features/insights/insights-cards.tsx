/**
 * The first cards on the Insights screen: KPIs, daily activity, top sites,
 * rhythm, searches and pages reopened. Each takes the range and owns its
 * query; each links into a drill-in.
 */
import { ScrollList } from '@/components/app/scroll-list'
import { useMemo, useState } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import type { MouseHandlerDataParam } from 'recharts'
import { EvilBarChart } from '@/components/evilcharts/charts/recharts-bar-chart'
import { SiteIcon } from '@/features/history/site-icon'
import { heatLevel } from '@/components/app/heat-level'
import { Heatmap, type HeatCell } from '@/components/app/heatmap'
import { CardError, SectionCard } from '@/components/app/section-card'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Skeleton } from '@/components/ui/skeleton'
import type { TopSite } from '@/lib/core-intelligence/types'
import { isStale } from '@/lib/backend-client/insights'
import { useI18n } from '@/lib/i18n'
import { barLabel } from './buckets'
import { RhythmSummary } from './rhythm-summary'
import { dayPath, pagePath, searchPath, sitePath } from './links'
import { formatDuration, rowLink } from './display'
import { ShareBar } from './share-bar'
import { KpiTile, ListSkeleton, QueryBody, SectionStatus } from './parts'
import {
  useDailyActivity,
  useDigest,
  useFrequentSearches,
  useRefindPages,
  useRhythm,
  useTopSites,
  type SiteOrder,
} from './queries'
import { scopeDays, type Scope } from './range'
import { useInsightsFormat } from './zone'

export function KpiRow({ scope }: { scope: Scope }) {
  const { t } = useI18n()
  const format = useInsightsFormat()
  const digest = useDigest(scope)
  const data = digest.data?.data
  // All time has no previous period to compare with.
  const days = scopeDays(scope)
  const hint = days ? t('insights.kpi.vsPrevious', { days }) : undefined
  const compared = <T,>(metric: T) => (days ? metric : undefined)
  const loading = data === undefined && !digest.isError
  const updating = isStale(digest.data)

  if (digest.isError && data === undefined) {
    return (
      <div className="rounded-xl border bg-card px-4 py-3.5 shadow-card">
        <CardError error={digest.error} onRetry={() => void digest.refetch()} />
      </div>
    )
  }
  return (
    <div className="grid grid-cols-[repeat(auto-fit,minmax(128px,1fr))] gap-3">
      <KpiTile
        label={t('insights.kpi.visits')}
        metric={compared(data?.totalVisits)}
        value={format.compact(data?.totalVisits.value ?? 0)}
        hint={hint}
        loading={loading}
        updating={updating}
      />
      <KpiTile
        label={t('insights.kpi.domains')}
        metric={compared(data?.distinctDomains)}
        value={format.number(data?.distinctDomains?.value ?? 0)}
        hint={hint}
        loading={loading}
        updating={updating}
      />
      <KpiTile
        label={t('insights.kpi.searches')}
        metric={compared(data?.totalSearches)}
        value={format.compact(data?.totalSearches.value ?? 0)}
        hint={hint}
        loading={loading}
        updating={updating}
      />
      <KpiTile
        label={t('insights.kpi.activeTime')}
        metric={compared(data?.activeTimeMs)}
        value={formatDuration(data?.activeTimeMs?.value ?? 0, t, format)}
        hint={hint}
        loading={loading}
        updating={updating}
      />
      {/* Over all time every site is new; the tile would repeat the one above. */}
      {days !== null && (
        <KpiTile
          label={t('insights.kpi.newSites')}
          metric={compared(data?.newDomains)}
          value={format.number(data?.newDomains.value ?? 0)}
          explain={t('insights.kpi.newSitesDetail')}
          hint={hint}
          loading={loading}
          updating={updating}
        />
      )}
      <KpiTile
        label={t('insights.kpi.deepDives')}
        metric={compared(data?.deepReadPages)}
        value={format.number(data?.deepReadPages.value ?? 0)}
        explain={t('insights.kpi.deepDivesDetail')}
        hint={hint}
        loading={loading}
        updating={updating}
      />
      <KpiTile
        label={t('insights.kpi.reopened')}
        metric={compared(data?.refindPages)}
        value={format.number(data?.refindPages.value ?? 0)}
        explain={t('insights.kpi.reopenedDetail')}
        hint={hint}
        loading={loading}
        updating={updating}
      />
    </div>
  )
}

export function DailyActivityCard({ scope }: { scope: Scope }) {
  const { t } = useI18n()
  const format = useInsightsFormat()
  const navigate = useNavigate()
  const daily = useDailyActivity(scope)
  const length = daily.data?.length ?? 'day'
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
  const points = daily.data?.bars ?? []
  const openDay = (state: MouseHandlerDataParam) => {
    if (length === 'day' && typeof state.activeLabel === 'string') {
      void navigate(dayPath(state.activeLabel, scope))
    }
  }

  return (
    <SectionCard
      title={t('insights.daily.title')}
      subtitle={t(
        {
          day: 'insights.daily.hint',
          week: 'insights.daily.perWeek',
          month: 'insights.daily.perMonth',
        }[length] as 'insights.daily.hint',
      )}
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
              style: length === 'day' ? { cursor: 'pointer' } : undefined,
            }}
          >
            <EvilBarChart.Grid vertical={false} strokeDasharray="3 4" />
            <EvilBarChart.XAxis
              dataKey="dateKey"
              tickLine={false}
              axisLine={false}
              minTickGap={32}
              tickFormatter={(value: string) => barLabel(format, length, value)}
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

/** How a site's place in Top sites is decided, and the number shown for it. */
const siteOrders = {
  visits: (site: TopSite) => site.visitCount,
  unique_days: (site: TopSite) => site.uniqueDays,
  average_daily_visits: (site: TopSite) => site.averageDailyVisits,
} satisfies Record<SiteOrder, (site: TopSite) => number>

export function TopSitesCard({ scope }: { scope: Scope }) {
  const { t } = useI18n()
  const format = useInsightsFormat()
  const [order, setOrder] = useState<SiteOrder>('visits')
  const sites = useTopSites(scope, order)
  const measure = siteOrders[order]
  const shown = (site: TopSite) =>
    order === 'average_daily_visits'
      ? format.number(Math.round(measure(site) * 10) / 10)
      : format.compact(measure(site))

  return (
    <SectionCard
      title={t('insights.topSites.title')}
      action={
        <span className="flex items-center gap-1.5">
          <Select
            value={order}
            onValueChange={(next) => setOrder(next as SiteOrder)}
          >
            <SelectTrigger
              size="sm"
              aria-label={t('insights.topSites.orderLabel')}
              className="h-7 gap-1 border-none bg-transparent px-2 text-xs text-muted-foreground shadow-none"
            >
              <SelectValue />
            </SelectTrigger>
            <SelectContent align="end">
              {(Object.keys(siteOrders) as SiteOrder[]).map((id) => (
                <SelectItem key={id} value={id}>
                  {t(`insights.topSites.order.${id}`)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <SectionStatus meta={sites.data?.meta} />
        </span>
      }
    >
      <QueryBody
        query={sites}
        skeleton={<ListSkeleton rows={6} />}
        isEmpty={(result) => result.data.items.length === 0}
        empty={t('insights.topSites.empty')}
      >
        {(result) => {
          const max = Math.max(1, ...result.data.items.map(measure))
          return (
            // As many of the 40 as the column beside it is tall, at least six;
            // the rest scroll, and the fades say so.
            <ScrollList fill maxHeight={6 * 38}>
              <ul className="flex flex-col gap-1.5">
                {result.data.items.map((site) => (
                  <li key={site.registrableDomain}>
                    <Link
                      to={sitePath(site.registrableDomain, scope)}
                      className="relative flex h-8 items-center gap-2.5 rounded-md px-2 text-[13px] hover:no-underline"
                    >
                      <SiteIcon domain={site.registrableDomain} />
                      <span className="flex-1 truncate font-medium">
                        {site.registrableDomain}
                      </span>
                      <ShareBar value={measure(site)} max={max} />
                      <span className="w-12 text-right font-mono text-xs text-muted-foreground">
                        {shown(site)}
                      </span>
                    </Link>
                  </li>
                ))}
              </ul>
            </ScrollList>
          )
        }}
      </QueryBody>
    </SectionCard>
  )
}

export function RhythmCard({ scope }: { scope: Scope }) {
  const { t } = useI18n()
  const format = useInsightsFormat()
  const rhythm = useRhythm(scope)
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
            hour: format.clockTime(hour),
            visits: t('common.visits', { count: visits }),
          }),
        }
      }),
    )
  }, [rhythm.data, t, weekdays, format])

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
        <>
          <div className="overflow-x-auto">
            <Heatmap
              columns={columns}
              label={t('insights.rhythm.subtitle')}
              cellSize={16}
              gap={3}
              rowLabels={weekdays}
              columnLabels={Array.from({ length: 24 }, (_, hour) =>
                hour % 6 === 0 || hour === 23 ? format.hour(hour) : null,
              )}
            />
          </div>
          <RhythmSummary scope={scope} rhythm={rhythm.data.data} />
        </>
      )}
    </SectionCard>
  )
}

export function SearchesAndRefindCard({ scope }: { scope: Scope }) {
  const { t } = useI18n()
  const format = useInsightsFormat()
  const searches = useFrequentSearches(scope)
  const refind = useRefindPages(scope)

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
                to={searchPath(concept.query, scope)}
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
        isEmpty={(result) => result.data.items.length === 0}
        empty={t('insights.refind.empty')}
      >
        {(result) => {
          // Two pages can share a title (two chats in one app); the path tells them apart.
          const titles = new Map<string, number>()
          for (const page of result.data.items)
            titles.set(
              page.title ?? '',
              (titles.get(page.title ?? '') ?? 0) + 1,
            )
          return (
            <ScrollList fill maxHeight={5 * 36}>
              <ul className="-mx-2 flex flex-col">
                {result.data.items.map((page) => {
                  const where = pageWhere(page.url)
                  const shared = (titles.get(page.title ?? '') ?? 0) > 1
                  return (
                    <li key={page.canonicalUrl}>
                      <Link
                        to={pagePath(page.canonicalUrl, scope, page.profileId)}
                        className={rowLink}
                      >
                        <SiteIcon domain={where.host} />
                        <span className="flex min-w-0 flex-1 items-baseline gap-1.5">
                          <span className="truncate">
                            {page.title || page.url}
                          </span>
                          {page.title && (
                            <span className="truncate text-xs text-muted-foreground">
                              {shared ? where.hostAndPath : where.host}
                            </span>
                          )}
                        </span>
                        <span className="text-xs text-muted-foreground tabular">
                          {t('insights.refind.days', {
                            count: page.crossDayCount,
                            days: format.number(page.crossDayCount),
                          })}
                        </span>
                      </Link>
                    </li>
                  )
                })}
              </ul>
            </ScrollList>
          )
        }}
      </QueryBody>
    </SectionCard>
  )
}

/** Where a page is, for a second line under its title: the host, or host and path. */
function pageWhere(url: string) {
  try {
    const parsed = new URL(url)
    const path = parsed.pathname === '/' ? '' : parsed.pathname
    return { host: parsed.hostname, hostAndPath: `${parsed.hostname}${path}` }
  } catch {
    return { host: url, hostAndPath: url }
  }
}
