/**
 * What the app knows about AI providers: the kinds the user can add for the
 * assistant and for semantic search's embeddings, whether the selected
 * assistant provider is usable, and how to label one.
 *
 * Shared by Ask (to decide between chat and the setup screen), Settings → AI
 * and onboarding. Not responsible for any UI.
 */
import type {
  AiProviderConfig,
  AiProviderPurpose,
  AiRequestFormat,
  AiSettings,
} from '@/lib/types'

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
    baseUrl: 'http://localhost:11434/v1',
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

/**
 * Services that turn text into embeddings for semantic search. PathKeep sends
 * them OpenAI's `/embeddings` request, so every address ends in `/v1`
 * (Ollama and LM Studio both answer it there). Anthropic has no embeddings,
 * and Gemini's are not OpenAI-shaped.
 */
export const embeddingPresets: readonly ProviderPreset[] = [
  {
    kind: 'ollama',
    requestFormat: 'ollama',
    baseUrl: 'http://localhost:11434/v1',
    baseUrlHint: '',
    modelHint: 'nomic-embed-text',
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
    baseUrl: 'https://api.openai.com/v1',
    baseUrlHint: '',
    modelHint: 'text-embedding-3-small',
    needsKey: true,
  },
]

export function presetsFor(purpose: AiProviderPurpose = 'llm') {
  return purpose === 'embedding' ? embeddingPresets : providerPresets
}

export function presetFor(kind: ProviderKind, purpose?: AiProviderPurpose) {
  const presets = presetsFor(purpose)
  return presets.find((preset) => preset.kind === kind) ?? presets[0]
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

/**
 * Every chat service Ask may answer with, the one picked in Settings first:
 * on, with a model, and with its key saved unless it runs on this computer.
 */
export function usableAssistantProviders(ai: AiSettings): AiProviderConfig[] {
  const selected = usableAssistantProvider(ai)
  if (!selected) return []
  const others = ai.llmProviders.filter(
    (provider) =>
      provider.id !== selected.id &&
      provider.enabled &&
      provider.defaultModel.trim() &&
      (isLocalProvider(provider) || provider.apiKeySaved),
  )
  return [selected, ...others]
}

export function providerLabel(provider: AiProviderConfig) {
  return `${provider.name} · ${provider.defaultModel}`
}

export function newProviderId(
  kind: ProviderKind,
  purpose: AiProviderPurpose = 'llm',
) {
  const prefix = purpose === 'embedding' ? 'embed' : 'llm'
  return `${prefix}-${kind}-${crypto.randomUUID().slice(0, 8)}`
}

export interface ProviderDraft {
  kind: ProviderKind
  name: string
  baseUrl: string
  model: string
  apiKey: string
}

export function emptyDraft(
  kind: ProviderKind = 'ollama',
  purpose?: AiProviderPurpose,
): ProviderDraft {
  const preset = presetFor(kind, purpose)
  return { kind, name: '', baseUrl: preset.baseUrl, model: '', apiKey: '' }
}

const KIND_FOR_FORMAT: Record<AiRequestFormat, ProviderKind> = {
  ollama: 'ollama',
  'lm-studio': 'lm-studio',
  openai: 'openai',
  anthropic: 'anthropic',
  google: 'gemini',
}

/** A saved service as the form shows it for editing; its key stays where it is. */
export function draftFromProvider(provider: AiProviderConfig): ProviderDraft {
  return {
    kind: KIND_FOR_FORMAT[provider.requestFormat],
    name: provider.name,
    baseUrl: provider.baseUrl ?? '',
    model: provider.defaultModel,
    apiKey: '',
  }
}

export function draftToProvider(
  draft: ProviderDraft,
  id: string,
  label: string,
  purpose: AiProviderPurpose = 'llm',
): AiProviderConfig {
  const preset = presetFor(draft.kind, purpose)
  const model = draft.model.trim()
  return {
    id,
    name: draft.name.trim() || label,
    purpose,
    requestFormat: preset.requestFormat,
    enabled: true,
    baseUrl: draft.baseUrl.trim() || null,
    apiKeySaved: false,
    defaultModel: model,
    modelCatalog: model ? [model] : [],
  }
}

/** Hosted services need a key; anything on this computer does not. */
export function draftNeedsKey(
  draft: ProviderDraft,
  purpose?: AiProviderPurpose,
) {
  const preset = presetFor(draft.kind, purpose)
  return preset.needsKey && !isLocalUrl(draft.baseUrl)
}

/** Ready to save. `keySaved`: editing a service whose key is already stored. */
export function draftIsComplete(
  draft: ProviderDraft,
  {
    purpose,
    keySaved = false,
  }: { purpose?: AiProviderPurpose; keySaved?: boolean } = {},
) {
  if (!draft.model.trim()) return false
  if (purpose === 'embedding' && !draft.baseUrl.trim()) return false
  return (
    keySaved || !draftNeedsKey(draft, purpose) || draft.apiKey.trim().length > 0
  )
}

/** "8K", "256K", "1M": context lengths are powers of two, and read that way. */
export function contextSize(tokens: number) {
  if (tokens >= 1 << 20) return `${Math.round(tokens / (1 << 20))}M`
  return `${Math.max(1, Math.round(tokens / 1024))}K`
}
