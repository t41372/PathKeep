/**
 * Local semantic search as the UI drives it: switch on (save the setting,
 * download the in-app model, build the first index), switch off, choose the
 * model, and the chosen model's index build: start, pause, continue, cancel,
 * with its live progress and pace.
 *
 * Changing the model builds nothing. The queue reports which index search
 * reads (`semanticIndex.serving`) and how far the chosen model's is; the user
 * starts its build, and search keeps reading the last complete index until
 * the new one is complete.
 *
 * The queue is read every second while this is mounted, so a build that
 * anything else started (a backup, Background work) shows here too. Not
 * responsible for any rendering or copy.
 */
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useCallback, useRef, useState } from 'react'
import { appClient } from '@/lib/backend-client/app'
import { intelligenceClient } from '@/lib/backend-client/intelligence'
import { describeError } from '@/lib/errors'
import { useModelDownloadProgress } from '@/lib/ipc/model-download'
import { runLocalSemanticSetup } from '@/lib/ipc/semantic-setup'
import { queryKeys } from '@/lib/query'
import { useSaveConfig, useSnapshot } from '@/lib/queries/app'
import {
  BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID,
  type AiIndexStatus,
  type AiSettings,
  type IndexBuildJob,
  type ReembedScope,
} from '@/lib/types'
import { aiQueueKey } from '../settings/background/use-background-work'
import { useIndexPace, useSmoothCount } from './use-index-pace'

/**
 * potion-multilingual-128M: a 512 MB matrix plus an 18.6 MB tokenizer
 * (Hugging Face file listing). The backend reports the exact size only once
 * the files are on disk (`staticEmbedding.modelSizeBytes`).
 */
export const STATIC_MODEL_APPROX_BYTES = 531 * 1024 * 1024

/** Shared with Settings → Background work, which reads the same query. */
export const AI_QUEUE_POLL_MS = 1000

/** Master switch stays on only while something else still needs it. */
function masterStaysOn(ai: AiSettings) {
  return ai.assistantEnabled || ai.mcpEnabled || ai.skillEnabled
}

export interface IndexProgress {
  /** Moves smoothly between the counts the queue reports. */
  done: number
  total: number
  perSecond: number | null
  secondsLeft: number | null
}

