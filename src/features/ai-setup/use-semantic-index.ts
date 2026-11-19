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
 * anything else started (a backup, Background work) shows here too.
 *
 * Another window can change the same settings. Saves here change the settings
 * as saved now, not this window's copy, and an action builds only the model it
 * was shown for: when the saved choice is another model, nothing is built (nor
 * saved, when that is seen before the save), the screen catches up and a
 * notice says so. Page text never goes to a service from another model's
 * button.
 *
 * Not responsible for rendering; its only copy is the notice it raises and
 * the error it keeps.
 */
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useCallback, useState } from 'react'
import { toast } from 'sonner'
import { appClient } from '@/lib/backend-client/app'
import { intelligenceClient } from '@/lib/backend-client/intelligence'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import {
  cancelModelDownload,
  MODEL_DOWNLOAD_IDLE_TIMEOUT_MS,
  useModelDownloadProgress,
} from '@/lib/ipc/model-download'
import {
  ModelDownloadCancelled,
  ModelDownloadStalled,
  runLocalSemanticSetup,
} from '@/lib/ipc/semantic-setup'
import { queryKeys } from '@/lib/query'
import { useSaveConfig, useSnapshot } from '@/lib/queries/app'
import {
  BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID,
  type AiIndexStatus,
  type AiSettings,
  type IndexBuildJob,
  type IndexBuildProgress,
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

const chosenModel = (ai: AiSettings) =>
  ai.embeddingProviderId ?? BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID

/** The saved settings choose another model than the one an action was for (`ai`, as saved). */
class ChoiceChanged extends Error {
  readonly ai: AiSettings

  constructor(ai: AiSettings) {
    super(`semantic search uses ${chosenModel(ai)} now`)
    this.name = 'ChoiceChanged'
    this.ai = ai
  }
}

export interface IndexProgress extends IndexBuildProgress {
  /** Moves smoothly between the counts the queue reports. */
  done: number
  total: number | null
  perSecond: number | null
  secondsLeft: number | null
}

export function useSemanticIndex() {
  const { t } = useI18n()
  const client = useQueryClient()
  const snapshot = useSnapshot()
  const save = useSaveConfig({ fromSaved: true })
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

  const [setupRunning, setSetupRunning] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const download = useModelDownloadProgress(modelDownloaded)

  const refresh = useCallback(async () => {
    await client.invalidateQueries({ queryKey: aiQueueKey })
    const shown = client.getQueryData(queryKeys.snapshot)
    const current = await appClient.getSnapshot()
    // A save answered meanwhile shows settings at least as new as this read.
    if (client.getQueryData(queryKeys.snapshot) === shown)
      client.setQueryData(queryKeys.snapshot, current)
  }, [client])

  /**
   * How an action ended, for `error` and the notices. A stop the user asked for is not a
   * failure; nor is another window's choice of model, which the screen then shows.
   */
  const failed = useCallback(
    function failed(reason: unknown) {
      const catchUp = () => void refresh().catch(() => undefined)
      if (reason instanceof ChoiceChanged) {
        const chosen = chosenModel(reason.ai)
        const service = reason.ai.embeddingProviders.find(
          (provider) => provider.id === chosen,
        )
        toast.info(
          chosen === BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID
            ? t('settingsAi.semantic.choiceChanged.inApp')
            : t('settingsAi.semantic.choiceChanged.service', {
                name: service?.name ?? chosen,
              }),
          { id: 'semantic-choice-changed' },
        )
        catchUp()
        return
      }
      // The saved settings stopped it: the screen shows them.
      if (reason instanceof ModelDownloadCancelled) return catchUp()
      if (reason instanceof ModelDownloadStalled) {
        setError(
          t('settingsAi.semantic.downloadStalled', {
            count: MODEL_DOWNLOAD_IDLE_TIMEOUT_MS / 60_000,
          }),
        )
        // The quiet download can still end: its build is then asked for, and the screen
        // catches up with how it ended.
        reason.resumed?.then(
          () => {
            setError(null)
            catchUp()
          },
          (late: unknown) => {
            setError(null)
            failed(late)
          },
        )
        return
      }
      setError(describeError(reason))
    },
    [refresh, t],
  )

  const queue = useQuery({
    queryKey: aiQueueKey,
    queryFn: intelligenceClient.getQueueStatus,
    refetchInterval: AI_QUEUE_POLL_MS,
    staleTime: 0,
  })
  const semantic = queue.data?.semanticIndex ?? null
  const build: IndexBuildJob | null = semantic?.build ?? null

  const done = build?.done ?? 0
  const total = build?.total ?? null
  const shownDone = useSmoothCount(
    done,
    build ? `${build.jobId}:${build.progress.phase}` : null,
  )
  const pace = useIndexPace(
    build?.state === 'running' && build.progress.phase === 'visits'
      ? build.jobId
      : null,
    done,
    total ?? 0,
    queue.dataUpdatedAt,
  )
  const progress: IndexProgress | null = build
    ? {
        ...build.progress,
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

  /**
   * Builds the chosen model's index: reads every visit it does not have yet,
   * so an unfinished index is continued and never wiped (only Rebuild starts
   * from nothing). The in-app model is downloaded first if it isn't here yet.
   * The request names the model, so the backend refuses it when the saved
   * choice is another one (`ChoiceChanged`).
   */
  const buildWith = useCallback(
    async (providerId: string) => {
      if (providerId !== BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID) {
        try {
          // `incremental`: every visit the index lacks. `full` would wipe it first.
          await intelligenceClient.buildIndex({
            providerId,
            fullRebuild: false,
            clearOnly: false,
            scope: 'incremental',
          })
        } catch (reason) {
          const saved = (await appClient.getSnapshot()).config.ai
          if (chosenModel(saved) !== providerId) throw new ChoiceChanged(saved)
          throw reason
        }
      } else {
        // A save that stops wanting the model stops the download (the backend does it), and
        // the backend refuses the build it would have allowed.
        await runLocalSemanticSetup()
      }
      await refresh()
    },
    [refresh],
  )

  /**
   * Turning semantic search on, or the in-app model's Download or Retry, is the user's go-ahead
   * for the first build of `requested`, the model the screen showed. When the saved settings
   * choose another model, nothing is saved, downloaded or built (`ChoiceChanged`).
   */
  const enable = useCallback(
    async (requested: string) => {
      setError(null)
      setSetupRunning(true)
      try {
        const saved = await save.mutateAsync((config) => {
          if (chosenModel(config.ai) !== requested)
            throw new ChoiceChanged(config.ai)
          config.ai.enabled = true
          config.ai.semanticIndexEnabled = true
          config.ai.embeddingProviderId = requested
          // On by default, shown right under the switch where it can be turned off.
          config.ai.autoIndexAfterBackup = true
          return config
        })
        // Another window's choice saved between the read and this save.
        if (chosenModel(saved.config.ai) !== requested)
          throw new ChoiceChanged(saved.config.ai)
        await buildWith(requested)
      } catch (reason) {
        failed(reason)
      } finally {
        setSetupRunning(false)
      }
    },
    [buildWith, failed, save],
  )

  /** Chooses the model; its index is built only when the user says so. */
  const chooseModel = useCallback(
    async (providerId: string) => {
      setError(null)
      try {
        // Choosing another service stops a built-in download once saved (backend rule).
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
      // Turning it off stops a built-in download once saved (backend rule).
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
    await cancelModelDownload().catch(() => false)
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
    // A cancelled download (a switch to another service mid-build) was asked for.
    onError: failed,
  })

  /**
   * Rebuilds the index of the model this screen shows from nothing (Rebuild
   * search index). The request names that model, so when another window chose
   * a different one the backend refuses it and nothing is sent anywhere.
   */
  const rebuild = useCallback(
    async (scope: ReembedScope = 'full') => {
      setError(null)
      // A failed or empty index needs the stuck job cleared first.
      if (status.state === 'failed' || status.state === 'degraded') {
        await intelligenceClient.resetAiIndexBuild()
      } else {
        try {
          await intelligenceClient.buildIndex({
            providerId: modelId,
            fullRebuild: true,
            clearOnly: false,
            scope,
          })
        } catch (reason) {
          const saved = (await appClient.getSnapshot()).config.ai
          if (chosenModel(saved) !== modelId)
            return failed(new ChoiceChanged(saved))
          throw reason
        }
      }
      await refresh()
    },
    [failed, modelId, refresh, status.state],
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
