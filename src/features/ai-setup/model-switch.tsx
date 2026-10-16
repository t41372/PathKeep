/**
 * What switching the semantic search model does, said before it happens: the
 * new model needs its own index, search keeps the complete one until that is
 * built, the old index is deleted once it is, an unfinished one is not kept,
 * and with an embeddings service every search depends on that service.
 *
 * Responsibilities: those consequences, worked out from the queue's view of
 * the indexes, and the confirmation the model picker asks for. Not
 * responsible for saving the choice or building anything
 * (`useSemanticIndex`, `useAiProviders`).
 */
import { useQuery } from '@tanstack/react-query'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Skeleton } from '@/components/ui/skeleton'
import { intelligenceClient } from '@/lib/backend-client/intelligence'
import { useFormat, useI18n } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import {
  BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID,
  type EmbeddingIndexRef,
} from '@/lib/types'
import { aiQueueKey } from '../settings/background/use-background-work'
import { STATIC_MODEL_APPROX_BYTES } from './use-semantic-index'

/** The model semantic search would use instead of the chosen one. */
export interface SwitchTarget {
  /** Null for a service that is not saved yet. */
  providerId: string | null
  model: string
  /** The embeddings service that runs it; null for the in-app model. */
  service: { name: string; local: boolean } | null
}

/** `minishlab/potion-multilingual-128M` → `potion-multilingual-128M`. */
function shortModel(model: string) {
  return model.split('/').at(-1) ?? model
}

function sameRef(a: EmbeddingIndexRef, b: EmbeddingIndexRef | null) {
  return b !== null && a.providerId === b.providerId && a.model === b.model
}

function isTarget(ref: EmbeddingIndexRef | null, target: SwitchTarget) {
  if (!ref || ref.providerId !== target.providerId) return false
  // There is one in-app model; its index is the same whatever it is called.
  return (
    target.providerId === BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID ||
    ref.model === target.model
  )
}

/**
 * The consequences as a list, most important first. `dataNote`: also say what
 * an online service receives (off where the dialog already says it).
 */
export function SwitchConsequences({
  target,
  dataNote = true,
}: {
  target: SwitchTarget
  dataNote?: boolean
}) {
  const { t } = useI18n()
  const format = useFormat()
  const snapshot = useSnapshot()
  const queue = useQuery({
    queryKey: aiQueueKey,
    queryFn: intelligenceClient.getQueueStatus,
  })
  const estimate = useQuery({
    queryKey: ['ai', 'reembed-estimate', 'full'],
    queryFn: () => intelligenceClient.estimateReembed('full'),
    staleTime: 60_000,
  })
  // Which indexes exist decides most of what a switch does.
  if (!queue.data) return <Skeleton className="h-16 w-full" />
  const semantic = queue.data.semanticIndex
  const ai = snapshot.config.ai
  const on = ai.enabled && ai.semanticIndexEnabled
  const chosen = semantic.chosen
  // The complete index search reads: without one, `serving` is the chosen
  // model's own unfinished index.
  const complete = semantic.chosenComplete
    ? chosen
    : semantic.serving && !sameRef(semantic.serving, chosen)
      ? semantic.serving
      : null
  const model = shortModel(target.model)
  const lines: string[] = []

  if (isTarget(complete, target)) {
    lines.push(t('settingsAi.semantic.switch.ready', { model }))
  } else {
    const count = estimate.data?.pageCount ?? 0
    lines.push(
      count > 0
        ? t(
            on
              ? 'settingsAi.semantic.switch.newIndex'
              : 'settingsAi.semantic.switch.newIndexLater',
            { model, count: format.number(count) },
          )
        : t(
            on
              ? 'settingsAi.semantic.switch.newIndexCounting'
              : 'settingsAi.semantic.switch.newIndexLaterCounting',
            { model },
          ),
    )
    if (complete) {
      const old = shortModel(complete.model)
      lines.push(t('settingsAi.semantic.meanwhilePrevious', { model: old }))
      lines.push(t('settingsAi.semantic.switch.replaces', { old }))
    } else {
      lines.push(t('settingsAi.semantic.meanwhilePartial'))
    }
  }

  // The chosen model's unfinished index: job start keeps only the chosen and
  // the serving index (`semantic-index-control.md`).
  const unfinished =
    !semantic.chosenComplete && chosen && !isTarget(chosen, target)
      ? snapshot.aiStatus.indexedItems
      : 0
  if (chosen && unfinished > 0) {
    lines.push(
      t('settingsAi.semantic.switch.dropsUnfinished', {
        model: shortModel(chosen.model),
        count: unfinished,
      }),
    )
  }

  if (target.service) {
    lines.push(
      t('settingsAi.semantic.switch.service', { name: target.service.name }),
    )
    if (dataNote && !target.service.local) {
      lines.push(t('settingsAi.semantic.serviceRemoteNote'))
    }
  } else {
    lines.push(t('settingsAi.semantic.switch.static'))
    if (!snapshot.aiStatus.staticEmbedding?.modelDownloaded) {
      lines.push(
        t('settingsAi.semantic.switch.download', {
          size: format.bytes(
            snapshot.aiStatus.staticEmbedding?.modelSizeBytes ||
              STATIC_MODEL_APPROX_BYTES,
          ),
        }),
      )
    }
  }

  return (
    <ul className="flex list-disc flex-col gap-1.5 pl-5 text-[13px] text-foreground">
      {lines.map((line) => (
        <li key={line}>{line}</li>
      ))}
    </ul>
  )
}

/** The model picker's confirmation: nothing changes until it is accepted. */
export function SwitchModelDialog({
  target,
  name,
  onCancel,
  onConfirm,
}: {
  /** Null while closed. */
  target: SwitchTarget | null
  /** What the picker calls the target. */
  name: string
  onCancel: () => void
  onConfirm: () => void
}) {
  const { t } = useI18n()
  return (
    <AlertDialog
      open={target !== null}
      onOpenChange={(open) => !open && onCancel()}
    >
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>
            {t('settingsAi.semantic.switch.title', { name })}
          </AlertDialogTitle>
          <AlertDialogDescription asChild>
            <div>{target && <SwitchConsequences target={target} />}</div>
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>{t('common.cancel')}</AlertDialogCancel>
          <AlertDialogAction onClick={onConfirm}>
            {t('settingsAi.semantic.switch.confirm')}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
