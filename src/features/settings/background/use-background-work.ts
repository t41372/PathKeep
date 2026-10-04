/**
 * The two background queues as Settings → Background work reads them: the
 * Insights queue (`load_intelligence_runtime`: rebuilds, title tidying, page
 * summaries) and the search-index / assistant queue (`load_ai_queue_status`).
 *
 * Responsibilities: the status queries, polling them every 2 s only while one
 * of them has work running (or waiting while not paused), and the per-job
 * actions the backend allows. Both status reads also restart the backend's
 * drain workers, so refetching after "resume" is what gets work moving again.
 *
 * Not responsible for rendering or copy.
 */
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect, useRef } from 'react'
import { toast } from 'sonner'
import { intelligenceClient } from '@/lib/backend-client/intelligence'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { queryKeys } from '@/lib/query'
import type { AiQueueStatus, IntelligenceRuntimeSnapshot } from '@/lib/types'

/** Shared with `use-semantic-index`, so the two never poll the same queue twice. */
export const aiQueueKey = ['ai', 'queue'] as const
export const runtimeKey = ['intelligence', 'runtime'] as const

const POLL_MS = 2000

function runtimeActive(
  data: IntelligenceRuntimeSnapshot | undefined,
  paused: boolean,
) {
  if (!data) return false
  return data.queue.running > 0 || (!paused && data.queue.queued > 0)
}

function aiActive(data: AiQueueStatus | undefined) {
  if (!data) return false
  return data.running > 0 || (!data.paused && data.queued > 0)
}

export function useIntelligenceRuntime(paused: boolean) {
  const client = useQueryClient()
  const query = useQuery({
    queryKey: runtimeKey,
    queryFn: intelligenceClient.getRuntime,
    staleTime: 0,
    refetchInterval: (current) =>
      runtimeActive(current.state.data, paused) ? POLL_MS : false,
  })

  // When a run of work drains, Insights read new data: refresh what shows it.
  const wasActive = useRef(false)
  const active = runtimeActive(query.data, paused)
  useEffect(() => {
    if (wasActive.current && !active) {
      void client.invalidateQueries({ queryKey: queryKeys.archiveData })
    }
    wasActive.current = active
  }, [active, client])

  return query
}

export function useAiQueue() {
  return useQuery({
    queryKey: aiQueueKey,
    queryFn: intelligenceClient.getQueueStatus,
    staleTime: 0,
    refetchInterval: (current) =>
      aiActive(current.state.data) ? POLL_MS : false,
  })
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
  const store = (snapshot: IntelligenceRuntimeSnapshot) =>
    client.setQueryData(runtimeKey, snapshot)

  const retry = useMutation({
    mutationFn: intelligenceClient.retryRuntimeJob,
    onSuccess: (snapshot) => {
      store(snapshot)
      toast.success(t('settingsBackground.retried'))
    },
    onError: (error) => failed(error, 'retry_intelligence_job'),
  })
  const cancel = useMutation({
    mutationFn: intelligenceClient.cancelRuntimeJob,
    onSuccess: (snapshot, jobId) => {
      store(snapshot)
      // A running job only gets a stop request; its worker ends it shortly.
      const stillRunning = snapshot.recentJobs.some(
        (job) => job.id === jobId && job.state === 'running',
      )
      toast.success(
        stillRunning
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
  const refresh = () => client.invalidateQueries({ queryKey: aiQueueKey })

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
  return { replay, cancel }
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
    },
    onError: (error) =>
      toast.error(t('settingsBackground.insights.failed'), {
        description: describeError(error, 'queue_core_intelligence_rebuild'),
      }),
  })
}
