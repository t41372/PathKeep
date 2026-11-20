/**
 * The Records workbench (`agent-history.md` §3, §5.10): the conversation
 * list, or the search results while searching (full text or semantic), on
 * the left; the reader of the selected conversation on the right. History
 * is the workbench over every conversation; a project's page is the same
 * workbench with the project fixed. Search and its mode, filters, sort,
 * group, the open conversation and the reader's find all live in the URL.
 *
 * Responsible for: wiring the URL, the list query, the search, the marks
 * and the reader together. Not responsible for: rows, menus, results or
 * the reader itself (the sibling files, `features/agent-search` and
 * `features/agent-reader`).
 */
import { MessagesSquare } from 'lucide-react'
import { useCallback, useMemo } from 'react'
import { useNavigate } from 'react-router-dom'
import { dragRegion } from '@/components/app/drag-region'
import { Kbd } from '@/components/ui/kbd'
import { projectName } from '@/features/agent-projects/project-words'
import { Reader } from '@/features/agent-reader/reader'
import { SearchBox } from '@/features/agent-search/search-box'
import { SearchResults } from '@/features/agent-search/search-results'
import { SemanticResults } from '@/features/agent-search/semantic-results'
import { useInsightsFormat, useInsightsZone } from '@/features/insights/zone'
import type { ChatSummary } from '@/lib/backend-client/agent-chats'
import type {
  SearchCard,
  SearchMatch,
  SemanticPassage,
} from '@/lib/backend-client/agent-search'
import { useI18n } from '@/lib/i18n'
import { useAgentKind } from '@/lib/queries/agent-backup'
import { useAgentChatsSync, useChatList } from '@/lib/queries/agent-chats'
import { useProjectList } from '@/lib/queries/agent-projects'
import { shortcutLabel } from '@/lib/runtime'
import { agentProjectPath } from '@/lib/spaces'
import { useChatMarks } from './chat-marks'
import { ChatList } from './chat-list'
import {
  conditionCount,
  listQuery,
  searchRequest,
  semanticRequest,
  useChatsParams,
} from './chats-params'
import { FilterChips, FilterMenu } from './filter-menu'
import { buildList } from './list-model'
import {
  ListFailed,
  ListNote,
  ListSkeleton,
  NoConversations,
  NoMatch,
} from './list-states'
import { RowMenu } from './row-menu'

function EmptyReader({ scoped }: { scoped: boolean }) {
  const { t } = useI18n()
  return (
    <div
      className="flex flex-1 items-center justify-center p-10"
      {...dragRegion()}
    >
      <div className="animate-rise flex max-w-[360px] flex-col items-center gap-3 text-center">
        <MessagesSquare
          className="size-8 text-muted-foreground/70"
          strokeWidth={1.5}
          aria-hidden
        />
        <p className="text-[14px] font-medium">
          {t('agentChats.emptyReader.title')}
        </p>
        <div className="flex flex-col gap-1.5 text-[12.5px] text-muted-foreground">
          <span>
            <Kbd>↑</Kbd> <Kbd>↓</Kbd> {t('agentChats.emptyReader.switch')}
          </span>
          <span>
            <Kbd>/</Kbd>{' '}
            {t(
              scoped
                ? 'agentSearch.hint.project'
                : 'agentSearch.hint.everything',
            )}
          </span>
          <span>
            <Kbd>{shortcutLabel('K')}</Kbd>{' '}
            {t('agentChats.emptyReader.palette')}
          </span>
        </div>
      </div>
    </div>
  )
}

