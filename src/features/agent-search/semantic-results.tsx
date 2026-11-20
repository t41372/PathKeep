/**
 * Records while searching by meaning: the same two rows of choices as a
 * full-text search, then either what stands in the way (the index's own
 * state, or the backend's refusal; `semantic-blocked.tsx`) or the passages,
 * closest first, a page of up to 100 at a time, in a virtualized list that
 * asks for the next page as it nears its end, until there are no more. A
 * line says how many are shown and whether more load as you scroll or
 * that's all (aloud, at most once a second); another says which model built
 * the index, when, and whether conversations changed since.
 *
 * Responsible for: drawing a semantic search's state, and asking for the
 * next page near the end of the list. Not responsible for: running it
 * (`use-semantic-search.ts`), deciding whether to send it
 * (`semantic-gate.ts`) or the URL (`chats-params.ts`).
 */
import { useVirtualizer } from '@tanstack/react-virtual'
import { SearchX, Sparkles, TriangleAlert } from 'lucide-react'
import { useCallback, useEffect, useRef } from 'react'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import {
  refusalWords,
  type RefusalWords,
} from '@/features/ai-setup/ai-refusals'
import type {
  ChatSearchMode,
  RecordAt,
} from '@/features/agent-chats/chats-params'
import type { SemanticGeneration } from '@/lib/backend-client/agent-semantic'
import type {
  SemanticPassage,
  SemanticSearchRequest,
} from '@/lib/backend-client/agent-search'
import { cn } from '@/lib/cn'
import { useI18n, type Formatters } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import { refetchSemanticIndex } from '@/lib/queries/agent-semantic'
import { BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID } from '@/lib/types'
import { PassageRow } from './passage-row'
import { SearchControls, type RolesChange } from './search-controls'
import { SemanticBlocked } from './semantic-blocked'
import { useSemanticGate, type SemanticGate } from './semantic-gate'
import { SemanticListEnd } from './semantic-list-end'
import { useAnnounced } from './use-announced'
import {
  useSemanticSearch,
  type SemanticPhase,
  type SemanticState,
} from './use-semantic-search'

type Translate = ReturnType<typeof useI18n>['t']
type IndexState = Extract<SemanticGate, { kind: 'search' }>

/** The next page is asked for once the list draws this close to its end. */
const NEAR_END = 20

/** Said aloud at once; the others wait their turn (once a second). */
const SETTLED = new Set<SemanticPhase>([
  'done',
  'stopped',
  'failed',
  'refused',
  'notReady',
])

/**
 * The list ends with `SemanticListEnd` instead of a passage: a page on its
 * way, stopped, failed, or refused for a cause that can pass.
 */
function hasListEnd(state: SemanticState, words: RefusalWords | null) {
  switch (state.phase) {
    case 'loading':
    case 'stopped':
    case 'failed':
      return true
    case 'refused':
      return words?.retry === true
    default:
      return false
  }
}

/** The in-app model by its name; a service by its name and the model it ran. */
function useModelName(serving: SemanticGeneration) {
  const { t } = useI18n()
  const providers = useSnapshot().config.ai.embeddingProviders
  if (serving.provider === BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID)
    return t('settingsAi.semantic.switch.inAppName')
  const service = providers.find((item) => item.id === serving.provider)
  return service ? `${service.name} · ${serving.model}` : serving.model
}

function statusText(t: Translate, f: Formatters, state: SemanticState) {
  const count = state.passages.count
  const shown = { count, shown: f.number(count) }
  switch (state.phase) {
    case 'starting':
      return t('agentSearch.semantic.status.starting')
    case 'done':
      return t('agentSearch.semantic.status.done', shown)
    case 'stopped':
      return t('agentSearch.semantic.status.stopped', shown)
    case 'failed':
    case 'refused':
    case 'notReady':
      return t('agentSearch.semantic.status.halted', shown)
    default:
      return t('agentSearch.semantic.status.more', shown)
  }
}

