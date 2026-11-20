/**
 * Records while searching the full text (`agent-history.md` §3.4): how it
 * matches and which kinds of text are searched (`search-controls.tsx`),
 * how far the search is (said aloud as it goes), and every
 * matching conversation as it arrives, in a virtualized list. When it
 * ends: the exact numbers, or that nothing matched and what was searched;
 * a regular expression that does not compile or a search that failed says
 * so instead of "no results".
 *
 * Responsible for: drawing a search's state. Not responsible for: running
 * it (`use-chat-search.ts`) or the URL (`chats-params.ts`).
 */
import { useVirtualizer } from '@tanstack/react-virtual'
import { SearchX, TriangleAlert } from 'lucide-react'
import { useMemo, useRef } from 'react'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import { Skeleton } from '@/components/ui/skeleton'
import type { ChatSearchMode } from '@/features/agent-chats/chats-params'
import {
  SEARCH_ROLES,
  type SearchCard,
  type SearchMatch,
  type SearchRequest,
  type SearchRole,
} from '@/lib/backend-client/agent-search'
import { useI18n, type Formatters } from '@/lib/i18n'
import { ResultCard } from './result-card'
import { SearchControls } from './search-controls'
import { useAnnounced } from './use-announced'
import { useChatSearch, type SearchState } from './use-chat-search'

/** The sentence under the roles: running, done, stopped. */
function statusText(
  t: ReturnType<typeof useI18n>['t'],
  f: Formatters,
  state: SearchState,
) {
  const matches = t('agentSearch.status.matches', {
    count: state.hits,
    shown: f.number(state.hits),
  })
  if (state.phase === 'done')
    return t('agentSearch.status.done', {
      conversations: t('agentSearch.status.conversations', {
        count: state.conversations,
        shown: f.number(state.conversations),
      }),
      matches,
      searched: t(
        state.filtered
          ? 'agentSearch.status.searchedFiltered'
          : 'agentSearch.status.searchedAll',
        { count: state.total ?? 0, shown: f.number(state.total ?? 0) },
      ),
    })
  const found = t('agentSearch.status.found', {
    count: state.conversations,
    shown: f.number(state.conversations),
  })
  const progress =
    state.total === null
      ? f.number(state.scanned)
      : `${f.number(state.scanned)} / ${f.number(state.total)}`
  if (state.phase === 'stopped')
    return t('agentSearch.status.stopped', { found, matches, progress })
  return t('agentSearch.status.searching', { found, matches, progress })
}

function NoResults({
  query,
  state,
  roles,
}: {
  query: string
  state: SearchState
  roles: SearchRole[]
}) {
  const { t, locale } = useI18n()
  const all = !state.filtered && roles.length === SEARCH_ROLES.length
  const kinds = new Intl.ListFormat(locale, { type: 'conjunction' }).format(
    SEARCH_ROLES.filter((role) => roles.includes(role)).map((role) =>
      t(`agentSearch.scope.${role}`),
    ),
  )
  return (
    <div className="animate-rise flex flex-col items-center gap-2 px-8 py-10 text-center">
      <SearchX className="size-6 text-muted-foreground/70" aria-hidden />
      <p className="text-[14px] font-medium">
        {t('agentSearch.empty.title', { query })}
      </p>
      <p className="text-[12.5px] text-muted-foreground">
        {all
          ? t('agentSearch.empty.all', { count: state.total ?? 0 })
          : t('agentSearch.empty.some', {
              count: state.total ?? 0,
              kinds,
            })}
      </p>
    </div>
  )
}

function Failed({
  state,
  onRetry,
}: {
  state: SearchState
  onRetry: () => void
}) {
  const { t } = useI18n()
  const regex = state.error?.code === 'invalid_regex'
  return (
    <div
      role="alert"
      className="animate-rise flex flex-col items-center gap-2 px-8 py-10 text-center"
    >
      <TriangleAlert className="size-6 text-muted-foreground/70" aria-hidden />
      <p className="text-[14px] font-medium">
        {regex
          ? t('agentSearch.error.regex')
          : state.error?.code === 'index_behind'
            ? t('agentSearch.error.index')
            : t('agentSearch.error.failed')}
      </p>
      {state.error && state.error.message !== state.error.code && (
        <p className="max-w-[320px] text-[12.5px] break-words text-muted-foreground">
          {state.error.message}
        </p>
      )}
      {!regex && (
        <Button size="sm" variant="outline" onClick={onRetry}>
          {t('agentReader.rows.retry')}
        </Button>
      )}
    </div>
  )
}

