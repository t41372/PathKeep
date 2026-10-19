/**
 * Pages the user went back and forth between within one search: product
 * pages on two shops, two libraries' docs, the same release on two sites.
 * Newest first; each search links to its drill-in, its date to the day, each
 * page to its own. Every page of the set is shown, the one first opened from
 * the search marked, with how often it was opened when more than once.
 */
import { Link } from 'react-router-dom'
import { SiteIcon } from '@/features/history/site-icon'
import { SectionCard } from '@/components/app/section-card'
import { useI18n } from '@/lib/i18n'
import { cn } from '@/lib/cn'
import { dayPath, pagePath, searchPath } from './links'
import { ScrollList } from '@/components/app/scroll-list'
import { ListSkeleton, QueryBody, SectionStatus } from './parts'
import { useCompareSets } from './queries'
import type { Scope } from './range'
import { useInsightsFormat } from './zone'
import { dateKeyIn } from '@/lib/time-zone'

export function ComparedPagesCard({
  scope,
  empty,
}: {
  scope: Scope
  /** What to say when no search in the scope had compared pages. */
  empty?: string
}) {
  const { t } = useI18n()
  const format = useInsightsFormat()
  const sets = useCompareSets(scope)
  return (
    <SectionCard
      title={t('insights.compared.title')}
      subtitle={t('insights.compared.subtitle')}
      action={
        <SectionStatus
          meta={sets.data?.meta}
          explain={t('insights.compared.explain')}
        />
      }
    >
      <QueryBody
        query={sets}
        skeleton={<ListSkeleton rows={3} height={64} />}
        isEmpty={(result) => result.data.items.length === 0}
        empty={empty ?? t('insights.compared.empty')}
      >
        {(result) => (
          <ScrollList maxHeight={3 * 92}>
            <ul className="flex flex-col gap-3">
              {result.data.items.map((set) => {
                const sites = new Set(
                  set.pages.map((page) => page.registrableDomain),
                ).size
                return (
                  <li key={set.compareSetId} className="flex flex-col gap-1.5">
                    <span className="flex items-baseline justify-between gap-2 text-[13px]">
                      <Link
                        to={searchPath(set.searchQuery, scope)}
                        className="truncate font-medium hover:underline"
                      >
                        {set.searchQuery}
                      </Link>
                      <Link
                        to={dayPath(
                          dateKeyIn(Date.parse(set.lastVisitedAt), scope.zone),
                          scope,
                        )}
                        title={t('insights.compared.openDay')}
                        className="shrink-0 text-xs text-muted-foreground hover:text-foreground"
                      >
                        {format.relative(set.lastVisitedAt)}
                      </Link>
                    </span>
                    <span className="flex flex-wrap gap-1.5">
                      {set.pages.map((page) => (
                        <Link
                          key={page.canonicalUrl}
                          to={pagePath(page.canonicalUrl, scope)}
                          title={
                            page.isLanding
                              ? t('insights.compared.landing')
                              : undefined
                          }
                          className={cn(
                            'flex h-7 max-w-[220px] items-center gap-1.5 rounded-full border bg-card px-2.5 text-xs transition-colors hover:bg-muted hover:no-underline',
                            page.isLanding && 'border-brand/50',
                          )}
                        >
                          <SiteIcon domain={page.registrableDomain} />
                          <span className="truncate">
                            {page.title || page.registrableDomain}
                          </span>
                          {page.visitCount > 1 && (
                            <span className="font-mono text-[11px] text-muted-foreground">
                              {t('insights.times', { count: page.visitCount })}
                            </span>
                          )}
                          {page.isLanding && (
                            <span className="sr-only">
                              {t('insights.compared.landing')}
                            </span>
                          )}
                        </Link>
                      ))}
                    </span>
                    <span className="text-xs text-muted-foreground">
                      {t('insights.compared.pages', {
                        count: set.pages.length,
                        pages: format.number(set.pages.length),
                      })}{' '}
                      {t('insights.compared.sites', {
                        count: sites,
                        sites: format.number(sites),
                      })}
                    </span>
                  </li>
                )
              })}
            </ul>
          </ScrollList>
        )}
      </QueryBody>
    </SectionCard>
  )
}
