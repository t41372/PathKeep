/**
 * The two background queues as Settings → Background work reads them: the
 * Insights queue (`load_intelligence_runtime`: rebuilds, title tidying, page
 * summaries) and the search-index / assistant queue (`load_ai_queue_status`).
 *
 * Responsibilities: the status queries and each queue's full job history, a
 * page at a time (`list_intelligence_jobs` / `list_ai_jobs`), read again every
 * few seconds while the page is open, so work that anything else started (a
 * backup, Settings → AI) shows up as it starts; and the per-job actions the
 * backend allows. Both status reads also restart the backend's drain workers,
 * so refetching after "resume" is what gets work moving again.
 *
 * Not responsible for rendering or copy.
 */
import {
  type InfiniteData,
  useInfiniteQuery,
  useMutation,
  useQuery,
  useQueryClient,
} from '@tanstack/react-query'
import { useCallback, useEffect, useMemo, useRef } from 'react'
import { toast } from 'sonner'
import { intelligenceClient } from '@/lib/backend-client/intelligence'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { queryKeys } from '@/lib/query'
import type {
  AiQueueStatus,
  IntelligenceRuntimeSnapshot,
  ListJobsRequest,
} from '@/lib/types'
import { AI_QUEUE_POLL_MS } from '@/features/ai-setup/use-semantic-index'
import type { JobView } from './job-view'

/** Shared with `use-semantic-index`, so the two never poll the same queue twice. */
export const aiQueueKey = ['ai', 'queue'] as const
export const runtimeKey = ['intelligence', 'runtime'] as const

export type QueueName = 'insights' | 'ai'
const jobsKey = (queue: QueueName) => ['background', 'jobs', queue] as const

const POLL_MS = 2000
const JOB_PAGE = 50

/** Whether the Insights queue has work running, or waiting while not paused. */
export function runtimeActive(
  data: IntelligenceRuntimeSnapshot | undefined,
  paused: boolean,
) {
  if (!data) return false
  return data.queue.running > 0 || (!paused && data.queue.queued > 0)
}

/** Whether the search-index queue has work running, or waiting while not paused. */
export function aiActive(data: AiQueueStatus | undefined) {
  if (!data) return false
  return data.running > 0 || (!data.paused && data.queued > 0)
}

export function useIntelligenceRuntime(paused: boolean) {
  const client = useQueryClient()
  const query = useQuery({
    queryKey: runtimeKey,
    queryFn: intelligenceClient.getRuntime,
    staleTime: 0,
    refetchInterval: POLL_MS,
  })

  // When a run of work drains, Insights read new data: refresh what shows it.
  const wasActive = useRef(false)
  const active = runtimeActive(query.data, paused)
  useEffect(() => {
    if (wasActive.current && !active) {
      void client.invalidateQueries({ queryKey: queryKeys.archiveData })
      // The last poll may have caught a job just before it finished.
      void client.invalidateQueries({ queryKey: jobsKey('insights') })
    }
    wasActive.current = active
  }, [active, client])

  return query
}

export function useAiQueue() {
  const client = useQueryClient()
  const query = useQuery({
    queryKey: aiQueueKey,
    queryFn: intelligenceClient.getQueueStatus,
    staleTime: 0,
    refetchInterval: AI_QUEUE_POLL_MS,
  })
  const wasActive = useRef(false)
  const active = aiActive(query.data)
  useEffect(() => {
    if (wasActive.current && !active) {
      void client.invalidateQueries({ queryKey: jobsKey('ai') })
    }
    wasActive.current = active
  }, [active, client])
  return query
}

const listJobs = {
  insights: intelligenceClient.listRuntimeJobs,
  ai: intelligenceClient.listQueueJobs,
} as const

/**
 * A queue's whole job history, newest first, read on a page at a time as the
 * list scrolls. Every page read so far is read again while the page is open,
 * so a job keeps its place and shows its current state wherever it is.
 * `onFold` drops the rest when the list folds, so a long history scrolled
 * once is not polled from then on.
 */
