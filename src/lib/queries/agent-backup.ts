/**
 * Queries for backing up Agent conversations: the overview, the sources,
 * the run ledger and one run, and the conversation backups as background
 * tasks; and keeping them current from the backend's events.
 *
 * Responsibilities: one cached copy of each answer, read only while the
 * kind is visible and its store opens (a person who only keeps browsing
 * history never makes these reads); paging through the backend's cursors;
 * patching a running task with each progress event (one shape, however the
 * event nests it); refetching after an event or a reconnect. The lock clears
 * every answer with the rest of the cache.
 * Not responsible for: any copy, or what a finished backup tells the person
 * (the caller of `useAgentBackupSync` decides).
 */
import {
  useInfiniteQuery,
  useQuery,
  type InfiniteData,
} from '@tanstack/react-query'
import { useEffect, useRef } from 'react'
import {
  agentBackupClient,
  type ArchiveRunDetail,
  type BackgroundTaskChangedEvent,
  type BackupProgress,
  type CursorPage,
  type KindBackupFinishedEvent,
  type KindTask,
  type TaskGroup,
  type TaskSummary,
  type ToolProgress,
} from '@/lib/backend-client/agent-backup'
import type { DataKindRow } from '@/lib/backend-client/data-kinds'
import { agentNeedsReview, isViewOnly } from '@/lib/backend-client/data-kinds'
import { subscribeToAgentBackup } from '@/lib/ipc/agent-backup-events'
import { patchQueries, queryClient, queryKeys } from '@/lib/query'
import { useDataKinds } from './data-kinds'

/** Rows of a source list page; there are a handful of tools and folders. */
const SOURCES_PAGE = 100
const RUNS_PAGE = 20
const TASKS_PAGE = 20

/** Agent conversations as the backend last described them, and what that allows. */
export function useAgentKind() {
  const kinds = useDataKinds()
  const row: DataKindRow | undefined = kinds.data?.kinds.find(
    (item) => item.kind === 'agent-history',
  )
  const visible = row !== undefined && row.state !== 'off'
  return {
    kinds,
    row,
    policyToken: kinds.data?.policyToken,
    /** On or paused, and its store opens: the backup page and tasks can be read. */
    readable: visible && row.availability === 'available',
    viewOnly: isViewOnly(row),
    /** On, but an import left no approved folder: they are chosen again. */
    needsReview: agentNeedsReview(row),
  }
}

export function useAgentOverview(purpose: 'backup' | 'space', enabled = true) {
  const { readable } = useAgentKind()
  return useQuery({
    queryKey: queryKeys.agentOverview(purpose),
    queryFn: () => agentBackupClient.overview(purpose),
    enabled: readable && enabled,
    // Events and a finished backup refresh it.
    staleTime: Infinity,
  })
}

export function useAgentSources() {
  const { readable } = useAgentKind()
  return useInfiniteQuery({
    queryKey: queryKeys.agentSources,
    queryFn: ({ pageParam }) =>
      agentBackupClient.sources(SOURCES_PAGE, pageParam),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (page) => page.nextCursor,
    enabled: readable,
    staleTime: Infinity,
  })
}

/** This space's runs, newest first; a page at a time, never the whole ledger. */
export function useAgentRuns(enabled = true) {
  const { readable, policyToken } = useAgentKind()
  return useInfiniteQuery({
    queryKey: queryKeys.agentRuns(policyToken ?? ''),
    queryFn: ({ pageParam }) => agentBackupClient.runs(RUNS_PAGE, pageParam),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (page) => page.nextCursor,
    enabled: readable && enabled && policyToken !== undefined,
    staleTime: Infinity,
  })
}

/**
 * One run, `null` for none. Its warnings and folders come a page at a time;
 * `fetchNextPage` reads the next.
 */
export function useArchiveRun(runRef: string | null, enabled = true) {
  return useInfiniteQuery({
    queryKey: queryKeys.archiveRun(runRef ?? ''),
    queryFn: ({ pageParam }) => agentBackupClient.run(runRef!, pageParam),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (page) => page.nextCursor,
    enabled: runRef !== null && enabled,
    staleTime: Infinity,
  })
}