export function useSemanticIndex() {
  const client = useQueryClient()
  const snapshot = useSnapshot()
  const save = useSaveConfig()
  const status: AiIndexStatus = snapshot.aiStatus
  // Why the chosen model's index is behind. `textFormat` (built from an older
  // version of the page text) is the one an update never fixes: only a rebuild.
  const staleReason =
    status.warningCode?.code === 'indexStale' ? status.warningCode.reason : null
  const ai = snapshot.config.ai
  // Only the in-app model is downloaded; an embeddings service set in the
  // config is ready as it is.
  const modelId =
    ai.embeddingProviderId ?? BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID
  const usesInAppModel = modelId === BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID
  const modelDownloaded =
    !usesInAppModel || (status.staticEmbedding?.modelDownloaded ?? false)
  const enabled = ai.enabled && ai.semanticIndexEnabled

  const cancelled = useRef(false)
  const [restart, setRestart] = useState(0)
  const [setupRunning, setSetupRunning] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const download = useModelDownloadProgress(modelDownloaded, cancelled, restart)

  const queue = useQuery({
    queryKey: aiQueueKey,
    queryFn: intelligenceClient.getQueueStatus,
    refetchInterval: AI_QUEUE_POLL_MS,
    staleTime: 0,
  })
  const semantic = queue.data?.semanticIndex ?? null
  const build: IndexBuildJob | null = semantic?.build ?? null

  const done = build?.done ?? 0
  const total = build?.total ?? 0
  const shownDone = useSmoothCount(done)
  const pace = useIndexPace(
    build?.state === 'running' ? build.jobId : null,
    done,
    total,
    queue.dataUpdatedAt,
  )
  const progress: IndexProgress | null = build
    ? {
        done: Math.min(shownDone, done),
        total,
        perSecond: pace?.perSecond ?? null,
        secondsLeft: pace?.secondsLeft ?? null,
      }
    : null

  /** The service chosen in Settings, or null for the in-app model. */
  const service = usesInAppModel
    ? null
    : (ai.embeddingProviders.find((provider) => provider.id === modelId) ??
      null)
  const services = ai.embeddingProviders.filter(
    (provider) => provider.id !== BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID,
  )

  const refresh = useCallback(async () => {
    await client.invalidateQueries({ queryKey: aiQueueKey })
    client.setQueryData(queryKeys.snapshot, await appClient.getSnapshot())
  }, [client])

  /**
   * Builds the chosen model's index: reads every visit it does not have yet,
   * so an unfinished index is continued and never wiped (only Rebuild starts
   * from nothing). The in-app model is downloaded first if it isn't here yet.
   */
  const buildWith = useCallback(
    async (providerId: string) => {
      if (providerId !== BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID) {
        // `incremental`: every visit the index lacks. `full` would wipe it first.
        await intelligenceClient.buildIndex({
          fullRebuild: false,
          clearOnly: false,
          scope: 'incremental',
        })
      } else {
        cancelled.current = false
        setRestart((count) => count + 1)
        await runLocalSemanticSetup()
      }
      await refresh()
    },
    [refresh],
  )

  /** Turning semantic search on is the user's go-ahead for the first build. */
  const enable = useCallback(async () => {
    setError(null)
    cancelled.current = false
    setSetupRunning(true)
    try {
      const saved = await save.mutateAsync((config) => {
        config.ai.enabled = true
        config.ai.semanticIndexEnabled = true
        config.ai.embeddingProviderId ??= BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID
        // On by default, shown right under the switch where it can be turned off.
        config.ai.autoIndexAfterBackup = true
        return config
      })
      await buildWith(
        saved.config.ai.embeddingProviderId ??
          BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID,
      )
    } catch (reason) {
      if (!cancelled.current) setError(describeError(reason))
    } finally {
      setSetupRunning(false)
    }
  }, [buildWith, save])

  /** Chooses the model; its index is built only when the user says so. */
  const chooseModel = useCallback(
    async (providerId: string) => {
      setError(null)
      try {
        await save.mutateAsync((config) => {
          config.ai.embeddingProviderId = providerId
          return config
        })
        await refresh()
      } catch (reason) {
        setError(describeError(reason))
      }
    },
    [refresh, save],
  )

  const disable = useCallback(async () => {
    setError(null)
    try {
      await save.mutateAsync((config) => {
        config.ai.semanticIndexEnabled = false
        config.ai.enabled = masterStaysOn(config.ai)
        return config
      })
    } catch (reason) {
      setError(describeError(reason))
    }
  }, [save])

  const setAutoUpdate = useCallback(
    (next: boolean) =>
      save.mutateAsync((config) => {
        config.ai.autoIndexAfterBackup = next
        return config
      }),
    [save],
  )

  const cancelDownload = useCallback(async () => {
    cancelled.current = true
    await intelligenceClient
      .cancelStaticEmbeddingModelDownload()
      .catch(() => undefined)
    await disable()
  }, [disable])

  const act = useMutation({
    mutationFn: async (
      action:
        | { kind: 'start' }
        | { kind: 'pause' | 'resume' | 'cancel'; jobId: number },
    ) => {
      switch (action.kind) {
        case 'start':
          return buildWith(modelId)
        case 'pause':
          return intelligenceClient.pauseJob(action.jobId)
        case 'resume':
          return intelligenceClient.replayJob(action.jobId)
        case 'cancel':
          return intelligenceClient.cancelJob(action.jobId)
      }
    },
    onMutate: () => setError(null),
    onSettled: () => void refresh().catch(() => undefined),
    onError: (reason) => setError(describeError(reason)),
  })

  /** Rebuilds the chosen model's index from nothing (Rebuild search index). */
  const rebuild = useCallback(
    async (scope: ReembedScope = 'full') => {
      setError(null)
      // A failed or empty index needs the stuck job cleared first.
      if (status.state === 'failed' || status.state === 'degraded') {
        await intelligenceClient.resetAiIndexBuild()
      } else {
        await intelligenceClient.buildIndex({
          fullRebuild: true,
          clearOnly: false,
          scope,
        })
      }
      await refresh()
    },
    [refresh, status.state],
  )

  return {
    status,
    staleReason,
    enabled,
    modelDownloaded,
    download,
    setupRunning,
    /** Which index search reads and how far the chosen model's is; null until read. */
    semantic,
    build,
    progress,
    error,
    modelId,
    service,
    services,
    enable,
    chooseModel,
    disable,
    setAutoUpdate,
    cancelDownload,
    start: () => act.mutate({ kind: 'start' }),
    pause: (jobId: number) => act.mutate({ kind: 'pause', jobId }),
    resume: (jobId: number) => act.mutate({ kind: 'resume', jobId }),
    cancel: (jobId: number) => act.mutate({ kind: 'cancel', jobId }),
    acting: act.isPending,
    rebuild,
    saving: save.isPending,
  }
}
