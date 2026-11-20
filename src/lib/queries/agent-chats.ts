/**
 * Queries for the conversation list and the reader: list pages, counts,
 * the projects the filter offers, one conversation's header, info panel,
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
  type ChatListPage,
  type ChatListQuery,
  type ChatProjectGroup,
} from '@/lib/backend-client/agent-chats'
import { subscribeToAgentConversations } from '@/lib/ipc/agent-chats-events'
import { queryClient, queryKeys } from '@/lib/query'
import { useAgentKind } from './agent-backup'

/** Rows per list page; the backend's default. */
const LIST_PAGE = 50
/** Project groups per page of the filter's project list. */
const PROJECTS_PAGE = 100
const CHILDREN_PAGE = 50

/** The list's filters, sort and group, without a cursor: one cache entry each. */
export function useChatList(query: Omit<ChatListQuery, 'cursor' | 'limit'>) {
  const { readable } = useAgentKind()
  return useInfiniteQuery({
    queryKey: queryKeys.agentChatList(query),
    queryFn: ({ pageParam }) =>
      agentChatsClient.list({
        ...query,
        limit: LIST_PAGE,
        ...(pageParam ? { cursor: pageParam } : {}),
      }),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (page) => page.nextCursor ?? undefined,
    enabled: readable && query.timeZone !== undefined,
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

/** A project as the filter lists it: the newest activity in it first. */
export interface ChatProjectOption {
  id: string
  label: string
  /** The newest last activity among its conversations. */
  lastActivityAt: number | null
  count: number
}

/**
 * Every working folder that has a conversation, newest first, read a page
 * of groups at a time while `enabled` (the filter's menu is open) until the
 * end. Folders the tool did not record cannot be picked, so they are left
 * out.
 */
export function useChatProjects(
  timeZone: string | undefined,
  enabled: boolean,
) {
  const { readable } = useAgentKind()
  const query = useInfiniteQuery({
    queryKey: queryKeys.agentChatProjects,
    queryFn: ({ pageParam }) =>
      agentChatsClient.list({
        group: 'project',
        sort: 'lastActivity',
        timeZone,
        limit: PROJECTS_PAGE,
        ...(pageParam ? { cursor: pageParam } : {}),
      }),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (page) => page.nextCursor ?? undefined,
    enabled: readable && enabled && timeZone !== undefined,
    staleTime: Infinity,
  })
  const { hasNextPage, isFetchingNextPage, fetchNextPage } = query
  useEffect(() => {
    if (enabled && hasNextPage && !isFetchingNextPage) void fetchNextPage()
  }, [enabled, hasNextPage, isFetchingNextPage, fetchNextPage])
  const options: ChatProjectOption[] = []
  for (const page of query.data?.pages ?? []) {
    for (const group of page.groups as ChatProjectGroup[]) {
      if (!group.project) continue
      options.push({
        id: group.project.id,
        label: group.project.label,
        lastActivityAt: group.items[0]?.lastActivityAt ?? null,
        count: group.count,
      })
    }
  }
  return { ...query, options, complete: query.isSuccess && !hasNextPage }
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
  queryClient.setQueriesData<InfiniteData<ChatListPage>>(
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
  for (const part of ['list', 'counts', 'projects'])
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
