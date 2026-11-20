/**
 * Queries for the conversation list and the reader: list pages, counts,
 * one conversation's header, info panel,
 * subagents and prompt versions; the marks a person puts on a
 * conversation; and keeping all of them current.
 *
 * Responsibilities: one cached copy of each answer, read only while the
 * kind is visible and its store opens; paging through the backend's
 * cursors; after a mark is set, the header everywhere at once and the
 * list and counts again; after the backend's change event, everything
 * again, except the change this window just made and already applied.
 * The lock clears every answer with the rest of the cache.
 * Not responsible for: the reader's windows of rows
 * (`features/agent-reader/use-windows.ts`) or any copy.
 */
import {
  keepPreviousData,
  useInfiniteQuery,
  useQuery,
  type InfiniteData,
} from '@tanstack/react-query'
import { useCallback, useEffect } from 'react'
import {
  agentChatsClient,
  type ChatEditRequest,
  type ChatHeader,
  type ChatGroup,
  type ChatListPage,
  type ChatListQuery,
} from '@/lib/backend-client/agent-chats'
import { subscribeToAgentConversations } from '@/lib/ipc/agent-chats-events'
import { queryClient, queryKeys } from '@/lib/query'
import { useAgentKind } from './agent-backup'

/** Rows per list page; the backend's default. */
const LIST_PAGE = 50
/** Subagent cards per page, wherever a conversation's subagents are read. */
export const CHILDREN_PAGE = 50

/**
 * A page of the list and the grouping it was asked in. A day page and a
 * project page are shaped differently, and while another grouping loads the
 * old pages stay on screen: they are drawn by their own grouping.
 */
export interface ShownChatListPage extends ChatListPage {
  group: ChatGroup
}

/**
 * The list's filters, sort and group, without a cursor: one cache entry
 * each. Not read while a search shows instead (`enabled` false).
 */
export function useChatList(
  query: Omit<ChatListQuery, 'cursor' | 'limit'>,
  enabled = true,
) {
  const { readable } = useAgentKind()
  return useInfiniteQuery({
    queryKey: queryKeys.agentChatList(query),
    queryFn: async ({ pageParam }): Promise<ShownChatListPage> => ({
      ...(await agentChatsClient.list({
        ...query,
        limit: LIST_PAGE,
        ...(pageParam ? { cursor: pageParam } : {}),
      })),
      // By day when not given, as the backend does.
      group: query.group ?? 'day',
    }),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (page) => page.nextCursor ?? undefined,
    enabled: readable && enabled && query.timeZone !== undefined,
    // Changing the filters keeps the old rows on screen, dimmed, until the
    // new ones arrive, instead of flashing an empty list.
    placeholderData: keepPreviousData,
    staleTime: Infinity,
  })
}

export function useChatCounts(query: Omit<ChatListQuery, 'cursor' | 'limit'>) {
  const { readable } = useAgentKind()
  return useQuery({
    queryKey: queryKeys.agentChatCounts(query),
    queryFn: () => agentChatsClient.counts(query),
    enabled: readable && query.timeZone !== undefined,
    placeholderData: keepPreviousData,
    staleTime: Infinity,
  })
}

export function useChatHeader(id: string | null) {
  const { readable } = useAgentKind()
  return useQuery({
    queryKey: queryKeys.agentChat(id ?? ''),
    queryFn: () => agentChatsClient.header(id!),
    enabled: readable && id !== null,
    staleTime: Infinity,
  })
}

export function useChatInfo(id: string, enabled: boolean) {
  const { readable } = useAgentKind()
  return useQuery({
    queryKey: queryKeys.agentChatInfo(id),
    queryFn: () => agentChatsClient.info(id),
    enabled: readable && enabled,
    staleTime: Infinity,
  })
}

