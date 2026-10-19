/**
 * Responsibilities: show up to eight semantic neighbours with all-time search evidence.
 * Not responsible for: lexical wordings, clustering or embedding calls.
 * Failure modes: old results replace a new query; unavailable service leaves an empty card;
 * wordings already shown appear again; loading/error/not-indexed states disappear.
 * Tests: semantic-index.spec.ts records rows, navigation and a screenshot.
 */
import { useQuery } from '@tanstack/react-query'
import { Link } from 'react-router-dom'
import { CardError, SectionCard } from '@/components/app/section-card'
import { useSemanticInfo } from '@/features/history/semantic-info'
import { similarSearchesClient } from '@/lib/backend-client/similar-searches'
import { useI18n } from '@/lib/i18n'
import { queryKeys } from '@/lib/query'
import { rowLink } from './display'
import { searchPath } from './links'
import { ListSkeleton } from './parts'
import type { Scope } from './range'
import { useInsightsFormat } from './zone'

/** Suggestions cross the current range, while their links retain the drill-in's range. */
export function SimilarSearches({
  query,
  wordings,
  scope,
  wordingsLoading,
}: {
  query: string
  wordings: string[]
  scope: Scope
  wordingsLoading: boolean
}) {
  const { t } = useI18n()
  const format = useInsightsFormat()
  const semantic = useSemanticInfo()
  const result = useQuery({
    queryKey: [
      ...queryKeys.archiveData,
      'similar-searches',
      query,
      wordings,
      semantic.provider,
      semantic.lastIndexedAt,
      semantic.available,
    ],
    queryFn: () => similarSearchesClient.get(query, wordings),
    enabled: semantic.available && !wordingsLoading,
    refetchOnWindowFocus: false,
  })
  const data = result.data
  if (!semantic.available || data?.state === 'unavailable') return null
  return (
    <SectionCard
      title={t('insights.search.similar.title')}
      subtitle={t('insights.search.similar.hint')}
    >
      {data?.staticModel && (
        <p className="mb-2 text-xs text-muted-foreground">
          {t('insights.search.similar.staticNote')}
        </p>
      )}
      {result.isPending ? (
        <div role="status" aria-label={t('insights.search.similar.loading')}>
          <ListSkeleton rows={3} />
        </div>
      ) : result.isError ? (
        <CardError error={result.error} onRetry={() => void result.refetch()} />
      ) : data?.state === 'ready' && data.items.length > 0 ? (
        <ul className="-mx-2 flex flex-col" data-similar-searches>
          {data.items.map((row) => (
            <li key={row.query}>
              <Link
                to={searchPath(row.query, scope)}
                className={rowLink}
                data-similar-query={row.query}
                aria-label={t('insights.search.similar.open', {
                  query: row.query,
                })}
              >
                <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                  <span className="truncate font-medium">{row.query}</span>
                  <span className="text-xs text-muted-foreground">
                    {t('insights.times', {
                      count: format.number(row.searches),
                    })}{' '}
                    · {format.relative(row.lastSearchedAt)}
                  </span>
                </span>
              </Link>
            </li>
          ))}
        </ul>
      ) : (
        <p className="text-sm text-muted-foreground" role="status">
          {t(
            data?.state === 'notIndexed'
              ? 'insights.search.similar.notIndexed'
              : 'insights.search.similar.empty',
          )}
        </p>
      )}
    </SectionCard>
  )
}