export function SearchResults({
  request,
  selectedId,
  f,
  zone,
  onMode,
  onRoles,
  onOpen,
}: {
  request: SearchRequest
  selectedId: string | null
  f: Formatters
  zone: string
  onMode: (mode: ChatSearchMode) => void
  onRoles: (roles: SearchRole[]) => void
  onOpen: (card: SearchCard, snippet: SearchMatch | null) => void
}) {
  const { t } = useI18n()
  const { state, stop, retry } = useChatSearch(request)
  const scrollRef = useRef<HTMLDivElement>(null)
  const { order, byId } = state.cards
  const running = state.phase === 'starting' || state.phase === 'searching'
  // eslint-disable-next-line react-hooks/incompatible-library -- the virtualizer is only read below
  const virtualizer = useVirtualizer({
    count: order.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 132,
    getItemKey: (index) => order[index],
    overscan: 6,
  })
  const status = statusText(t, f, state)
  const announced = useAnnounced(status, !running)
  const progress = useMemo(
    () =>
      state.total ? Math.min(100, (state.scanned / state.total) * 100) : 0,
    [state.scanned, state.total],
  )
  return (
    <section
      aria-label={t('agentSearch.results')}
      className="flex min-h-0 flex-1 flex-col"
    >
      <SearchControls
        mode="text"
        onMode={onMode}
        roles={request.roles}
        onRoles={onRoles}
      />
      {state.phase !== 'failed' && (
        <div className="flex flex-col gap-1.5 border-b px-4 pb-2">
          <div className="flex items-center gap-2">
            <p
              aria-live="polite"
              className="min-w-0 flex-1 text-[12px] text-muted-foreground tabular"
            >
              <span className="sr-only">{announced}</span>
              <span aria-hidden data-search-status="">
                {state.phase === 'starting'
                  ? t('agentSearch.status.starting')
                  : status}
              </span>
            </p>
            {running && (
              <Button size="xs" variant="outline" onClick={stop}>
                {t('agentSearch.stop')}
              </Button>
            )}
          </div>
          {running && (
            <Progress
              value={progress}
              aria-label={t('agentSearch.progress')}
              className="h-1"
            />
          )}
          {state.indexLag > 0 && (
            <p className="text-[12px] text-muted-foreground">
              {t('agentSearch.indexLag', { count: state.indexLag })}
            </p>
          )}
        </div>
      )}
      {state.phase === 'failed' ? (
        <Failed state={state} onRetry={retry} />
      ) : order.length === 0 ? (
        running ? (
          <div
            role="status"
            aria-label={t('agentSearch.status.starting')}
            className="flex flex-col gap-3 px-4 pt-3"
          >
            {Array.from({ length: 4 }, (_, index) => (
              <div key={index} className="flex flex-col gap-1.5">
                <Skeleton className="h-3.5 w-3/5" />
                <Skeleton className="h-3 w-4/5" />
              </div>
            ))}
          </div>
        ) : (
          <NoResults
            query={request.query}
            state={state}
            roles={request.roles}
          />
        )
      ) : (
        <div
          ref={scrollRef}
          role="feed"
          aria-label={t('agentSearch.results')}
          aria-busy={running}
          className="min-h-0 flex-1 overflow-y-auto"
        >
          <div
            className="relative w-full"
            style={{ height: virtualizer.getTotalSize() }}
          >
            {virtualizer.getVirtualItems().map((item) => {
              const card = byId.get(order[item.index])
              return (
                <div
                  key={item.key}
                  data-index={item.index}
                  ref={virtualizer.measureElement}
                  className="absolute top-0 left-0 w-full"
                  style={{ transform: `translateY(${item.start}px)` }}
                >
                  {card && (
                    <ResultCard
                      card={card}
                      selected={card.id === selectedId}
                      f={f}
                      zone={zone}
                      onOpen={onOpen}
                    />
                  )}
                </div>
              )
            })}
          </div>
        </div>
      )}
    </section>
  )
}
