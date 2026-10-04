/**
 * One search, by its text: how often it was searched, the other wordings of
 * it, and each time it was searched in the range with where it led. Reached
 * from Frequent searches, Threads, the day and site drill-ins; links onward
 * to the day drill-in and History.
 *
 * Reads: the range's query families (`get_query_families`, shared with the
 * Frequent searches card) to find this text's families (one per browser
 * profile and engine), `get_query_family_detail` for each, and
 * `get_trail_detail` only when a search is opened.
 */
import { ChevronDown, Search } from 'lucide-react'
import { useState } from 'react'
import { Link, useParams } from 'react-router-dom'
import { Favicon } from '@/components/app/favicon'
import {
  CardError,
  PageScroll,
  SectionCard,
} from '@/components/app/section-card'
import { Button } from '@/components/ui/button'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'
import { localDateKey } from '@/lib/backend-client/insights'
import { cn } from '@/lib/cn'
import type { TrailSummary } from '@/lib/core-intelligence/types'
import { useFormat, useI18n } from '@/lib/i18n'
import { useSearch, useTrailDetail } from './drill-queries'
import {
  dayPath,
  historyPath,
  insightsPath,
  searchPath,
  sitePath,
} from './links'
import { rowLink } from './display'
import {
  DrillHeader,
  KpiTile,
  ListSkeleton,
  QueryBody,
  SectionStatus,
  RangeToggle,
} from './parts'
import { useRange, type RangeId } from './range'

function hostOf(url: string) {
  try {
    return new URL(url).hostname.replace(/^www\./, '')
  } catch {
    return url
  }
}

export default function SearchPage() {
  const { t } = useI18n()
  const format = useFormat()
  const { query = '' } = useParams()
  const [range, setRange] = useRange()
  const search = useSearch(query, range)
  const { families, matches } = search
  const loading = families.data === undefined && !families.isError

  const total = matches.reduce((sum, family) => sum + family.memberCount, 0)
  const first = matches.map((family) => family.firstSeenAt).sort()[0]
  const last = matches
    .map((family) => family.lastSeenAt)
    .sort()
    .at(-1)
  const wordings = [
    ...new Set(
      matches
        .flatMap((family) => family.queries)
        .filter(
          (wording) =>
            wording.trim().toLowerCase() !== query.trim().toLowerCase(),
        ),
    ),
  ]

  return (
    <PageScroll wide>
      <DrillHeader
        back={insightsPath(range)}
        eyebrow={t('insights.search.eyebrow')}
        title={`“${query}”`}
        actions={
          <>
            <RangeToggle value={range} onChange={setRange} />
            <Button variant="outline" size="sm" asChild>
              <Link to={historyPath({ q: query })}>
                {t('insights.search.inHistory')}
              </Link>
            </Button>
          </>
        }
      />
      {families.isError && families.data === undefined ? (
        <SectionCard>
          <CardError
            error={families.error}
            onRetry={() => void families.refetch()}
          />
        </SectionCard>
      ) : !loading && matches.length === 0 ? (
        <SectionCard>
          <p className="text-sm text-muted-foreground">
            {t('insights.search.notFound')}
          </p>
        </SectionCard>
      ) : (
        <>
          <div className="grid grid-cols-[repeat(auto-fit,minmax(170px,1fr))] gap-3">
            <KpiTile
              label={t('insights.search.timesSearched')}
              value={format.number(total)}
              detail={t('insights.search.allTime')}
              loading={loading}
            />
            <KpiTile
              label={t('insights.search.first')}
              value={first ? format.date(first) : '—'}
              loading={loading}
            />
            <KpiTile
              label={t('insights.search.last')}
              value={last ? format.relative(last) : '—'}
              loading={loading}
            />
            <KpiTile
              label={t('insights.search.engines')}
              value={
                <span className="capitalize">
                  {[
                    ...new Set(matches.map((family) => family.searchEngine)),
                  ].join(', ')}
                </span>
              }
              loading={loading}
            />
          </div>
          <div className="grid grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)] gap-4 max-[900px]:grid-cols-1">
            <SectionCard
              title={t('insights.search.times')}
              subtitle={t('insights.search.timesHint')}
              action={<SectionStatus meta={families.data?.meta} />}
            >
              {search.error ? (
                <CardError error={search.error} onRetry={search.retry} />
              ) : loading || search.pending ? (
                <ListSkeleton rows={5} height={44} />
              ) : search.trails.length === 0 ? (
                <p className="text-sm text-muted-foreground">
                  {t('insights.search.noTimes')}
                </p>
              ) : (
                <ul className="-mx-2 flex flex-col">
                  {search.trails.map((trail) => (
                    <TrailRow key={trail.trailId} trail={trail} />
                  ))}
                </ul>
              )}
            </SectionCard>
            <div className="flex min-w-0 flex-col gap-4">
              <LandingsCard
                trails={search.trails}
                loading={loading || search.pending}
                range={range}
              />
              <SectionCard title={t('insights.search.wordings')}>
                {loading ? (
                  <ListSkeleton rows={3} />
                ) : wordings.length === 0 ? (
                  <p className="text-sm text-muted-foreground">
                    {t('insights.search.oneWording')}
                  </p>
                ) : (
                  <div className="flex flex-wrap gap-1.5">
                    {wordings.map((wording) => (
                      <Link
                        key={wording}
                        to={searchPath(wording, range)}
                        className="flex h-7 items-center rounded-full border bg-card px-3 text-[13px] transition-colors hover:bg-muted hover:no-underline"
                      >
                        {wording}
                      </Link>
                    ))}
                  </div>
                )}
              </SectionCard>
            </div>
          </div>
        </>
      )}
    </PageScroll>
  )
}