export function Workbench({
  fixedProject,
}: {
  /** A project's page: its id. History: `null`. */
  fixedProject: string | null
}) {
  const { t } = useI18n()
  const navigate = useNavigate()
  const [params, update] = useChatsParams(fixedProject)
  const zone = useInsightsZone().shown
  const f = useInsightsFormat()
  const { row } = useAgentKind()
  const query = useMemo(() => listQuery(params, zone), [params, zone])
  const listKey = useMemo(() => JSON.stringify(query), [query])
  const search = useMemo(() => searchRequest(params, zone), [params, zone])
  const semantic = useMemo(
    () => (params.mode === 'semantic' ? semanticRequest(params, zone) : null),
    [params, zone],
  )
  const list = useChatList(query, search === null)
  const marks = useChatMarks()
  useAgentChatsSync()
  // The chip's name for a project picked in the filter.
  const projects = useProjectList({}, params.project !== null && !fixedProject)

  const pages = list.data?.pages
  // While another grouping loads, the old pages stay on screen (dimmed),
  // drawn as the grouping they came in.
  const shownGroup = pages?.[0]?.group ?? params.group
  const model = useMemo(
    () => buildList(pages ?? [], shownGroup),
    [pages, shownGroup],
  )
  const select = useCallback(
    (chat: ChatSummary) => update({ chat: chat.id, find: null, at: null }),
    [update],
  )
  const close = useCallback(
    () => update({ chat: null, find: null, at: null }),
    [update],
  )
  const openHit = useCallback(
    (card: SearchCard, snippet: SearchMatch | null) =>
      update({
        chat: card.id,
        find: params.q,
        at: snippet?.address
          ? {
              sourceId: snippet.address.sourceId,
              ordinal: snippet.address.ordinal,
            }
          : null,
      }),
    [update, params.q],
  )
  // A passage has no words to find: the reader opens at its record, in the
  // conversation that holds it (a subagent's on its own), since the reader
  // reaches only the records of the conversation it shows.
  const openPassage = useCallback(
    (passage: SemanticPassage) =>
      update({
        chat: passage.address.conversationId,
        find: null,
        at: {
          sourceId: passage.address.sourceId,
          ordinal: passage.address.ordinal,
        },
      }),
    [update],
  )
  const { hasNextPage, isFetchingNextPage, fetchNextPage } = list
  const more = useCallback(() => {
    if (hasNextPage && !isFetchingNextPage) void fetchNextPage()
  }, [hasNextPage, isFetchingNextPage, fetchNextPage])

  const picked = projects.data?.items.find((item) => item.id === params.project)
  const projectLabel =
    params.project === null
      ? null
      : picked
        ? projectName(t, picked)
        : (model.chats.find((chat) => chat.project?.id === params.project)
            ?.project?.label ?? params.project)

  const body = () => {
    if (semantic)
      return (
        <SemanticResults
          request={semantic}
          selected={{ chat: params.chat, at: params.at }}
          f={f}
          zone={zone}
          onMode={(mode) => update({ mode })}
          onRoles={(roles) => update({ roles })}
          onOpen={openPassage}
        />
      )
    if (search)
      return (
        <SearchResults
          request={search}
          selectedId={params.chat}
          f={f}
          zone={zone}
          onMode={(mode) => update({ mode })}
          onRoles={(roles) => update({ roles })}
          onOpen={openHit}
        />
      )
    if (list.isError && !list.data)
      return (
        <ListFailed error={list.error} onRetry={() => void list.refetch()} />
      )
    if (!list.data) return <ListSkeleton />
    if (model.items.length === 0)
      return conditionCount(params) > 0 ? <NoMatch /> : <NoConversations />
    return (
      <RowMenu
        find={(id) => model.chats.find((chat) => chat.id === id)}
        marks={marks}
        onOpen={select}
        onTrashed={(chat) => {
          if (chat.id === params.chat) close()
        }}
      >
        <ChatList
          // Other filters, sort or group: a different list, from its top.
          key={listKey}
          model={model}
          selectedId={params.chat}
          sort={params.sort}
          group={shownGroup}
          showProject={shownGroup !== 'project' && !fixedProject}
          zone={zone}
          f={f}
          hasMore={hasNextPage}
          dimmed={list.isPlaceholderData}
          onSelect={select}
          onPickProject={(project) =>
            update({ project: project.id, group: 'day' }, { push: true })
          }
          onEndReached={more}
        />
      </RowMenu>
    )
  }

  return (
    <div className="flex min-h-0 min-w-0 flex-1">
      <section
        aria-label={t('agentChats.listRegion')}
        className="flex w-[400px] shrink-0 flex-col border-r max-[1240px]:w-[360px]"
      >
        <div {...dragRegion()} className="flex flex-col gap-2 px-3 pt-5 pb-2">
          {!fixedProject && (
            <h1 className="sr-only">{t('shell.nav.history')}</h1>
          )}
          <div className="flex min-h-9 items-center gap-1.5">
            <SearchBox
              value={params.q}
              scoped={fixedProject !== null}
              semantic={params.mode === 'semantic'}
              readerOpen={params.chat !== null}
              onCommit={(q, options) => update({ q }, options)}
            />
            <FilterMenu params={params} update={update} f={f} />
          </div>
          <FilterChips
            params={params}
            update={update}
            projectLabel={projectLabel}
            f={f}
          />
        </div>
        {row?.firstBackup === 'running' && (
          <ListNote>{t('agentChats.states.firstBackup')}</ListNote>
        )}
        {params.flags.includes('trash') && (
          <ListNote>{t('agentChats.states.trash')}</ListNote>
        )}
        {body()}
      </section>
      {params.chat ? (
        <Reader
          key={params.chat}
          id={params.chat}
          beside
          find={params.find}
          at={params.at}
          onFind={(find) => update({ find, at: null })}
          onClose={close}
          onTrashed={close}
          onOpenProject={(project) => void navigate(agentProjectPath(project))}
        />
      ) : (
        <EmptyReader scoped={fixedProject !== null} />
      )}
    </div>
  )
}
