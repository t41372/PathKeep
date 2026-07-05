/**
 * UI-facing AI/provider/assistant presentation helpers shared by Dashboard,
 * Explorer, Assistant, and Settings.
 *
 * Why this file exists:
 * - M11 moves AI status and assistant-tone copy out of the mixed intelligence
 *   route helper surface so route grammar and UI presentation stop sharing one
 *   file.
 */

import type {
  AiAssistantResponse,
  AiIndexStatus,
  AiProviderConfig,
  AppConfig,
} from './types'

export type IntelligenceTone = 'success' | 'warning' | 'blocked' | 'info'

type Translate = (key: string, vars?: Record<string, string | number>) => string

export function selectedAiProvider(
  config: AppConfig['ai'],
  purpose: 'embedding' | 'llm',
): AiProviderConfig | null {
  const providerId =
    purpose === 'embedding' ? config.embeddingProviderId : config.llmProviderId
  const providers =
    purpose === 'embedding' ? config.embeddingProviders : config.llmProviders
  return providers.find((provider) => provider.id === providerId) ?? null
}

export function aiStatusMeta(
  status: AiIndexStatus,
  t: Translate,
): {
  label: string
  tone: IntelligenceTone
  description: string
} {
  switch (status.state) {
    case 'ready':
      return {
        label: t('statusReadyLabel'),
        tone: 'success',
        description: t('statusReadyDescription', {
          count: status.indexedItems,
        }),
      }
    case 'rebuilding':
      return {
        label: t('statusRebuildingLabel'),
        tone: 'warning',
        description: t('statusRebuildingDescription'),
      }
    case 'queued':
      return {
        label: t('statusQueuedLabel'),
        tone: 'warning',
        description: t('statusQueuedDescription'),
      }
    case 'paused':
      return {
        label: t('statusPausedLabel'),
        tone: 'warning',
        description: t('statusPausedDescription'),
      }
    case 'failed':
      return {
        label: t('statusFailedLabel'),
        tone: 'blocked',
        description: status.warning ?? t('statusFailedDescription'),
      }
    case 'stale':
      return {
        label: t('statusStaleLabel'),
        tone: 'warning',
        description: status.warning ?? t('statusStaleDescription'),
      }
    case 'degraded':
      return {
        label: t('statusDegradedLabel'),
        tone: 'blocked',
        description: status.warning ?? t('statusDegradedDescription'),
      }
    case 'blocked':
      return {
        label: t('statusBlockedLabel'),
        tone: 'blocked',
        description: status.warning ?? t('statusBlockedDescription'),
      }
    case 'disabled':
      return {
        label: t('statusDisabledLabel'),
        tone: 'info',
        description: t('statusDisabledDescription'),
      }
    default:
      return {
        label: t('statusEmptyLabel'),
        tone: 'info',
        description: status.warning ?? t('statusEmptyDescription'),
      }
  }
}

/**
 * Map a semantic/hybrid relevance score to a labelled confidence band.
 *
 * The tones form an honest relevance LADDER, not a status signal: the stronger
 * the match the more accent it earns, the weaker the fainter it reads. The mid
 * tier deliberately does NOT use `warning` — "Relevant" is a positive result,
 * and a caution tone on a good match misreads as a problem. The three tiers map
 * to accent (`success`) → neutral (`info`) → faint (`blocked`); an absent score
 * reads as the faintest "unknown". See `bandToneClass` in `paper-search-result`
 * for the matching pill palette.
 */
export function scoreBand(
  score: number | null | undefined,
  t: Translate,
): {
  label: string
  tone: IntelligenceTone
} {
  if (score == null) return { label: t('noScore'), tone: 'blocked' }
  if (score >= 0.85) return { label: t('highConfidence'), tone: 'success' }
  if (score >= 0.65) return { label: t('relevant'), tone: 'info' }
  return { label: t('weakMatch'), tone: 'blocked' }
}

export function assistantResponseMeta(
  response: AiAssistantResponse,
  t: Translate,
): {
  label: string
  tone: IntelligenceTone
} {
  switch (response.state) {
    case 'completed':
      return { label: t('answerReady'), tone: 'success' }
    case 'queued':
      return { label: t('queued'), tone: 'warning' }
    case 'insufficient-evidence':
      return { label: t('evidenceMissing'), tone: 'blocked' }
    case 'failed':
      return { label: t('assistantFailed'), tone: 'blocked' }
    case 'cancelled':
      return { label: t('cancelled'), tone: 'info' }
    default:
      return { label: t('inProgress'), tone: 'info' }
  }
}