/** Which sites the shown searches ended on, most often first. */
function LandingsCard({
  trails,
  loading,
  range,
}: {
  trails: TrailSummary[]
  loading: boolean
  range: RangeId
}) {
  const { t } = useI18n()
  const counts = new Map<string, number>()
  for (const trail of trails) {
    const site =
      trail.landingDomain || (trail.landingUrl ? hostOf(trail.landingUrl) : '')
    if (site) counts.set(site, (counts.get(site) ?? 0) + 1)
  }
  const sites = [...counts].sort((a, b) => b[1] - a[1])
  return (
    <SectionCard
      title={t('insights.search.landings')}
      subtitle={t('insights.search.landingsHint', { count: trails.length })}
    >
      {loading ? (
        <ListSkeleton rows={2} />
      ) : sites.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          {t('insights.search.noLandings')}
        </p>
      ) : (
        <ul className="-mx-2 flex flex-col">
          {sites.map(([site, count]) => (
            <li key={site}>
              <Link to={sitePath(site, range)} className={rowLink}>
                <Favicon domain={site} />
                <span className="flex-1 truncate font-medium">{site}</span>
                <span className="font-mono text-xs text-muted-foreground">
                  {t('insights.times', { count: String(count) })}
                </span>
              </Link>
            </li>
          ))}
        </ul>
      )}
    </SectionCard>
  )
}

function TrailRow({ trail }: { trail: TrailSummary }) {
  const { t } = useI18n()
  const format = useFormat()
  const [open, setOpen] = useState(false)
  const date = localDateKey(new Date(trail.firstVisitMs))
  return (
    <li>
      <Collapsible open={open} onOpenChange={setOpen}>
        <div className="flex items-start gap-3 rounded-lg p-2 transition-colors hover:bg-muted">
          <Link
            to={dayPath(date)}
            className="min-w-[96px] pt-0.5 text-xs text-muted-foreground tabular hover:text-foreground"
          >
            {format.date(trail.firstVisitMs)}
            <span className="block font-mono">
              {format.time(trail.firstVisitMs)}
            </span>
          </Link>
          <CollapsibleTrigger className="flex min-w-0 flex-1 items-start gap-2 text-left">
            <span className="flex min-w-0 flex-1 flex-col gap-0.5">
              <span className="truncate text-[13px] font-medium">
                {trail.landingUrl
                  ? t('insights.search.ledTo', {
                      site: trail.landingDomain || hostOf(trail.landingUrl),
                    })
                  : t('insights.search.noLanding')}
              </span>
              <span className="text-xs text-muted-foreground capitalize">
                {trail.visitCount > 1
                  ? t('insights.search.trailMeta', {
                      pages: t('common.pages', {
                        count: trail.visitCount - 1,
                      }),
                      engine: trail.searchEngine,
                    })
                  : trail.searchEngine}
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
        </div>
        <CollapsibleContent className="overflow-hidden data-[state=closed]:animate-collapsible-up data-[state=open]:animate-collapsible-down">
          {open && <TrailPages trailId={trail.trailId} date={date} />}
        </CollapsibleContent>
      </Collapsible>
    </li>
  )
}

function TrailPages({ trailId, date }: { trailId: string; date: string }) {
  const { t } = useI18n()
  const format = useFormat()
  const detail = useTrailDetail(trailId, true)
  return (
    <div className="pb-2 pl-[108px] pr-2">
      <QueryBody
        query={detail}
        skeleton={<ListSkeleton rows={2} height={24} />}
        isEmpty={(result) => result.members.length === 0}
        empty={t('insights.day.noVisits')}
      >
        {(result) => (
          <ol className="flex flex-col border-l pl-3">
            {result.members.map((member) => (
              <li key={member.visitId}>
                <Link
                  to={historyPath({ date, visit: member.visitId })}
                  className={rowLink}
                >
                  <span className="w-10 font-mono text-[11px] text-muted-foreground tabular">
                    {format.time(member.visitTimeMs)}
                  </span>
                  {member.role === 'search_event' ? (
                    <Search
                      className="size-3.5 shrink-0 text-blue"
                      aria-hidden
                    />
                  ) : (
                    <Favicon
                      domain={member.registrableDomain || hostOf(member.url)}
                      className="size-4"
                    />
                  )}
                  <span className="truncate">
                    {member.role === 'search_event' && member.searchQuery
                      ? t('insights.day.searched', {
                          query: member.searchQuery,
                        })
                      : member.title || member.url}
                  </span>
                </Link>
              </li>
            ))}
          </ol>
        )}
      </QueryBody>
    </div>
  )
}