/** Which model made the index, when, and whether it is behind. */
function IndexLine({
  index,
  serving,
  stale,
  f,
}: {
  index: IndexState
  serving: SemanticGeneration
  stale: boolean
  f: Formatters
}) {
  const { t } = useI18n()
  const model = useModelName(serving)
  return (
    <p className="flex items-start gap-1.5 text-[12px] leading-snug text-muted-foreground">
      <Sparkles className="mt-px size-3.5 shrink-0 text-brand" aria-hidden />
      <span>
        {t('agentSearch.semantic.using', {
          model,
          when: f.relative(serving.builtAt),
        })}
        {(stale || index.stale) &&
          ` · ${t(index.job ? 'agentSearch.semantic.updating' : 'agentSearch.semantic.stale')}`}
      </span>
    </p>
  )
}

/** Passage-shaped placeholders while the first page (or the index's state) is read. */
function Placeholder({ className }: { className?: string }) {
  const { t } = useI18n()
  return (
    <div
      role="status"
      aria-label={t('agentSearch.semantic.status.starting')}
      className={cn('flex flex-col gap-4 px-4 pt-3', className)}
    >
      {Array.from({ length: 4 }, (_, index) => (
        <div key={index} className="flex flex-col gap-1.5">
          <Skeleton className="h-3.5 w-3/5" />
          <Skeleton className="h-3 w-2/5" />
          <Skeleton className="h-3 w-full" />
        </div>
      ))}
    </div>
  )
}

function Failed({
  state,
  onRetry,
}: {
  state: SemanticState
  onRetry: () => void
}) {
  const { t } = useI18n()
  return (
    <div
      role="alert"
      className="animate-rise flex flex-col items-center gap-2 px-8 py-10 text-center"
    >
      <TriangleAlert className="size-6 text-muted-foreground/70" aria-hidden />
      <p className="text-[14px] font-medium">{t('agentSearch.error.failed')}</p>
      {state.error && state.error.message !== state.error.code && (
        <p className="max-w-[320px] text-[12.5px] break-words text-muted-foreground">
          {state.error.message}
        </p>
      )}
      <Button size="sm" variant="outline" onClick={onRetry}>
        {t('common.retry')}
      </Button>
    </div>
  )
}

