/**
 * Records while searching by meaning: the same two rows of choices as a
 * full-text search, then either what stands in the way (AI settings, the
 * switch, the index; `semantic-blocked.tsx`) or the passages the semantic
 * index finds, closest first, as they stream, in a virtualized list. A
 * line says how many so far (aloud, at most once a second) and when they
 * are all in; another says which model built the index, when, and whether
 * conversations changed since.
 *
 * Responsible for: drawing a semantic search's state, and asking for the
 * index's state again when the backend refuses a search the screen thought
 * it could run. Not responsible for: running it (`use-semantic-search.ts`),
 * deciding whether it can run (`semantic-gate.ts`) or the URL
 * (`chats-params.ts`).
 */
import { useVirtualizer } from '@tanstack/react-virtual'
import { SearchX, Sparkles, TriangleAlert } from 'lucide-react'
import { useEffect, useRef } from 'react'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import type {
  ChatSearchMode,
  RecordAt,
} from '@/features/agent-chats/chats-params'
import type { SemanticGeneration } from '@/lib/backend-client/agent-semantic'
import type {
  SearchRole,
  SemanticPassage,
  SemanticSearchRequest,
} from '@/lib/backend-client/agent-search'
import { useI18n, type Formatters } from '@/lib/i18n'
import { queryClient } from '@/lib/query'
import { useSnapshot } from '@/lib/queries/app'
import { semanticIndexKey } from '@/lib/queries/agent-semantic'
import { BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID } from '@/lib/types'
import { PassageRow } from './passage-row'
import { SearchControls } from './search-controls'
import { SemanticBlocked } from './semantic-blocked'
import { useSemanticGate, type SemanticGate } from './semantic-gate'
import { useAnnounced } from './use-announced'
import { useSemanticSearch, type SemanticState } from './use-semantic-search'

type Translate = ReturnType<typeof useI18n>['t']

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
    default:
      return count === 0
        ? t('agentSearch.semantic.status.starting')
        : t('agentSearch.semantic.status.searching', shown)
  }
}

/** Which model made the index, when, and whether it is behind. */
function IndexLine({
  gate,
  stale,
  f,
}: {
  gate: Extract<SemanticGate, { kind: 'ready' }>
  stale: boolean
  f: Formatters
}) {
  const { t } = useI18n()
  const model = useModelName(gate.serving)
  return (
    <p className="flex items-start gap-1.5 text-[12px] leading-snug text-muted-foreground">
      <Sparkles className="mt-px size-3.5 shrink-0 text-brand" aria-hidden />
      <span>
        {t('agentSearch.semantic.using', {
          model,
          when: f.relative(gate.serving.builtAt),
        })}
        {(stale || gate.stale) &&
          ` · ${t(gate.job ? 'agentSearch.semantic.updating' : 'agentSearch.semantic.stale')}`}
      </span>
    </p>
  )
}

function Passages({
  request,
  gate,
  selected,
  f,
  zone,
  onTextInstead,
  onOpen,
}: {
  request: SemanticSearchRequest
  gate: Extract<SemanticGate, { kind: 'ready' }>
  selected: { chat: string | null; at: RecordAt | null }
  f: Formatters
  zone: string
  onTextInstead: () => void
  onOpen: (passage: SemanticPassage) => void
}) {
  const { t } = useI18n()
  const { state, stop, retry } = useSemanticSearch(request)
  const scrollRef = useRef<HTMLDivElement>(null)
  const { items, count } = state.passages
  const running = state.phase === 'starting' || state.phase === 'searching'
  // eslint-disable-next-line react-hooks/incompatible-library -- the virtualizer is only read below
  const virtualizer = useVirtualizer({
    count,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 104,
    overscan: 6,
  })
  const status = statusText(t, f, state)
  const announced = useAnnounced(status, !running)

  // Refused although the index looked ready: the switch, the AI settings or
  // the model changed since the status was read. Read it again; the gate
  // above then says what changed.
  const failed = state.phase === 'failed'
  useEffect(() => {
    if (failed)
      void queryClient.invalidateQueries({ queryKey: semanticIndexKey })
  }, [failed])

  if (state.error?.code === 'agent-semantic-index-not-ready')
    return (
      <SemanticBlocked
        block={{ kind: 'notReady' }}
        f={f}
        onTextInstead={onTextInstead}
        onRetry={retry}
      />
    )
  if (state.phase === 'failed')
    return (
      <div
        role="alert"
        className="animate-rise flex flex-col items-center gap-2 px-8 py-10 text-center"
      >
        <TriangleAlert
          className="size-6 text-muted-foreground/70"
          aria-hidden
        />
        <p className="text-[14px] font-medium">
          {t('agentSearch.error.failed')}
        </p>
        {state.error && state.error.message !== state.error.code && (
          <p className="max-w-[320px] text-[12.5px] break-words text-muted-foreground">
            {state.error.message}
          </p>
        )}
        <Button size="sm" variant="outline" onClick={retry}>
          {t('common.retry')}
        </Button>
      </div>
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
        <IndexLine gate={gate} stale={state.stale} f={f} />
      </div>
      {count === 0 ? (
        running ? (
          <div
            role="status"
            aria-label={t('agentSearch.semantic.status.starting')}
            className="flex flex-col gap-4 px-4 pt-3"
          >
            {Array.from({ length: 4 }, (_, index) => (
              <div key={index} className="flex flex-col gap-1.5">
                <Skeleton className="h-3.5 w-3/5" />
                <Skeleton className="h-3 w-2/5" />
                <Skeleton className="h-3 w-full" />
              </div>
            ))}
          </div>
        ) : (
          <div className="animate-rise flex flex-col items-center gap-2 px-8 py-10 text-center">
            <SearchX className="size-6 text-muted-foreground/70" aria-hidden />
            <p className="text-[14px] font-medium">
              {t('agentSearch.semantic.empty.title', { query: request.query })}
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
            {virtualizer.getVirtualItems().map((item) => {
              const passage = items[item.index]
              const { address } = passage
              return (
                <div
                  key={item.key}
                  data-index={item.index}
                  ref={virtualizer.measureElement}
                  className="absolute top-0 left-0 w-full"
                  style={{ transform: `translateY(${item.start}px)` }}
                >
                  <PassageRow
                    passage={passage}
                    selected={
                      address.conversationId === selected.chat &&
                      address.sourceId === selected.at?.sourceId &&
                      address.ordinal === selected.at.ordinal
                    }
                    f={f}
                    zone={zone}
                    onOpen={onOpen}
                  />
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
  onRoles: (roles: SearchRole[]) => void
  onOpen: (passage: SemanticPassage) => void
}) {
  const { t } = useI18n()
  const gate = useSemanticGate()
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
        <div
          role="status"
          aria-label={t('agentSearch.semantic.status.starting')}
          className="flex flex-col gap-4 border-t px-4 pt-3"
        >
          {Array.from({ length: 4 }, (_, index) => (
            <div key={index} className="flex flex-col gap-1.5">
              <Skeleton className="h-3.5 w-3/5" />
              <Skeleton className="h-3 w-full" />
            </div>
          ))}
        </div>
      ) : gate.kind === 'ready' ? (
        <Passages
          request={request}
          gate={gate}
          selected={selected}
          f={f}
          zone={zone}
          onTextInstead={textInstead}
          onOpen={onOpen}
        />
      ) : (
        <SemanticBlocked
          block={gate}
          f={f}
          onTextInstead={textInstead}
          onRetry={() =>
            void queryClient.invalidateQueries({ queryKey: semanticIndexKey })
          }
        />
      )}
    </section>
  )
}
