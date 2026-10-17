/**
 * Responsibilities: show related pages and pass a clicked page to History's selection.
 * Not responsible for: ranking, changing the list or calling an embedding service.
 * Failure modes: a late request replaces a newer selection; an empty state hides a load error.
 * Tests: semantic-index.spec.ts covers folding, visibility, navigation and optional availability.
 */
import { useId } from 'react'
import { useQuery } from '@tanstack/react-query'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { relatedPagesClient } from '@/lib/backend-client/related-pages'
import { useFormat, useI18n } from '@/lib/i18n'
import { queryKeys } from '@/lib/query'
import type { DetailTarget } from './history-types'
import { useSemanticInfo } from './semantic-info'
import { SiteIcon } from './site-icon'

/** Selection stays in the panel, including when a related page is outside the current list. */
export function RelatedPages({
  target,
  onSelect,
}: {
  target: DetailTarget
  onSelect: (target: DetailTarget) => void
}) {
  const { t } = useI18n()
  const format = useFormat()
  const heading = useId()
  const semantic = useSemanticInfo()
  const query = useQuery({
    queryKey: [
      ...queryKeys.archiveData,
      'related-pages',
      target.url,
      target.visitId ?? null,
      semantic.provider,
      semantic.lastIndexedAt,
      semantic.available,
    ],
    queryFn: () => relatedPagesClient.get(target.url, target.visitId),
    enabled: semantic.available,
    refetchOnWindowFocus: false,
    refetchInterval: (query) =>
      query.state.data?.state === 'updating' ? 5000 : false,
  })
  const data = query.data
  if (!semantic.available || data?.state === 'unavailable') return null

  return (
    <section aria-labelledby={heading} className="flex flex-col gap-1.5">
      <h3 id={heading} className="text-xs text-muted-foreground">
        {t('historyPage.related.label')}
      </h3>
      {data?.staticModel && (
        <p className="text-xs text-muted-foreground">
          {t('historyPage.related.staticNote')}
        </p>
      )}
      {query.isPending ? (
        <div role="status" aria-label={t('historyPage.related.loading')}>
          {[0, 1, 2].map((index) => (
            <div
              key={index}
              className="flex items-center gap-2 py-1.5"
              aria-hidden
            >
              <Skeleton className="size-6 shrink-0 rounded-md" />
              <div className="flex flex-1 flex-col gap-1.5">
                <Skeleton className="h-3.5 w-5/6" />
                <Skeleton className="h-3 w-1/2" />
              </div>
            </div>
          ))}
        </div>
      ) : query.isError ? (
        <div className="flex flex-col items-start gap-1">
          <p className="text-xs text-muted-foreground">
            {t('historyPage.related.failed')}
          </p>
          <Button
            variant="outline"
            size="xs"
            onClick={() => void query.refetch()}
          >
            {t('common.retry')}
          </Button>
        </div>
      ) : data?.state === 'ready' && data.items.length > 0 ? (
        <ul className="flex flex-col gap-0.5">
          {data.items.map((row) => (
            <li key={row.historyId}>
              <button
                type="button"
                data-related-url={row.url}
                aria-label={t('historyPage.related.show', {
                  title: row.title?.trim() || row.url,
                })}
                onClick={() =>
                  onSelect({
                    url: row.url,
                    title: row.title,
                    domain: row.domain,
                    visitId: row.historyId,
                    profileId: row.profileId,
                  })
                }
                className="flex w-full items-center gap-2 rounded py-1.5 text-left outline-none transition-colors duration-150 hover:text-brand focus-visible:ring-2 focus-visible:ring-ring/50"
              >
                <SiteIcon
                  domain={row.domain}
                  className="size-6 shrink-0 rounded-md"
                />
                <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                  <span className="truncate text-[13px]">
                    {row.title?.trim() || row.url}
                  </span>
                  <span className="truncate text-xs text-muted-foreground">
                    {row.domain} ·{' '}
                    {format.relative(row.lastVisitTime, undefined, 'elapsed')}
                  </span>
                </span>
              </button>
            </li>
          ))}
        </ul>
      ) : (
        <p className="text-xs text-muted-foreground" role="status">
          {t(
            data?.state === 'notIndexed'
              ? 'historyPage.related.notIndexed'
              : data?.state === 'updating'
                ? 'historyPage.related.updating'
                : 'historyPage.related.empty',
          )}
        </p>
      )}
    </section>
  )
}
