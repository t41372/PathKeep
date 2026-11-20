/**
 * History of the Agent space (`#/agent/chats`, `agent-history.md` §3): the
 * conversation list on the left, the reader of the selected one on the
 * right. Filters, sort, group and the open conversation live in the URL.
 *
 * Responsible for: wiring the URL, the list query, the marks and the
 * reader together. Not responsible for: rows, menus or the reader itself
 * (the sibling files and `features/agent-reader`).
 */
import { MessagesSquare } from 'lucide-react'
import { useCallback, useMemo } from 'react'
import { dragRegion } from '@/components/app/drag-region'
import { Kbd } from '@/components/ui/kbd'
import { Reader } from '@/features/agent-reader/reader'
import { useInsightsFormat, useInsightsZone } from '@/features/insights/zone'
import type { ChatSummary } from '@/lib/backend-client/agent-chats'
import { useI18n } from '@/lib/i18n'
import { useAgentKind } from '@/lib/queries/agent-backup'
import {
  useAgentChatsSync,
  useChatCounts,
  useChatList,
} from '@/lib/queries/agent-chats'
import { shortcutLabel } from '@/lib/runtime'
import { useChatMarks } from './chat-marks'
import { ChatList } from './chat-list'
import { listQuery, conditionCount, useChatsParams } from './chats-params'
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

function EmptyReader() {
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
            <Kbd>{shortcutLabel('K')}</Kbd>{' '}
            {t('agentChats.emptyReader.palette')}
          </span>
        </div>
      </div>
    </div>
  )
}

export default function ChatsPage() {
  const { t } = useI18n()
  const [params, update] = useChatsParams()
  const zone = useInsightsZone().shown
  const f = useInsightsFormat()
  const { row } = useAgentKind()
  const query = useMemo(() => listQuery(params, zone), [params, zone])
  const listKey = useMemo(() => JSON.stringify(query), [query])
  const list = useChatList(query)
  const everything = useChatCounts(useMemo(() => ({ timeZone: zone }), [zone]))
  const marks = useChatMarks()
  useAgentChatsSync()

  const pages = list.data?.pages
  const model = useMemo(
    () => buildList(pages ?? [], params.group),
    [pages, params.group],
  )
  const select = useCallback(
    (chat: ChatSummary) => update({ chat: chat.id }),
    [update],
  )
  const close = useCallback(() => update({ chat: null }), [update])
  const { hasNextPage, isFetchingNextPage, fetchNextPage } = list
  const more = useCallback(() => {
    if (hasNextPage && !isFetchingNextPage) void fetchNextPage()
  }, [hasNextPage, isFetchingNextPage, fetchNextPage])

  const projectLabel =
    params.project === null
      ? null
      : (model.chats.find((chat) => chat.project?.id === params.project)
          ?.project?.label ??
        params.project.split(/[\\/]/).filter(Boolean).pop() ??
        params.project)

  const body = () => {
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
          group={params.group}
          showProject={params.group !== 'project'}
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
          <div className="flex min-h-9 items-center gap-1.5">
            {/* The search box takes this place once conversations can be searched. */}
            <h1 className="min-w-0 flex-1 truncate px-2 text-[15px] font-semibold tracking-[-0.01em]">
              {t('shell.nav.history')}
            </h1>
            <FilterMenu
              params={params}
              update={update}
              timeZone={zone}
              folderCount={everything.data?.folderCount}
              f={f}
            />
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
          onClose={close}
          onTrashed={close}
          onOpenProject={(project) =>
            update({ project, chat: null }, { push: true })
          }
        />
      ) : (
        <EmptyReader />
      )}
    </div>
  )
}
