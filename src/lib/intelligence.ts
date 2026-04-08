import type {
  AiAssistantResponse,
  AiAssistantCitation,
  AiIndexStatus,
  AiProviderConfig,
  AppConfig,
  InsightEvidenceItem,
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

export function assistantHref(question: string) {
  const params = new URLSearchParams()
  params.set('question', question)
  return `/assistant?${params.toString()}`
}

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
