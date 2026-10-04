/**
 * What the app knows about AI providers: the kinds the user can add, whether
 * the selected assistant provider is usable, and how to label it.
 *
 * Shared by Ask (to decide between chat and the setup screen), Settings → AI
 * and onboarding. Not responsible for any UI.
 */
import type { AiProviderConfig, AiRequestFormat, AiSettings } from '@/lib/types'

export type ProviderKind =
  | 'openai'
  | 'anthropic'
  | 'gemini'
  | 'ollama'
  | 'lm-studio'

export interface ProviderPreset {
  kind: ProviderKind
  requestFormat: AiRequestFormat
  /** Pre-filled address. Empty means the provider's own default is used. */
  baseUrl: string
  /** Shown in the empty address field. */
  baseUrlHint: string
  /** Model name to suggest; empty when the right one depends on the account. */
  modelHint: string
  needsKey: boolean
}

export const providerPresets: readonly ProviderPreset[] = [
  {
    kind: 'ollama',
    requestFormat: 'ollama',
    baseUrl: 'http://localhost:11434',
    baseUrlHint: '',
    modelHint: 'llama3.2:3b',
    needsKey: false,
  },
  {
    kind: 'lm-studio',
    requestFormat: 'lm-studio',
    baseUrl: 'http://localhost:1234/v1',
    baseUrlHint: '',
    modelHint: '',
    needsKey: false,
  },
  {
    kind: 'openai',
    requestFormat: 'openai',
    baseUrl: '',
    baseUrlHint: 'https://api.openai.com/v1',
    modelHint: 'gpt-4o-mini',
    needsKey: true,
  },
  {
    kind: 'anthropic',
    requestFormat: 'anthropic',
    baseUrl: '',
    baseUrlHint: 'https://api.anthropic.com',
    modelHint: 'claude-3-5-haiku-latest',
    needsKey: true,
  },
  {
    kind: 'gemini',
    requestFormat: 'google',
    baseUrl: '',
    baseUrlHint: 'https://generativelanguage.googleapis.com',
    modelHint: 'gemini-2.0-flash',
    needsKey: true,
  },
]

export function presetFor(kind: ProviderKind) {
  return (
    providerPresets.find((preset) => preset.kind === kind) ?? providerPresets[0]
  )
}

const LOCAL_HOSTS = new Set(['localhost', '127.0.0.1', '[::1]', '0.0.0.0'])

/** Local means the request never leaves this computer. */
export function isLocalUrl(url: string | null | undefined) {
  try {
    return LOCAL_HOSTS.has(new URL(url ?? '').hostname)
  } catch {
    return false
  }
}

export function isLocalProvider(provider: AiProviderConfig) {
  if (
    provider.requestFormat === 'ollama' ||
    provider.requestFormat === 'lm-studio'
  ) {
    return true
  }
  return isLocalUrl(provider.baseUrl)
}

export function selectedLlmProvider(ai: AiSettings): AiProviderConfig | null {
  return (
    ai.llmProviders.find((provider) => provider.id === ai.llmProviderId) ?? null
  )
}

/** The assistant can run: switched on, and its provider has what it needs. */
export function usableAssistantProvider(
  ai: AiSettings,
): AiProviderConfig | null {
  if (!ai.enabled || !ai.assistantEnabled) return null
  const provider = selectedLlmProvider(ai)
  if (!provider || !provider.enabled || !provider.defaultModel.trim())
    return null
  if (!isLocalProvider(provider) && !provider.apiKeySaved) return null
  return provider
}

export function providerLabel(provider: AiProviderConfig) {
  return `${provider.name} · ${provider.defaultModel}`
}

export function newProviderId(kind: ProviderKind) {
  return `llm-${kind}-${crypto.randomUUID().slice(0, 8)}`
}

export interface ProviderDraft {
  kind: ProviderKind
  name: string
  baseUrl: string
  model: string
  apiKey: string
}

export function emptyDraft(kind: ProviderKind = 'ollama'): ProviderDraft {
  const preset = presetFor(kind)
  return { kind, name: '', baseUrl: preset.baseUrl, model: '', apiKey: '' }
}

export function draftToProvider(
  draft: ProviderDraft,
  id: string,
  label: string,
): AiProviderConfig {
  const preset = presetFor(draft.kind)
  const model = draft.model.trim()
  return {
    id,
    name: draft.name.trim() || label,
    purpose: 'llm',
    requestFormat: preset.requestFormat,
    enabled: true,
    baseUrl: draft.baseUrl.trim() || null,
    apiKeySaved: false,
    defaultModel: model,
    modelCatalog: model ? [model] : [],
  }
}

/** Hosted services need a key; anything on this computer does not. */
export function draftNeedsKey(draft: ProviderDraft) {
  const preset = presetFor(draft.kind)
  return preset.needsKey && !isLocalUrl(draft.baseUrl)
}

export function draftIsComplete(draft: ProviderDraft) {
  if (!draft.model.trim()) return false
  return !draftNeedsKey(draft) || draft.apiKey.trim().length > 0
}
