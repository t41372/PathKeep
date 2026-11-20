/**
 * One group of the background tasks page, with every kind's rows in it:
 * conversation backups (kept current by the backend's task events, as the
 * frame already does), the conversations' semantic index build (listed on
 * the backups' first page; its events keep it current, see
 * `useAgentSemanticSync`) and the run ledger's rows: browsing history's
 * work and work on the whole archive (read again on the ledger's event;
 * see `useBrowsingTasks`).
 *
 * Responsibilities: reading both, a page at a time; merging them newest
 * first (`mergeNewestFirst`); reading on from whichever kind holds the next
 * rows; and the group's state: still loading, failed, or the rows. A kind
 * whose store will not open, or that is off, adds nothing and does not
 * hold the others back.
 * Not responsible for: which rows a group shows (the backend's groups;
 * "Done today" leaves out what is under "Needs a look", decided by the
 * caller) or drawing.
 */
import { useMemo } from 'react'
import type { TaskGroup } from '@/lib/backend-client/agent-backup'
import { useAgentKind, useKindTasks } from '@/lib/queries/agent-backup'
import { useBrowsingTasks } from '@/lib/queries/background-tasks'
import {
  agentEntry,
  browsingEntry,
  mergeNewestFirst,
  semanticEntry,
  type TaskEntry,
} from './task-entry'

export interface TaskGroupRows {
  entries: TaskEntry[]
  /** No answer yet from a kind that will answer. */
  loading: boolean
  /** A kind's read failed and nothing of it is shown. */
  error: unknown
  retry: () => void
  hasMore: boolean
  loadingMore: boolean
  loadMore: () => void
}

export function useTaskGroup(group: TaskGroup) {
  const agentKind = useAgentKind()
  const agent = useKindTasks(group)
  const browsing = useBrowsingTasks(group)
  const agentShown = agentKind.readable

  const merged = useMemo(() => {
    const sources = [
      {
        entries: agentShown
          ? (agent.data?.pages ?? []).flatMap((page) =>
              page.items.map((task) => agentEntry(task, group)),
            )
          : [],
        hasMore: agentShown && agent.hasNextPage,
      },
      {
        entries: (browsing.data?.pages ?? []).flatMap((page) =>
          page.items.map(browsingEntry),
        ),
        hasMore: browsing.hasNextPage,
      },
    ]
    const merged = mergeNewestFirst(sources)
    const semantic = agentShown
      ? (agent.data?.pages ?? []).flatMap((page) =>
          page.semantic ? [semanticEntry(page.semantic)] : [],
        )
      : []
    return { ...merged, entries: [...semantic, ...merged.entries] }
  }, [
    agent.data,
    agent.hasNextPage,
    agentShown,
    browsing.data,
    browsing.hasNextPage,
    group,
  ])

  // A disabled query stays pending without fetching: it is not loading.
  const waiting = (query: { isPending: boolean; fetchStatus: string }) =>
    query.isPending && query.fetchStatus !== 'idle'
  const agentError = agentShown && agent.isError && !agent.data
  const browsingError = browsing.isError && !browsing.data

  return {
    entries: merged.entries,
    loading: waiting(browsing) || (agentShown && waiting(agent)),
    error: agentError ? agent.error : browsingError ? browsing.error : null,
    retry: () => {
      if (agentError) void agent.refetch()
      if (browsingError) void browsing.refetch()
    },
    hasMore: merged.next !== null,
    loadingMore: agent.isFetchingNextPage || browsing.isFetchingNextPage,
    loadMore: () => {
      if (merged.next === 0) void agent.fetchNextPage()
      else if (merged.next === 1) void browsing.fetchNextPage()
    },
  } satisfies TaskGroupRows
}
