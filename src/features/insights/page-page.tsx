/**
 * A page the user keeps coming back to: its visits across every browser,
 * the last 12 weeks, why PathKeep counts it as re-found, the recent days it
 * was opened and the searches that led to it. Reached from "Pages you keep
 * reopening" and Threads; links onward to the day and site drill-ins and
 * History.
 *
 * Reads: `get_refind_page_detail` (score evidence for one profile's row)
 * and `get_url_detail` (exact totals for the URL across all browsers, the
 * same read as History's detail panel).
 */
import { ExternalLink, Search } from 'lucide-react'
import { useMemo } from 'react'
import { Link, useParams, useSearchParams } from 'react-router-dom'
import { EvilBarChart } from '@/components/evilcharts/charts/recharts-bar-chart'
import { Favicon } from '@/components/app/favicon'
import {
  CardError,
  PageScroll,
  SectionCard,
} from '@/components/app/section-card'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { openInBrowser } from '@/features/history/open-link'
import { useUrlDetail } from '@/features/history/queries'
import { useSourceStats } from '@/features/home/queries'
import type { RefindScoreFactor } from '@/lib/core-intelligence/types'
import { useFormat, useI18n } from '@/lib/i18n'
import { useRefindPage } from './drill-queries'
import {
  dayPath,
  historyPath,
  insightsPath,
  searchPath,
  sitePath,
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
import { useRange } from './range'

const factorKeys: Record<string, 'days' | 'trails' | 'searches' | 'direct'> = {
  cross_day_count: 'days',
  trail_count: 'trails',
  search_arrival_count: 'searches',
  typed_revisit_count: 'direct',
}

export default function PagePage() {
  const { t } = useI18n()
  const format = useFormat()
  const { url: canonicalUrl = '' } = useParams()
  const [params] = useSearchParams()
  const profileId = params.get('profile')
  const [range, setRange] = useRange()
  const refind = useRefindPage(canonicalUrl, range, profileId)
  const page = refind.data?.data.page
  const pageUrl = page?.url ?? canonicalUrl
  const detail = useUrlDetail(pageUrl)
  const loading = detail.data === undefined && !detail.isError
  const title = page?.title || detail.data?.title || pageUrl

  return (
    <PageScroll wide>
      <DrillHeader
        back={insightsPath(range)}
        eyebrow={t('insights.page.eyebrow')}
        title={title}
        subtitle={<span className="font-mono">{pageUrl}</span>}
        icon={
          <Favicon
            domain={page?.registrableDomain || detail.data?.domain || pageUrl}
            className="size-10 rounded-xl text-base"
          />
        }
        actions={
          <>
            <RangeToggle value={range} onChange={setRange} />
            <Button
              variant="outline"
              size="sm"
              onClick={() =>
                void openInBrowser(pageUrl, t('insights.page.openFailed'))
              }
            >
              <ExternalLink />
              {t('insights.page.open')}
            </Button>
            <Button variant="outline" size="sm" asChild>
              <Link
                to={historyPath({
                  q: page?.title || pageUrl,
                  domain: page?.registrableDomain,
                })}
              >
                {t('insights.openInHistory')}
              </Link>
            </Button>
          </>
        }
      />
      <div className="grid grid-cols-[repeat(auto-fit,minmax(170px,1fr))] gap-3">
        <KpiTile
          label={t('insights.page.totalVisits')}
          value={format.number(detail.data?.totalVisits ?? 0)}
          detail={t('insights.page.allBrowsers')}
          loading={loading}
        />
        <KpiTile
          label={t('insights.page.firstVisit')}
          value={
            detail.data?.firstVisitAt
              ? format.date(detail.data.firstVisitAt)
              : '—'
          }
          loading={loading}
        />
        <KpiTile
          label={t('insights.page.lastVisit')}
          value={
            detail.data?.lastVisitAt
              ? format.relative(detail.data.lastVisitAt)
              : '—'
          }
          loading={loading}
        />
        <KpiTile
          label={t('insights.page.browsers')}
          value={format.number(detail.data?.browsers.length ?? 0)}
          detail={detail.data?.browsers.join(', ')}
          loading={loading}
        />
      </div>
      <div className="grid grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)] gap-4 max-[900px]:grid-cols-1">
        <WeeksCard url={pageUrl} />
        <WhyCard canonicalUrl={canonicalUrl} profileId={profileId} />
      </div>
      <div className="grid grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)] gap-4 max-[900px]:grid-cols-1">
        <SectionCard title={t('insights.page.searchesTitle')}>
          <QueryBody
            query={refind}
            skeleton={<ListSkeleton rows={3} />}
            isEmpty={(result) => result.data.relatedTrails.length === 0}
            empty={t('insights.page.noSearches')}
          >
            {(result) => {
              const byQuery = new Map<string, { count: number; last: number }>()
              for (const trail of result.data.relatedTrails) {
                const seen = byQuery.get(trail.initialQuery)
                byQuery.set(trail.initialQuery, {
                  count: (seen?.count ?? 0) + 1,
                  last: Math.max(seen?.last ?? 0, trail.firstVisitMs),
                })
              }
              return (
                <ul className="-mx-2 flex flex-col">
                  {[...byQuery]
                    .sort((a, b) => b[1].last - a[1].last)
                    .map(([query, { count, last }]) => (
                      <li key={query}>
                        <Link to={searchPath(query, range)} className={rowLink}>
                          <Search
                            className="size-3.5 shrink-0 text-muted-foreground"
                            aria-hidden
                          />
                          <span className="flex-1 truncate">{query}</span>
                          <span className="text-xs text-muted-foreground tabular">
                            {count > 1 &&
                              `${t('insights.times', { count: String(count) })} · `}
                            {format.date(last)}
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
          title={t('insights.page.recentDays')}
          action={
            page && (
              <Link
                to={sitePath(page.registrableDomain, range)}
                className="text-[13px] text-muted-foreground hover:text-foreground"
              >
                {page.registrableDomain} →
              </Link>
            )
          }
        >
          <QueryBody
            query={refind}
            skeleton={<ListSkeleton rows={2} />}
            isEmpty={(result) => result.data.recentDays.length === 0}
            empty={t('insights.page.noDays')}
          >
            {(result) => (
              <div className="flex flex-wrap gap-1.5">
                {result.data.recentDays.map((day) => (
                  <Link
                    key={day}
                    to={dayPath(day)}
                    className="flex h-7 items-center rounded-full border bg-card px-3 text-[13px] tabular transition-colors hover:bg-muted hover:no-underline"
                  >
                    {format.date(`${day}T00:00:00`)}
                  </Link>
                ))}
              </div>
            )}
          </QueryBody>
        </SectionCard>
      </div>
    </PageScroll>
  )
}

function WeeksCard({ url }: { url: string }) {
  const { t } = useI18n()
  const format = useFormat()
  const detail = useUrlDetail(url)
  const config = useMemo(
    () => ({
      visits: {
        label: t('insights.day.visits'),
        colors: { light: ['var(--brand)'], dark: ['var(--brand)'] },
      },
    }),
    [t],
  )
  return (
    <SectionCard title={t('insights.page.weeks')}>
      <div className="h-[180px]">
        {detail.isError ? (
          <CardError
            error={detail.error}
            onRetry={() => void detail.refetch()}
          />
        ) : (
          <EvilBarChart
            data={detail.data?.weeklyVisits ?? []}
            config={config}
            xDataKey="weekStart"
            isLoading={detail.data === undefined}
            className="aspect-auto h-full"
            barRadius={3}
          >
            <EvilBarChart.Grid vertical={false} strokeDasharray="3 4" />
            <EvilBarChart.XAxis
              dataKey="weekStart"
              tickLine={false}
              axisLine={false}
              minTickGap={24}
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

/**
 * The evidence behind "re-found": each signal the score counts, its value and
 * how much it contributed. Scores are kept per browser profile, so this names
 * the profile the numbers are from.
 */
function WhyCard({
  canonicalUrl,
  profileId,
}: {
  canonicalUrl: string
  profileId: string | null
}) {
  const { t } = useI18n()
  const format = useFormat()
  const [range] = useRange()
  const refind = useRefindPage(canonicalUrl, range, profileId)
  const sources = useSourceStats()
  const page = refind.data?.data.page
  const profile = sources.data?.find(
    (source) => source.profileId === page?.profileId,
  )

  return (
    <SectionCard
      title={t('insights.page.why')}
      action={
        <SectionStatus
          meta={refind.data?.meta}
          explain={t('insights.page.whyExplain')}
        />
      }
    >
      <QueryBody
        query={refind}
        skeleton={<Skeleton className="h-32" />}
        isEmpty={(result) => result.data.explanation.factors.length === 0}
        empty={t('insights.page.notRefound')}
      >
        {(result) => {
          const factors = result.data.explanation.factors
          const max = Math.max(
            1,
            ...factors.map((factor) => factor.contribution),
          )
          return (
            <div className="flex flex-col gap-3">
              <ul className="flex flex-col gap-2">
                {factors.map((factor: RefindScoreFactor) => {
                  const name = factorKeys[factor.signal]
                  return (
                    <li key={factor.signal} className="flex flex-col gap-1">
                      <span className="flex items-baseline justify-between gap-2 text-[13px]">
                        <span>
                          {name
                            ? t(`insights.page.factors.${name}`, {
                                count: Math.round(factor.rawValue),
                              })
                            : factor.signal}
                        </span>
                        <span className="font-mono text-[11px] text-muted-foreground">
                          ×{format.number(factor.weight)}
                        </span>
                      </span>
                      <span className="h-1.5 overflow-hidden rounded-full bg-muted">
                        <span
                          className="block h-full rounded-full bg-brand transition-[width] duration-500"
                          style={shareBar(factor.contribution, max)}
                        />
                      </span>
                    </li>
                  )
                })}
              </ul>
              {page && (
                <p className="text-xs text-muted-foreground">
                  {t('insights.page.countedIn', {
                    profile: profile
                      ? `${profile.browserName} · ${profile.profileName}`
                      : page.profileId,
                  })}
                  {page.profileCount > 1 &&
                    ` ${t('insights.page.alsoIn', { count: page.profileCount - 1 })}`}
                </p>
              )}
            </div>
          )
        }}
      </QueryBody>
    </SectionCard>
  )
}
