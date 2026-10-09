/**
 * Pages the user went back and forth between within one search: product
 * pages on two shops, two libraries' docs, the same release on two sites.
 * Newest first, six at most; each search links to its drill-in, each page to
 * its own.
 */
import { Link } from 'react-router-dom'
import { Favicon } from '@/components/app/favicon'
import { SectionCard } from '@/components/app/section-card'
import { useFormat, useI18n } from '@/lib/i18n'
import { pagePath, searchPath } from './links'
import { ListSkeleton, QueryBody, ScrollList, SectionStatus } from './parts'
import { useCompareSets } from './queries'
import type { Scope } from './range'

export function ComparedPagesCard({ scope }: { scope: Scope }) {
  const { t } = useI18n()
  const format = useFormat()
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
        empty={t('insights.compared.empty')}
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
                      <span className="shrink-0 text-xs text-muted-foreground">
                        {format.relative(set.lastVisitedAt)}
                      </span>
                    </span>
                    <span className="flex flex-wrap gap-1.5">
                      {set.pages.slice(0, 3).map((page) => (
                        <Link
                          key={page.canonicalUrl}
                          to={pagePath(page.canonicalUrl, scope)}
                          className="flex h-7 max-w-[220px] items-center gap-1.5 rounded-full border bg-card px-2.5 text-xs transition-colors hover:bg-muted hover:no-underline"
                        >
                          <Favicon domain={page.registrableDomain} />
                          <span className="truncate">
                            {page.title || page.registrableDomain}
                          </span>
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
