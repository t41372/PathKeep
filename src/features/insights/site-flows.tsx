/**
 * Where a site's visits came from and where its links led, over the scope:
 * the visits that came from elsewhere (typed or bookmarked, from a search,
 * from a link) with the sites behind them, and the sites opened from it;
 * and, for a site with a search engine, what was searched on it.
 *
 * Reads the site drill-in's `get_domain_deep_dive` (its `site_flows` sums);
 * the browser's record of which page opened each page decides every count.
 */
import { Search } from 'lucide-react'
import { Link } from 'react-router-dom'
import { SiteIcon } from '@/features/history/site-icon'
import { SectionCard } from '@/components/app/section-card'
import type { SiteFlowStat } from '@/lib/core-intelligence/types'
import { cn } from '@/lib/cn'
import { useFormat, useI18n } from '@/lib/i18n'
import { useSite } from './drill-queries'
import { rowLink } from './display'
import { searchPath, sitePath } from './links'
import { ListSkeleton, QueryBody, SectionStatus } from './parts'
import type { Scope } from './range'
import { ShareBar } from './share-bar'

const kinds = [
  { key: 'direct', label: 'insights.site.direct', tone: 'bg-brand' },
  { key: 'search', label: 'insights.site.fromSearch', tone: 'bg-blue' },
  { key: 'link', label: 'insights.site.fromLink', tone: 'bg-green' },
] as const

export function ArrivalsCard({
  domain,
  scope,
}: {
  domain: string
  scope: Scope
}) {
  const { t } = useI18n()
  const format = useFormat()
  const site = useSite(domain, scope)
  return (
    <SectionCard
      title={t('insights.site.arrivals')}
      subtitle={t('insights.site.arrivalsHint')}
      action={
        <SectionStatus
          meta={site.data?.meta}
          explain={t('insights.site.arrivalsExplain')}
        />
      }
    >
      <QueryBody
        query={site}
        skeleton={<ListSkeleton rows={4} />}
        isEmpty={({ data: { arrivals } }) =>
          arrivals.direct + arrivals.search + arrivals.link === 0
        }
        empty={t('insights.site.noArrivals')}
      >
        {({ data }) => {
          const total =
            data.arrivals.direct + data.arrivals.search + data.arrivals.link
          return (
            <div className="flex flex-col gap-4">
              <div
                aria-hidden
                className="flex h-2 overflow-hidden rounded-full bg-muted"
              >
                {kinds.map((kind) => (
                  <span
                    key={kind.key}
                    className={cn('h-full transition-[width]', kind.tone)}
                    style={{
                      width: `${(data.arrivals[kind.key] / total) * 100}%`,
                    }}
                  />
                ))}
              </div>
              <ul className="flex flex-col gap-1">
                {kinds.map((kind) => (
                  <li
                    key={kind.key}
                    className="flex h-7 items-center gap-2.5 px-1 text-[13px]"
                  >
                    <span
                      aria-hidden
                      className={cn('size-2 shrink-0 rounded-full', kind.tone)}
                    />
                    <span className="flex-1 truncate">{t(kind.label)}</span>
                    <span className="font-mono text-xs text-muted-foreground">
                      {format.number(data.arrivals[kind.key])}
                    </span>
                    <span className="w-12 text-right font-mono text-xs text-muted-foreground">
                      {format.percent(data.arrivals[kind.key] / total)}
                    </span>
                  </li>
                ))}
              </ul>
              <SiteList
                title={t('insights.site.referrers')}
                sites={data.referrers}
                scope={scope}
              />
              <SiteList
                title={t('insights.site.searchSites')}
                sites={data.searchSites}
                scope={scope}
              />
            </div>
          )
        }}
      </QueryBody>
    </SectionCard>
  )
}

export function OnwardCard({
  domain,
  scope,
}: {
  domain: string
  scope: Scope
}) {
  const { t } = useI18n()
  const site = useSite(domain, scope)
  return (
    <SectionCard
      title={t('insights.site.onward')}
      subtitle={t('insights.site.onwardHint')}
      action={
        <SectionStatus
          meta={site.data?.meta}
          explain={t('insights.site.arrivalsExplain')}
        />
      }
    >
      <QueryBody
        query={site}
        skeleton={<ListSkeleton rows={4} />}
        isEmpty={(result) => result.data.onward.length === 0}
        empty={t('insights.site.noOnward')}
      >
        {(result) => <SiteList sites={result.data.onward} scope={scope} />}
      </QueryBody>
    </SectionCard>
  )
}

/**
 * What was searched on the site, for a site with a search engine (YouTube's
 * own search, say). Shown once the site's numbers are in and only when there
 * is something, at the end of the page, so it never moves the cards above.
 */
export function SiteSearchesCard({
  domain,
  scope,
}: {
  domain: string
  scope: Scope
}) {
  const { t } = useI18n()
  const site = useSite(domain, scope)
  const searches = site.data?.data.siteSearches ?? []
  if (searches.length === 0) return null
  return (
    <SectionCard
      title={t('insights.site.siteSearches')}
      subtitle={t('insights.site.siteSearchesHint')}
      action={<SectionStatus meta={site.data?.meta} />}
    >
      <ul className="-mx-2 flex flex-col">
        {searches.map((search) => (
          <li key={search.query}>
            <Link to={searchPath(search.query, scope)} className={rowLink}>
              <Search
                className="size-3.5 shrink-0 text-muted-foreground"
                aria-hidden
              />
              <span className="flex-1 truncate">{search.query}</span>
              <span className="font-mono text-xs text-muted-foreground">
                {t('insights.times', { count: search.count })}
              </span>
            </Link>
          </li>
        ))}
      </ul>
    </SectionCard>
  )
}

/** Other sites, most visits first, each opening its own site drill-in. */
function SiteList({
  title,
  sites,
  scope,
}: {
  title?: string
  sites: SiteFlowStat[]
  scope: Scope
}) {
  const format = useFormat()
  if (sites.length === 0) return null
  const max = Math.max(1, ...sites.map((site) => site.visitCount))
  return (
    <div className="flex flex-col gap-1">
      {title && (
        <span className="px-1 text-xs font-medium text-muted-foreground">
          {title}
        </span>
      )}
      <ul className="flex flex-col gap-1">
        {sites.map((site) => (
          <li key={site.registrableDomain}>
            <Link
              to={sitePath(site.registrableDomain, scope)}
              className="flex h-8 items-center gap-2.5 rounded-md px-1 text-[13px] transition-colors hover:bg-muted hover:no-underline"
            >
              <SiteIcon domain={site.registrableDomain} />
              <span className="flex-1 truncate font-medium">
                {site.registrableDomain}
              </span>
              <ShareBar value={site.visitCount} max={max} />
              <span className="w-10 text-right font-mono text-xs text-muted-foreground">
                {format.number(site.visitCount)}
              </span>
            </Link>
          </li>
        ))}
      </ul>
    </div>
  )
}
