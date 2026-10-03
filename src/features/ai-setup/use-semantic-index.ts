/**
 * Local semantic search as the UI drives it: switch on (save the setting,
 * download the on-device model, build the index), switch off, rebuild, and
 * live progress for both the download and the index build.
 *
 * The queue is polled only while a build is actually running. Not responsible
 * for any rendering or copy.
 */
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useCallback, useEffect, useRef, useState } from 'react'
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
  type ReembedScope,
} from '@/lib/types'

/**
 * potion-multilingual-128M: a 512 MB matrix plus an 18.6 MB tokenizer
 * (Hugging Face file listing). The backend reports the exact size only once
 * the files are on disk (`staticEmbedding.modelSizeBytes`).
 */
export const STATIC_MODEL_APPROX_BYTES = 531 * 1024 * 1024

const POLL_MS = 2000

/** Master switch stays on only while something else still needs it. */
function masterStaysOn(ai: AiSettings) {
  return ai.assistantEnabled || ai.mcpEnabled || ai.skillEnabled
}

export interface IndexProgress {
  embedded: number
  target: number
}

export function useSemanticIndex() {
  const client = useQueryClient()
  const snapshot = useSnapshot()
  const save = useSaveConfig()
  const status: AiIndexStatus = snapshot.aiStatus
  const ai = snapshot.config.ai
  const modelDownloaded = status.staticEmbedding?.modelDownloaded ?? false

  const cancelled = useRef(false)
  const [restart, setRestart] = useState(0)
  const [setupRunning, setSetupRunning] = useState(false)
  const [building, setBuilding] = useState(
    status.queuedJobs + status.runningJobs > 0 && status.state !== 'disabled',
  )
  const [error, setError] = useState<string | null>(null)
  const download = useModelDownloadProgress(modelDownloaded, cancelled, restart)

  const refreshSnapshot = useCallback(
    async () =>
      client.setQueryData(queryKeys.snapshot, await appClient.getSnapshot()),
    [client],
  )

  const queue = useQuery({
    queryKey: ['ai', 'queue'],
    queryFn: intelligenceClient.getQueueStatus,
    enabled: building,
    refetchInterval: POLL_MS,
    staleTime: 0,
  })

  const queueIdle =
    queue.data !== undefined &&
    queue.data.indexQueued + queue.data.indexRunning === 0
  useEffect(() => {
    if (!building || !queueIdle) return
    setBuilding(false)
    void refreshSnapshot().catch(() => undefined)
  }, [building, queueIdle, refreshSnapshot])

  const running = queue.data?.recentJobs.find(
    (job) => job.state === 'running' && job.progressEmbedTarget !== undefined,
  )
  const progress: IndexProgress | null =
    building && running && (running.progressEmbedTarget ?? 0) > 0
      ? {
          embedded: running.progressEmbedded ?? 0,
          target: running.progressEmbedTarget ?? 0,
        }
      : null

  const enabled = ai.enabled && ai.semanticIndexEnabled

  const enable = useCallback(async () => {
    setError(null)
    cancelled.current = false
    setSetupRunning(true)
    try {
      await save.mutateAsync((config) => {
        config.ai.enabled = true
        config.ai.semanticIndexEnabled = true
        config.ai.autoIndexAfterBackup = true
        config.ai.embeddingProviderId = BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID
        return config
      })
      setRestart((count) => count + 1)
      // Downloads the model if needed, then queues the full index build.
      await runLocalSemanticSetup()
      setBuilding(true)
      await refreshSnapshot()
    } catch (reason) {
      if (!cancelled.current) setError(describeError(reason))
    } finally {
      setSetupRunning(false)
    }
  }, [refreshSnapshot, save])

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

  const cancelDownload = useCallback(async () => {
    cancelled.current = true
    await intelligenceClient
      .cancelStaticEmbeddingModelDownload()
      .catch(() => undefined)
    await disable()
  }, [disable])

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
      setBuilding(true)
    },
    [status.state],
  )

  return {
    status,
    enabled,
    modelDownloaded,
    download,
    setupRunning,
    building,
    progress,
    error,
    enable,
    disable,
    cancelDownload,
    rebuild,
    saving: save.isPending,
  }
}