export function useJobHistory<T>(
  queue: QueueName,
  toView: (job: T) => JobView,
) {
  const client = useQueryClient()
  const query = useInfiniteQuery({
    queryKey: jobsKey(queue),
    queryFn: ({ pageParam }) =>
      (
        listJobs[queue] as (
          request: ListJobsRequest,
        ) => Promise<{ items: T[]; nextCursor: number | null }>
      )({ cursor: pageParam, limit: JOB_PAGE }),
    initialPageParam: null as number | null,
    getNextPageParam: (page) => page.nextCursor ?? undefined,
    staleTime: 0,
    refetchInterval: POLL_MS,
  })
  const jobs = useMemo(
    () => query.data?.pages.flatMap((page) => page.items.map(toView)) ?? [],
    [query.data, toView],
  )
  const { fetchNextPage } = query
  const onLoadMore = useCallback(() => void fetchNextPage(), [fetchNextPage])
  const onFold = useCallback(() => {
    client.setQueryData<InfiniteData<unknown, number | null>>(
      jobsKey(queue),
      (data) =>
        data && {
          pages: data.pages.slice(0, 1),
          pageParams: data.pageParams.slice(0, 1),
        },
    )
  }, [client, queue])
  return {
    jobs,
    loading: query.isPending,
    hasMore: query.hasNextPage,
    loadingMore: query.isFetchingNextPage,
    loadError: query.isError ? query.error : null,
    onLoadMore,
    onFold,
  }
}

/** Re-reads both queues, which also restarts their workers on the backend. */
export function useRefreshQueues() {
  const client = useQueryClient()
  return () =>
    Promise.all([
      client.invalidateQueries({ queryKey: runtimeKey }),
      client.invalidateQueries({ queryKey: aiQueueKey }),
    ])
}

function useFailureToast() {
  const { t } = useI18n()
  return (error: unknown, command: string) =>
    toast.error(t('settingsBackground.actionFailed'), {
      description: describeError(error, command),
    })
}

export function useRuntimeJobActions() {
  const { t } = useI18n()
  const client = useQueryClient()
  const failed = useFailureToast()
  const store = (snapshot: IntelligenceRuntimeSnapshot) => {
    client.setQueryData(runtimeKey, snapshot)
    void client.invalidateQueries({ queryKey: jobsKey('insights') })
  }

  const retry = useMutation({
    mutationFn: intelligenceClient.retryRuntimeJob,
    onSuccess: (snapshot) => {
      store(snapshot)
      toast.success(t('settingsBackground.retried'))
    },
    onError: (error) => failed(error, 'retry_intelligence_job'),
  })
  const cancel = useMutation({
    mutationFn: (job: JobView) => intelligenceClient.cancelRuntimeJob(job.id),
    onSuccess: (snapshot, job) => {
      store(snapshot)
      // A running job only gets a stop request; its worker ends it shortly.
      toast.success(
        job.state === 'running'
          ? t('settingsBackground.stopRequested')
          : t('settingsBackground.cancelled'),
      )
    },
    onError: (error) => failed(error, 'cancel_intelligence_job'),
  })
  return { retry, cancel }
}

export function useAiJobActions() {
  const { t } = useI18n()
  const client = useQueryClient()
  const failed = useFailureToast()
  const refresh = () =>
    Promise.all([
      client.invalidateQueries({ queryKey: aiQueueKey }),
      client.invalidateQueries({ queryKey: jobsKey('ai') }),
    ])

  const replay = useMutation({
    mutationFn: intelligenceClient.replayJob,
    onSuccess: () => {
      void refresh()
      toast.success(t('settingsBackground.retried'))
    },
    onError: (error) => failed(error, 'replay_ai_job'),
  })
  const cancel = useMutation({
    mutationFn: intelligenceClient.cancelJob,
    onSuccess: (job) => {
      void refresh()
      toast.success(
        job.state === 'running'
          ? t('settingsBackground.stopRequested')
          : t('settingsBackground.cancelled'),
      )
    },
    onError: (error) => failed(error, 'cancel_ai_job'),
  })
  const pause = useMutation({
    mutationFn: intelligenceClient.pauseJob,
    onSuccess: () => {
      void refresh()
      toast.success(t('settingsBackground.ai.pausing'))
    },
    onError: (error) => failed(error, 'pause_ai_job'),
  })
  return { replay, cancel, pause }
}

/** Queues a full Insights rebuild and shows it in the list right away. */
export function useQueueRebuild() {
  const { t } = useI18n()
  const client = useQueryClient()
  return useMutation({
    mutationFn: () =>
      intelligenceClient.queueCoreIntelligenceRebuild({ fullRebuild: true }),
    onSuccess: () => {
      toast.success(t('settingsBackground.insights.started'))
      void client.invalidateQueries({ queryKey: runtimeKey })
      void client.invalidateQueries({ queryKey: jobsKey('insights') })
    },
    onError: (error) =>
      toast.error(t('settingsBackground.insights.failed'), {
        description: describeError(error, 'queue_core_intelligence_rebuild'),
      }),
  })
}
