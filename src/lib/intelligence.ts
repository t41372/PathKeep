/**
 * This module holds UI-facing intelligence helpers such as provider state, evidence links, and assistant response metadata.
 *
 * Why this file exists:
 * - Files in `src/lib/` are where UI policy becomes testable without inflating every route component.
 * - If you are trying to understand a front-end contract quickly, these helpers usually explain the reusable part of the story.
 *
 * Main declarations:
 * - `IntelligenceTone`
 * - `selectedAiProvider`
 * - `aiStatusMeta`
 * - `scoreBand`
 * - `evidenceHref`
 * - `assistantHref`
 * - `dedupeEvidence`
 * - `assistantResponseMeta`
 *
 * Source-of-truth notes:
 * - Keep helper behavior aligned with the shipping design, feature, and architecture docs rather than local route assumptions.
 * - Avoid burying user-visible copy or route-only workflow rules here unless the helper truly owns that cross-cutting contract.
 */

import type {
  AiAssistantResponse,
  AiAssistantCitation,
  AiIndexStatus,
  AiProviderConfig,
  AppConfig,
  InsightEvidenceItem,
} from './types'

/**
 * Defines the type-level contract for intelligence tone.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export type IntelligenceTone = 'success' | 'warning' | 'blocked' | 'info'
/**
 * Defines the type-level contract for translate.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
type Translate = (key: string, vars?: Record<string, string | number>) => string

/**
 * Provides selected ai to descendant components.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
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

/**
 * Explains how ai status meta works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
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
 * Explains how score band works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function scoreBand(
  score: number | null | undefined,
  t: Translate,
): {
  label: string
  tone: IntelligenceTone
} {
  if (score == null) return { label: t('noScore'), tone: 'info' }
  if (score >= 0.85) return { label: t('highConfidence'), tone: 'success' }
  if (score >= 0.65) return { label: t('relevant'), tone: 'warning' }
  return { label: t('weakMatch'), tone: 'info' }
}

/**
 * Explains how evidence href works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function evidenceHref(evidence: {
  profileId?: string | null
  url?: string | null
  domain?: string | null
  title?: string | null
}) {
  const params = new URLSearchParams()
  if (evidence.profileId) params.set('profileId', evidence.profileId)
  if (evidence.domain) params.set('domain', evidence.domain)
  if (evidence.url) params.set('q', evidence.url)
  else if (evidence.title) params.set('q', evidence.title)
  const query = params.toString()
  return query ? `/explorer?${query}` : '/explorer'
}

/**
 * Explains how assistant href works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function assistantHref(question: string) {
  const params = new URLSearchParams()
  params.set('question', question)
  return `/assistant?${params.toString()}`
}

/**
 * Explains how dedupe evidence works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function dedupeEvidence<
  T extends AiAssistantCitation | InsightEvidenceItem,
>(items: T[]) {
  const seen = new Set<string>()
  return items.filter((item) => {
    const key = `${item.historyId}:${item.url}`
    if (seen.has(key)) return false
    seen.add(key)
    return true
  })
}

/**
 * Explains how assistant response meta works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
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