/** The first page's header with every page's warnings, notes and folders after it. */
export function joinRunPages(
  data: InfiniteData<ArchiveRunDetail> | undefined,
): ArchiveRunDetail | undefined {
  const [first, ...rest] = data?.pages ?? []
  if (!first) return undefined
  return {
    ...first,
    warnings: [first, ...rest].flatMap((page) => page.warnings ?? []),
    notes: [first, ...rest].flatMap((page) => page.notes ?? []),
    roots: [first, ...rest].flatMap((page) => page.roots ?? []),
  }
}

/**
 * What runs, waits and needs a look, every kind counted, for the rail and
 * the first-save screens. Read once the kinds are known (the archive is
 * open); it covers browsing and whole-archive work even with Agent
 * conversations off. Their task events (`useAgentBackupSync`) and the
 * ledger's (`useBackgroundTasksSync`) keep it current.
 */
export function useTaskSummary() {
  const { kinds } = useAgentKind()
  return useQuery({
    queryKey: queryKeys.taskSummary,
    queryFn: agentBackupClient.taskSummary,
    enabled: kinds.data !== undefined,
    staleTime: Infinity,
  })
}

export function useKindTasks(group: TaskGroup) {
  const { readable } = useAgentKind()
  return useInfiniteQuery({
    queryKey: queryKeys.taskGroup(group),
    queryFn: ({ pageParam }) =>
      agentBackupClient.tasks(group, TASKS_PAGE, pageParam),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (page) => page.nextCursor,
    enabled: readable,
    staleTime: Infinity,
  })
}

/** A count or size from the wire as a number; they stay far below 2^53. */
export function wireNumber(value: string | null | undefined) {
  const number = Number(value ?? 0)
  return Number.isFinite(number) ? number : 0
}

/** A task still at work: it can show progress. */
export function isActive(task: Pick<KindTask, 'state'>) {
  return (
    task.state === 'running' ||
    task.state === 'cancelling' ||
    task.state === 'queued'
  )
}

type Live = Partial<KindTask> & { taskId: string }

/**
 * A live event as a task's fields. The task-changed event nests `perTool`
 * inside `progress`, the progress event sends it beside; a task row has it
 * beside.
 */
function liveFields(
  taskId: string,
  progress: (BackupProgress & { perTool?: ToolProgress[] }) | undefined,
  perTool: ToolProgress[] | undefined,
): Live {
  const live: Live = { taskId }
  if (progress) {
    const { perTool: nested, ...rest } = progress
    live.progress = {
      phase: rest.phase,
      unit: rest.unit,
      done: rest.done,
      total: rest.total,
    }
    const tools = perTool ?? nested
    if (tools) live.perTool = tools
  } else if (perTool) {
    live.perTool = perTool
  }
  return live
}

/** Applies `live` to the running task it names; false when no cached summary has it. */
function patchSummary(live: Live) {
  let found = false
  queryClient.setQueryData<TaskSummary>(queryKeys.taskSummary, (summary) => {
    if (!summary) return summary
    const running = summary.running.map((task) => {
      if (task.taskId !== live.taskId) return task
      found = true
      return { ...task, ...live }
    })
    return found ? { ...summary, running } : summary
  })
  return found
}

function patchGroups(live: Live) {
  patchQueries<InfiniteData<CursorPage<KindTask>>>(
    queryKeys.kindTasks,
    (data) => {
      if (!data.pages) return undefined
      return {
        ...data,
        pages: data.pages.map((page) => ({
          ...page,
          items: page.items.map((task) =>
            task.taskId === live.taskId ? { ...task, ...live } : task,
          ),
        })),
      }
    },
  )
}

/** The cached task with this id, wherever it is cached: lists or a run's details. */
export function cachedTask(taskId: string): KindTask | undefined {
  const summary = queryClient.getQueryData<TaskSummary>(queryKeys.taskSummary)
  const running = summary?.running.find((task) => task.taskId === taskId)
  if (running) return running
  for (const [, data] of queryClient.getQueriesData<
    InfiniteData<CursorPage<KindTask>>
  >({ queryKey: queryKeys.kindTasks })) {
    for (const page of data?.pages ?? []) {
      const task = page.items?.find((item) => item.taskId === taskId)
      if (task) return task
    }
  }
  for (const [, data] of queryClient.getQueriesData<
    InfiniteData<ArchiveRunDetail>
  >({ queryKey: queryKeys.archiveRuns })) {
    const task = data?.pages?.[0]?.task
    if (task?.taskId === taskId) return task
  }
  return undefined
}

/**
 * The task an action answered with, in every cached copy at once, so the
 * next action sends its new token even before the lists are read again.
 * The whole task replaces the old one: a dismissed notice is gone from it.
 */
