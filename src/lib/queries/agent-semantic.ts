/**
 * The Agent conversations' semantic index as Settings, the tasks page, the
 * rail and Records read it: one cached status, kept current by the
 * backend's change event, and the controls that act on its build.
 *
 * Responsibilities: reading the status while Agent conversations are on or
 * paused, again each time a screen that shows it opens (another window may
 * have changed the AI settings; the read is one small file); and, once for
 * the whole window (`useAgentSemanticSync`), putting each event's job into
 * the cached status and into the task lists' copy of the build at once
 * (progress arrives several times a second), reading the status again when
 * a build ends (only then does the serving generation change) and the task
 * lists when the build moves between their groups, and reading the status
 * again when the AI access switches change or this window saves the AI
 * settings that decide it (AI on, semantic search on, which embedding
 * model). Not responsible for: any wording, or searching.
 */
import { useMutation, useQuery, type InfiniteData } from '@tanstack/react-query'
import { useEffect, useRef } from 'react'
import type { TaskPage, TaskSummary } from '@/lib/backend-client/agent-backup'
import {
  AGENT_SEMANTIC_INDEX_CHANGED_EVENT,
  agentSemanticClient,
  type SemanticAction,
  type SemanticIndexStatus,
  type SemanticJob,
  type SemanticTask,
} from '@/lib/backend-client/agent-semantic'
import { AGENT_VIEWER_PREFERENCES_CHANGED_EVENT } from '@/lib/backend-client/agent-viewer'
import { listenDesktopEvent } from '@/lib/ipc/events'
import { patchQueries, queryClient, queryKeys } from '@/lib/query'
import { useAgentKind } from './agent-backup'
import { useSnapshot } from './app'

/** Under `archive`, so a lock clears it with every other answer. */
export const semanticIndexKey = ['archive', 'agent-semantic', 'status'] as const

const ACTIVE = new Set<SemanticJob['state']>([
  'running',
  'paused',
  'cancelling',
])

/** A build is going (or stopping): progress events will follow. */
export function isActiveJob(job: SemanticJob | null | undefined) {
  return job != null && ACTIVE.has(job.state)
}

function refetch() {
  void queryClient.invalidateQueries({ queryKey: semanticIndexKey })
}

/** The status read again now, e.g. once the switch's save is through. */
export const refetchSemanticIndex = refetch

/**
 * The task lists read again. A read already on its way may carry the
 * build's old state, so a change of state replaces it (`always`); a build
 * not listed yet only needs one read, not one per progress event.
 */
function refetchTasks(always: boolean) {
  if (!always && queryClient.isFetching({ queryKey: queryKeys.kindTasks }) > 0)
    return
  void queryClient.invalidateQueries({ queryKey: queryKeys.kindTasks })
}

/**
 * The lists' copy of the build, with this event's progress. A build that
 * changed state may belong to another group now (paused waits, running
 * runs) or to none (it ended), and one not listed yet has just started:
 * each reads the lists again.
 */
function patchTasks(job: SemanticJob) {
  if (!isActiveJob(job)) return refetchTasks(true)
  let listed = false
  let moved = false
  const patch = (task: SemanticTask | null): SemanticTask | null => {
    if (task?.jobId !== job.jobId) return task
    listed = true
    if (task.state !== job.state) moved = true
    return {
      ...task,
      state: job.state as SemanticTask['state'],
      progress: { ...task.progress, done: String(job.passages) },
    }
  }
  queryClient.setQueryData<TaskSummary>(queryKeys.taskSummary, (summary) =>
    summary ? { ...summary, semantic: patch(summary.semantic) } : summary,
  )
  patchQueries<InfiniteData<TaskPage>>(queryKeys.kindTasks, (data) =>
    data.pages
      ? {
          ...data,
          pages: data.pages.map((page) => ({
            ...page,
            semantic: patch(page.semantic ?? null),
          })),
        }
      : undefined,
  )
  if (moved) refetchTasks(true)
  else if (!listed) refetchTasks(false)
}

function apply(job: SemanticJob) {
  queryClient.setQueryData<SemanticIndexStatus>(semanticIndexKey, (status) =>
    status ? { ...status, job } : status,
  )
  patchTasks(job)
  if (!isActiveJob(job)) refetch()
}

export function useAgentSemanticIndex({ enabled = true } = {}) {
  const { row } = useAgentKind()
  return useQuery({
    queryKey: semanticIndexKey,
    queryFn: agentSemanticClient.status,
    enabled: enabled && row !== undefined && row.state !== 'off',
    staleTime: Infinity,
    refetchOnMount: 'always',
  })
}

/** Mounted once, in the app frame: the build's events and what decides the index. */
export function useAgentSemanticSync() {
  const ai = useSnapshot()?.config.ai
  const decides = ai
    ? `${ai.enabled}:${ai.semanticIndexEnabled}:${ai.embeddingProviderId ?? ''}`
    : null
  const seen = useRef(decides)
  useEffect(() => {
    if (seen.current === decides) return
    seen.current = decides
    refetch()
  }, [decides])
  useEffect(() => {
    const stops: (() => void)[] = []
    let gone = false
    const keep = (unlisten: () => void) => {
      if (gone) unlisten()
      else stops.push(unlisten)
    }
    void listenDesktopEvent<SemanticJob>(
      AGENT_SEMANTIC_INDEX_CHANGED_EVENT,
      apply,
    ).then(keep)
    // The switch, or another window's, decides whether there is an index at all.
    void listenDesktopEvent(
      AGENT_VIEWER_PREFERENCES_CHANGED_EVENT,
      refetch,
    ).then(keep)
    return () => {
      gone = true
      for (const stop of stops) stop()
    }
  }, [])
}

/**
 * Builds, pauses, resumes or cancels. The answer goes into the cached
 * status at once; a refusal rejects, for the caller to say so.
 */
export function useSemanticIndexControl() {
  return useMutation({
    mutationFn: ({
      action,
      jobId,
    }: {
      action: SemanticAction
      jobId?: string
    }) => agentSemanticClient.control(action, jobId),
    onSuccess: (answer) => {
      if ('jobId' in answer) apply(answer)
      else refetch()
    },
    onError: refetch,
  })
}