function Passages({
  query,
  search,
  index,
  selected,
  f,
  zone,
  onTextInstead,
  onRetry,
  onOpen,
}: {
  query: string
  search: ReturnType<typeof useSemanticSearch>
  index: IndexState
  selected: { chat: string | null; at: RecordAt | null }
  f: Formatters
  zone: string
  onTextInstead: () => void
  /** The status read again, then the page that failed or was refused again. */
  onRetry: () => void
  onOpen: (passage: SemanticPassage) => void
}) {
  const { t } = useI18n()
  const { state, more, stop, retry } = search
  const scrollRef = useRef<HTMLDivElement>(null)
  const { items, count } = state.passages
  const running = state.phase === 'starting' || state.phase === 'loading'
  const words = state.refusal
    ? refusalWords(t, state.refusal, 'semantic')
    : null
  const tail = count > 0 && hasListEnd(state, words)
  // eslint-disable-next-line react-hooks/incompatible-library -- the virtualizer is only read below
  const virtualizer = useVirtualizer({
    count: count + (tail ? 1 : 0),
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 104,
    overscan: 6,
  })
  const rows = virtualizer.getVirtualItems()
  const status = statusText(t, f, state)
  const announced = useAnnounced(status, SETTLED.has(state.phase))

  const lastDrawn = rows.at(-1)?.index ?? -1
  useEffect(() => {
    if (state.phase === 'more' && lastDrawn >= count - NEAR_END) more()
  }, [state.phase, lastDrawn, count, more])

  if (state.phase === 'notReady')
    return (
      <SemanticBlocked
        block={{ kind: 'notReady' }}
        f={f}
        onTextInstead={onTextInstead}
        onRetry={onRetry}
      />
    )
  // A setting stands in the way: nothing more until it changes. A cause
  // that can pass keeps the passages, with Retry at the end.
  if (state.phase === 'refused' && state.refusal && !tail)
    return (
      <SemanticBlocked
        block={{ kind: 'refused', code: state.refusal }}
        f={f}
        onTextInstead={onTextInstead}
        onRetry={onRetry}
      />
    )
  if (state.phase === 'failed' && count === 0)
    return <Failed state={state} onRetry={onRetry} />

  const end = (
    <SemanticListEnd
      state={state}
      words={words}
      onRetry={state.phase === 'stopped' ? retry : onRetry}
    />
  )
  return (
    <>
      <div className="flex flex-col gap-1.5 border-b px-4 pb-2">
        <div className="flex min-h-6 items-center gap-2">
          <p
            aria-live="polite"
            className="min-w-0 flex-1 text-[12px] text-muted-foreground tabular"
          >
            <span className="sr-only">{announced}</span>
            <span aria-hidden data-search-status="">
              {status}
            </span>
          </p>
          {running && (
            <Button size="xs" variant="outline" onClick={stop}>
              {t('agentSearch.stop')}
            </Button>
          )}
        </div>
        {index.serving && (
          <IndexLine
            index={index}
            serving={index.serving}
            stale={state.stale}
            f={f}
          />
        )}
      </div>
      {count === 0 ? (
        state.phase === 'starting' ? (
          <Placeholder />
        ) : state.phase === 'stopped' ? (
          end
        ) : (
          <div className="animate-rise flex flex-col items-center gap-2 px-8 py-10 text-center">
            <SearchX className="size-6 text-muted-foreground/70" aria-hidden />
            <p className="text-[14px] font-medium">
              {t('agentSearch.semantic.empty.title', { query })}
            </p>
            <p className="text-[12.5px] text-muted-foreground">
              {t('agentSearch.semantic.empty.body')}
            </p>
            <Button size="sm" variant="outline" onClick={onTextInstead}>
              {t('agentSearch.semantic.textInstead')}
            </Button>
          </div>
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
            {rows.map((item) => {
              const passage = items[item.index] as SemanticPassage | undefined
              return (
                <div
                  key={item.key}
                  data-index={item.index}
                  ref={virtualizer.measureElement}
                  className="absolute top-0 left-0 w-full"
                  style={{ transform: `translateY(${item.start}px)` }}
                >
                  {item.index < count && passage ? (
                    <PassageRow
                      passage={passage}
                      selected={
                        passage.address.conversationId === selected.chat &&
                        passage.address.sourceId === selected.at?.sourceId &&
                        passage.address.ordinal === selected.at.ordinal
                      }
                      f={f}
                      zone={zone}
                      onOpen={onOpen}
                    />
                  ) : (
                    end
                  )}
                </div>
              )
            })}
          </div>
        </div>
      )}
    </>
  )
}

export function SemanticResults({
  request,
  selected,
  f,
  zone,
  onMode,
  onRoles,
  onOpen,
}: {
  request: SemanticSearchRequest
  /** What the reader shows, to mark its passages. */
  selected: { chat: string | null; at: RecordAt | null }
  f: Formatters
  zone: string
  onMode: (mode: ChatSearchMode) => void
  onRoles: (change: RolesChange) => void
  onOpen: (passage: SemanticPassage) => void
}) {
  const { t } = useI18n()
  const { gate, signature } = useSemanticGate()
  // Here rather than in the list, so reading the status again after a
  // refusal never starts the search over by itself.
  const search = useSemanticSearch(
    gate.kind === 'search' ? request : null,
    signature,
  )
  const { retry } = search
  const again = useCallback(() => {
    refetchSemanticIndex()
    retry()
  }, [retry])
  const textInstead = () => onMode('text')
  return (
    <section
      aria-label={t('agentSearch.results')}
      className="flex min-h-0 flex-1 flex-col"
    >
      <SearchControls
        mode="semantic"
        onMode={onMode}
        roles={request.roles}
        onRoles={onRoles}
      />
      {gate.kind === 'loading' ? (
        <Placeholder className="border-t" />
      ) : gate.kind === 'search' ? (
        <Passages
          query={request.query}
          search={search}
          index={gate}
          selected={selected}
          f={f}
          zone={zone}
          onTextInstead={textInstead}
          onRetry={again}
          onOpen={onOpen}
        />
      ) : (
        <SemanticBlocked
          block={gate}
          f={f}
          onTextInstead={textInstead}
          onRetry={refetchSemanticIndex}
        />
      )}
    </section>
  )
}