export function rememberTask(task: KindTask) {
  queryClient.setQueryData<TaskSummary>(queryKeys.taskSummary, (summary) =>
    summary
      ? {
          ...summary,
          running: summary.running.map((item) =>
            item.taskId === task.taskId ? task : item,
          ),
        }
      : summary,
  )
  patchQueries<InfiniteData<CursorPage<KindTask>>>(
    queryKeys.kindTasks,
    (data) =>
      data.pages
        ? {
            ...data,
            pages: data.pages.map((page) => ({
              ...page,
              items: page.items.map((item) =>
                item.taskId === task.taskId ? task : item,
              ),
            })),
          }
        : undefined,
  )
  queryClient.setQueryData<InfiniteData<ArchiveRunDetail>>(
    queryKeys.archiveRun(task.runRef),
    (data) =>
      data?.pages?.[0]?.task
        ? {
            ...data,
            pages: data.pages.map((page, index) =>
              index === 0 ? { ...page, task } : page,
            ),
          }
        : data,
  )
}

function refetchTasks() {
  void queryClient.invalidateQueries({ queryKey: queryKeys.kindTasks })
}

/**
 * The same, unless a read of them is already on its way: progress arrives
 * ten times a second, and a refetch per event would never finish.
 */
function refetchTasksOnce() {
  if (queryClient.isFetching({ queryKey: queryKeys.kindTasks }) > 0) return
  refetchTasks()
}

/** Everything a finished or changed backup can change. */
export function refetchAfterBackup(runRef?: string) {
  void queryClient.invalidateQueries({ queryKey: queryKeys.agentBackup })
  void queryClient.invalidateQueries({ queryKey: queryKeys.dataKinds })
  if (runRef)
    void queryClient.invalidateQueries({
      queryKey: queryKeys.archiveRun(runRef),
    })
  refetchTasks()
}

function onTaskChanged(event: BackgroundTaskChangedEvent) {
  const live: Live = {
    ...liveFields(event.taskId, event.progress, undefined),
    state: event.state,
    phase: event.phase,
    actions: event.actions,
    attention: event.attention,
    snapshotToken: event.snapshotToken,
    // Absent once dismissed: the cached copy must lose it too.
    formatNotices: event.formatNotices,
  }
  const known = patchSummary(live)
  patchGroups(live)
  const active = isActive({ state: event.state })
  if (!known && active) refetchTasksOnce()
  else if (!active) {
    // Ended: the lists and counts move.
    refetchTasks()
    const runRef = cachedTask(event.taskId)?.runRef
    if (runRef)
      void queryClient.invalidateQueries({
        queryKey: queryKeys.archiveRun(runRef),
      })
  }
}

/**
 * Keeps every conversation-backup answer current: patches the running task
 * with each progress event, refetches what a task change, a finished backup
 * or a catalogue change touches, and refetches everything once listening
 * (an event sent before nobody heard it). Mounted once, by the frame,
 * which unmounts on lock.
 */
export function useAgentBackupSync(
  onFinished: (event: KindBackupFinishedEvent) => void,
) {
  const finished = useRef(onFinished)
  useEffect(() => {
    finished.current = onFinished
  })
  useEffect(() => {
    let stopped = false
    let stop: (() => void) | undefined
    void subscribeToAgentBackup({
      progress: (event) => {
        const live = liveFields(event.taskId, event, event.perTool)
        if (!patchSummary(live)) refetchTasksOnce()
        patchGroups(live)
      },
      taskChanged: onTaskChanged,
      finished: (event) => {
        refetchAfterBackup(event.runRef)
        finished.current(event)
      },
      catalogChanged: () =>
        void queryClient.invalidateQueries({
          queryKey: queryKeys.agentBackup,
        }),
      settingsChanged: () => {
        void queryClient.invalidateQueries({
          queryKey: queryKeys.agentSettings,
        })
        void queryClient.invalidateQueries({
          queryKey: queryKeys.agentSources,
        })
      },
    }).then((unsubscribe) => {
      if (stopped) {
        unsubscribe()
        return
      }
      stop = unsubscribe
      // What changed before this listener started sent its events to nobody.
      void queryClient.invalidateQueries({ queryKey: queryKeys.agentBackup })
      refetchTasks()
    })
    return () => {
      stopped = true
      stop?.()
    }
  }, [])
}
