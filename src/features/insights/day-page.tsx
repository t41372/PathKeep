/**
 * One local calendar day: how much, when, where, the browsing sessions and
 * the searches. Reached from the Home year heatmap, the daily activity chart
 * and every date in the other drill-ins; links onward to History.
 *
 * Reads: `get_day_insights` (KPIs against the day before, hours, top sites),
 * `get_sessions` and `get_search_queries` for the exact day, and
 * `get_session_detail` only when a session is opened.
 */
import { ChevronDown, ChevronLeft, ChevronRight, Search } from 'lucide-react'
import { useMemo, useState } from 'react'
import { Link, useParams } from 'react-router-dom'
import { EvilBarChart } from '@/components/evilcharts/charts/recharts-bar-chart'
import { Favicon } from '@/components/app/favicon'
import { PageScroll, SectionCard } from '@/components/app/section-card'
import { Button } from '@/components/ui/button'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'
import { localDateKey } from '@/lib/backend-client/insights'
import { cn } from '@/lib/cn'
import type { SessionSummary } from '@/lib/core-intelligence/types'
import { useFormat, useI18n } from '@/lib/i18n'
import {
  useDay,
  useDaySearches,
  useDaySessions,
  useSessionDetail,
} from './drill-queries'
import { historyPath, insightsPath, searchPath } from './links'
import { formatDuration, rowLink, shareBar } from './display'
import {
  DrillHeader,
  KpiTile,
  ListSkeleton,
  QueryBody,
  SectionStatus,
} from './parts'

const dayKey = /^\d{4}-\d{2}-\d{2}$/

function parseDay(value: string) {
  if (!dayKey.test(value)) return null
  const [year, month, day] = value.split('-').map(Number)
  const date = new Date(year, month - 1, day)
  return localDateKey(date) === value ? date : null
}

function shift(date: Date, days: number) {
  const next = new Date(date)
  next.setDate(next.getDate() + days)
  return localDateKey(next)
}

export default function DayPage() {
  const { t } = useI18n()
  const format = useFormat()
  const { date = '' } = useParams()
  const day = parseDay(date)
  const today = localDateKey(new Date())
  const valid = day !== null && date <= today

  return (
    <PageScroll wide>
      <DrillHeader
        back={insightsPath()}
        eyebrow={t('insights.day.eyebrow')}
        title={day ? format.weekdayDate(day) : date}
        subtitle={day ? String(day.getFullYear()) : undefined}
        actions={
          day && (
            <>
              <div className="flex items-center gap-1">
                <Button variant="outline" size="icon-sm" asChild>
                  <Link
                    to={`/insights/day/${shift(day, -1)}`}
                    aria-label={t('insights.day.previous')}
                  >
                    <ChevronLeft />
                  </Link>
                </Button>
                <Button
                  variant="outline"
                  size="icon-sm"
                  disabled={date >= today}
                  aria-label={t('insights.day.next')}
                  asChild={date < today}
                >
                  {date < today ? (
                    <Link to={`/insights/day/${shift(day, 1)}`}>
                      <ChevronRight />
                    </Link>
                  ) : (
                    <ChevronRight />
                  )}
                </Button>
              </div>
              <Button variant="outline" size="sm" asChild>
                <Link to={historyPath({ date })}>
                  {t('insights.openInHistory')}
                </Link>
              </Button>
            </>
          )
        }
      />
      {valid ? (
        <DayContent date={date} />
      ) : (
        <p className="text-sm text-muted-foreground">
          {t('insights.day.invalid')}
        </p>
      )}
    </PageScroll>
  )
}

