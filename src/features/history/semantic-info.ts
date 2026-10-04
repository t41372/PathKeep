/**
 * What semantic search is running on, or why it is not available, from
 * `aiStatus` and the AI config: provider and model, index size and age, or
 * the reason it is off. Shown by `SemanticNote` and the mode toggle.
 */
import { useI18n } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import { BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID } from '@/lib/types'

export type SemanticReason =
  | 'off'
  | 'building'
  | 'queued'
  | 'paused'
  | 'empty'
  | 'failed'
  | 'degraded'
  | 'blocked'

export interface SemanticInfo {
  available: boolean
  /** Why semantic search is unavailable; null when it is available. */
  reason: SemanticReason | null
  /** Provider and model, e.g. "Built-in model (potion-multilingual-128M)". */
  provider: string | null
  indexed: number
  lastIndexedAt: string | null
  /** The archive changed since the index was built; results miss new pages. */
  stale: boolean
}

function reasonOf(state: string, on: boolean): SemanticReason {
  if (!on) return 'off'
  switch (state) {
    case 'rebuilding':
      return 'building'
    case 'queued':
    case 'paused':
    case 'failed':
    case 'degraded':
    case 'blocked':
      return state
    default:
      return 'empty'
  }
}

export function useSemanticInfo(): SemanticInfo {
  const { t } = useI18n()
  const snapshot = useSnapshot()
  const ai = snapshot.config.ai
  const status = snapshot.aiStatus
  const on = ai.enabled && ai.semanticIndexEnabled
  const available = on && status.ready

  const providerId = status.embeddingProviderId ?? ai.embeddingProviderId
  let provider: string | null = null
  if (providerId === BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID) {
    const model = status.staticEmbedding?.modelRepo.split('/').pop()
    provider = model
      ? t('historySearch.semantic.builtInModel', { model })
      : t('historySearch.semantic.builtIn')
  } else if (providerId) {
    const config = ai.embeddingProviders.find((item) => item.id === providerId)
    provider = config
      ? [config.name, config.defaultModel].filter(Boolean).join(' · ')
      : providerId
  }

  return {
    available,
    reason: available ? null : reasonOf(status.state, on),
    provider,
    indexed: status.indexedItems,
    lastIndexedAt: status.lastIndexedAt ?? null,
    stale: status.state === 'stale',
  }
}

/** One sentence for why semantic search is unavailable, for the toggle's tooltip. */
export function useSemanticReasonText(info: SemanticInfo) {
  const { t } = useI18n()
  return info.reason ? t(`historySearch.semantic.reason.${info.reason}`) : null
}