/** A conversation's subagents (cards), every page of them. */
export function useChatChildren(id: string, publication: string | undefined) {
  const { readable } = useAgentKind()
  const query = useInfiniteQuery({
    queryKey: queryKeys.agentChatChildren(id, publication ?? ''),
    queryFn: ({ pageParam }) =>
      agentChatsClient.branches({
        id,
        publication,
        limit: CHILDREN_PAGE,
        ...(pageParam ? { childrenAfter: pageParam } : {}),
      }),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (page) => page.childrenAfter ?? undefined,
    enabled: readable && publication !== undefined,
    staleTime: Infinity,
  })
  const { hasNextPage, isFetchingNextPage, fetchNextPage } = query
  useEffect(() => {
    if (hasNextPage && !isFetchingNextPage) void fetchNextPage()
  }, [hasNextPage, isFetchingNextPage, fetchNextPage])
  return query
}

/** The versions of one prompt (asked again after rewriting it), first page. */
export function useChatVersions(
  id: string,
  publication: string,
  sourceId: string,
  ordinal: number,
  enabled: boolean,
) {
  return useQuery({
    queryKey: queryKeys.agentChatVersions(id, publication, ordinal),
    queryFn: () =>
      agentChatsClient.branches({
        id,
        publication,
        sourceId,
        promptOrdinal: ordinal,
        limit: 50,
      }),
    enabled,
    staleTime: Infinity,
  })
}

/**
 * The view token of the last mark this window set. The backend's change
 * event for it carries the same token; that one needs no refetch.
 */
let ownViewToken: string | null = null

function patchListRows(header: ChatHeader) {
  queryClient.setQueriesData<InfiniteData<ShownChatListPage>>(
    { queryKey: [...queryKeys.agentChats, 'list'] },
    (data) =>
      data?.pages
        ? {
            ...data,
            pages: data.pages.map((page) => ({
              ...page,
              items: page.items.map((item) =>
                item.id === header.summary.id
                  ? { ...header.summary, day: item.day }
                  : item,
              ),
            })),
          }
        : data,
  )
}

/**
 * The list, counts and projects again, and the switcher's line: a mark can
 * move a row in or out, and the trash leaves every count.
 */
function refetchLists() {
  for (const part of ['list', 'counts'])
    void queryClient.invalidateQueries({
      queryKey: [...queryKeys.agentChats, part],
    })
  void queryClient.invalidateQueries({
    queryKey: queryKeys.agentOverview('space'),
  })
}

/**
 * Sets a mark (star, put away, trash, put back, rename). The header comes
 * back committed: it replaces the cached one, the list's row shows it at
 * once, and the list and counts are read again, since the row may now
 * belong elsewhere or nowhere.
 */
export function useEditChat() {
  return useCallback(async (request: ChatEditRequest) => {
    const header = await agentChatsClient.edit(request)
    ownViewToken = header.viewToken
    queryClient.setQueryData(queryKeys.agentChat(request.id), header)
    patchListRows(header)
    refetchLists()
    void queryClient.invalidateQueries({
      queryKey: queryKeys.agentChatInfo(request.id),
    })
    return header
  }, [])
}

/**
 * Keeps the list and the reader current while mounted: the backend says a
 * conversation's marks changed, or a backup published conversations. Rows
 * are keyed by conversation, so a list read again keeps its scroll.
 */
export function useAgentChatsSync() {
  useEffect(() => {
    let stopped = false
    let stop: (() => void) | undefined
    void subscribeToAgentConversations((event) => {
      if (event.viewToken === ownViewToken) return
      refetchLists()
      // Headers and info panels: a backup can publish any conversation
      // again under a new publication. Windows, children and versions are
      // keyed by their publication, which never changes its content.
      void queryClient.invalidateQueries({
        predicate: ({ queryKey }) =>
          queryKey[1] === queryKeys.agentChats[1] &&
          queryKey[2] === 'chat' &&
          (event.id === null || queryKey[3] === event.id) &&
          (queryKey.length === 4 || queryKey[4] === 'info'),
      })
    }).then((unsubscribe) => {
      if (stopped) unsubscribe()
      else stop = unsubscribe
    })
    return () => {
      stopped = true
      stop?.()
    }
  }, [])
}