function DayContent({ date }: { date: string }) {
  const { t } = useI18n()
  const format = useFormat()
  const day = useDay(date)
  const data = day.data?.data
  const hint = t('insights.day.vsPrevious')
  const loading = day.data === undefined && !day.isError

  return (
    <>
      <div className="grid grid-cols-[repeat(auto-fit,minmax(170px,1fr))] gap-3">
        <KpiTile
          label={t('insights.kpi.visits')}
          value={format.number(data?.digestSummary.totalVisits.value ?? 0)}
          metric={data?.digestSummary.totalVisits}
          hint={hint}
          loading={loading}
        />
        <KpiTile
          label={t('insights.kpi.domains')}
          value={format.number(data?.digestSummary.distinctDomains.value ?? 0)}
          metric={data?.digestSummary.distinctDomains}
          hint={hint}
          loading={loading}
        />
        <KpiTile
          label={t('insights.kpi.searches')}
          value={format.number(data?.digestSummary.totalSearches.value ?? 0)}
          metric={data?.digestSummary.totalSearches}
          hint={hint}
          loading={loading}
        />
        <KpiTile
          label={t('insights.kpi.activeTime')}
          value={formatDuration(
            data?.digestSummary.activeTimeMs.value ?? 0,
            t,
            format,
          )}
          metric={data?.digestSummary.activeTimeMs}
          hint={hint}
          loading={loading}
        />
      </div>
      {day.isError && day.data === undefined ? (
        <SectionCard>
          <QueryBody query={day} skeleton={null} isEmpty={() => false} empty="">
            {() => null}
          </QueryBody>
        </SectionCard>
      ) : (
        <div className="grid grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)] gap-4 max-[900px]:grid-cols-1">
          <HoursCard date={date} />
          <DaySitesCard date={date} />
        </div>
      )}
      <div className="grid grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)] gap-4 max-[900px]:grid-cols-1">
        <SessionsCard date={date} />
        <DaySearchesCard date={date} />
      </div>
    </>
  )
}

function HoursCard({ date }: { date: string }) {
  const { t } = useI18n()
  const day = useDay(date)
  const config = useMemo(
    () => ({
      visits: {
        label: t('insights.day.visits'),
        colors: { light: ['var(--brand)'], dark: ['var(--brand)'] },
      },
    }),
    [t],
  )
  const points = useMemo(
    () =>
      (day.data?.data.hourlyActivity ?? []).map((bucket) => ({
        hour: String(bucket.hour).padStart(2, '0'),
        visits: bucket.visitCount,
      })),
    [day.data],
  )
  return (
    <SectionCard
      title={t('insights.day.hours')}
      action={<SectionStatus meta={day.data?.meta} />}
    >
      <div className="h-[200px]">
        <EvilBarChart
          data={points}
          config={config}
          xDataKey="hour"
          isLoading={day.data === undefined}
          className="aspect-auto h-full"
          barRadius={3}
        >
          <EvilBarChart.Grid vertical={false} strokeDasharray="3 4" />
          <EvilBarChart.XAxis
            dataKey="hour"
            tickLine={false}
            axisLine={false}
            interval={5}
            className="font-mono"
          />
          <EvilBarChart.Tooltip />
          <EvilBarChart.Bar dataKey="visits" />
        </EvilBarChart>
      </div>
    </SectionCard>
  )
}

function DaySitesCard({ date }: { date: string }) {
  const { t } = useI18n()
  const format = useFormat()
  const day = useDay(date)
  return (
    <SectionCard title={t('insights.topSites.title')}>
      <QueryBody
        query={day}
        skeleton={<ListSkeleton rows={6} />}
        isEmpty={(result) => result.data.topSites.length === 0}
        empty={t('insights.day.empty')}
      >
        {(result) => {
          const sites = result.data.topSites.slice(0, 6)
          const max = Math.max(1, ...sites.map((site) => site.visitCount))
          return (
            <ul className="flex flex-col gap-1.5">
              {sites.map((site) => (
                <li key={site.registrableDomain}>
                  <Link
                    to={historyPath({ date, domain: site.registrableDomain })}
                    title={t('insights.day.siteInHistory')}
                    className="relative flex h-8 items-center gap-2.5 rounded-md px-2 text-[13px] hover:no-underline"
                  >
                    <span
                      aria-hidden
                      className="absolute inset-y-0 left-0 rounded-md bg-brand-soft"
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
                      {format.number(site.visitCount)}
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

function SessionsCard({ date }: { date: string }) {
  const { t } = useI18n()
  const sessions = useDaySessions(date)
  return (
    <SectionCard
      title={t('insights.day.sessions')}
      subtitle={t('insights.day.sessionsHint')}
    >
      <QueryBody
        query={sessions}
        skeleton={<ListSkeleton rows={4} height={44} />}
        isEmpty={(result) => result.sessions.length === 0}
        empty={t('insights.day.noSessions')}
      >
        {(result) => (
          <ul className="-mx-2 flex flex-col">
            {[...result.sessions]
              .sort((a, b) => a.firstVisitMs - b.firstVisitMs)
              .map((session) => (
                <SessionRow
                  key={session.sessionId}
                  session={session}
                  date={date}
                />
              ))}
          </ul>
        )}
      </QueryBody>
    </SectionCard>
  )
}

function SessionRow({
  session,
  date,
}: {
  session: SessionSummary
  date: string
}) {
  const { t } = useI18n()
  const format = useFormat()
  const [open, setOpen] = useState(false)
  return (
    <li>
      <Collapsible open={open} onOpenChange={setOpen}>
        <CollapsibleTrigger className="flex w-full items-start gap-3 rounded-lg p-2 text-left transition-colors hover:bg-muted">
          <span className="min-w-[92px] pt-0.5 font-mono text-xs text-muted-foreground tabular">
            {format.time(session.firstVisitMs)}–
            {format.time(session.lastVisitMs)}
          </span>
          <span className="flex min-w-0 flex-1 flex-col gap-0.5">
            <span className="truncate text-[13px] font-medium">
              {session.autoTitle || t('insights.day.untitledSession')}
            </span>
            <span className="text-xs text-muted-foreground">
              {t('insights.day.sessionMeta', {
                pages: t('common.pages', { count: session.visitCount }),
                sites: t('common.sites', { count: session.domainCount }),
              })}
              {session.searchCount > 0 &&
                ` · ${t('insights.day.searchCount', { count: session.searchCount })}`}
            </span>
          </span>
          <ChevronDown
            aria-hidden
            className={cn(
              'mt-1 size-4 shrink-0 text-muted-foreground transition-transform duration-200',
              open && 'rotate-180',
            )}
          />
        </CollapsibleTrigger>
        <CollapsibleContent className="overflow-hidden data-[state=closed]:animate-collapsible-up data-[state=open]:animate-collapsible-down">
          {open && <SessionVisits sessionId={session.sessionId} date={date} />}
        </CollapsibleContent>
      </Collapsible>
    </li>
  )
}

function SessionVisits({
  sessionId,
  date,
}: {
  sessionId: string
  date: string
}) {
  const { t } = useI18n()
  const format = useFormat()
  const detail = useSessionDetail(sessionId, true)
  return (
    <div className="pb-2 pl-[104px] pr-2">
      <QueryBody
        query={detail}
        skeleton={<ListSkeleton rows={3} height={24} />}
        isEmpty={(result) => result.visits.length === 0}
        empty={t('insights.day.noVisits')}
      >
        {(result) => (
          <ol className="flex flex-col border-l pl-3">
            {result.visits.map((visit) => (
              <li key={visit.visitId}>
                {visit.isSearchEvent && visit.searchQuery ? (
                  <Link to={searchPath(visit.searchQuery)} className={rowLink}>
                    <span className="w-10 font-mono text-[11px] text-muted-foreground tabular">
                      {format.time(visit.visitTimeMs)}
                    </span>
                    <Search
                      className="size-3.5 shrink-0 text-blue"
                      aria-hidden
                    />
                    <span className="truncate">
                      {t('insights.day.searched', { query: visit.searchQuery })}
                    </span>
                  </Link>
                ) : (
                  <Link
                    to={historyPath({ date, visit: visit.visitId })}
                    className={rowLink}
                  >
                    <span className="w-10 font-mono text-[11px] text-muted-foreground tabular">
                      {format.time(visit.visitTimeMs)}
                    </span>
                    <Favicon
                      domain={visit.registrableDomain}
                      className="size-4"
                    />
                    <span className="truncate">{visit.title || visit.url}</span>
                  </Link>
                )}
              </li>
            ))}
          </ol>
        )}
      </QueryBody>
    </div>
  )
}

function DaySearchesCard({ date }: { date: string }) {
  const { t } = useI18n()
  const format = useFormat()
  const searches = useDaySearches(date)
  return (
    <SectionCard
      title={t('insights.day.searches')}
      action={<SectionStatus meta={searches.data?.meta} />}
    >
      <QueryBody
        query={searches}
        skeleton={<ListSkeleton rows={4} />}
        isEmpty={(result) => result.data.rows.length === 0}
        empty={t('insights.day.noSearches')}
      >
        {(result) => (
          <ul className="-mx-2 flex flex-col">
            {result.data.rows.map((row) => (
              <li key={`${row.searchEngine}-${row.normalizedQuery}`}>
                <Link to={searchPath(row.rawQuery)} className={rowLink}>
                  <Search
                    className="size-3.5 shrink-0 text-muted-foreground"
                    aria-hidden
                  />
                  <span className="flex-1 truncate">{row.rawQuery}</span>
                  <span className="font-mono text-xs text-muted-foreground tabular">
                    {format.time(row.searchedAtMs)}
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
