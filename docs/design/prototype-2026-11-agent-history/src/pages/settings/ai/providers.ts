/**
 * The AI service kinds and helpers, copied from src/features/ai-setup/providers.ts
 * so the prototype's forms offer exactly the app's choices. Fake services only.
 */

export type ProviderKind = 'openai' | 'anthropic' | 'gemini' | 'ollama' | 'lm-studio'
export type Purpose = 'llm' | 'embedding'

export interface ProviderPreset {
  kind: ProviderKind
  baseUrl: string
  baseUrlHint: string
  modelHint: string
  needsKey: boolean
}

export const providerPresets: readonly ProviderPreset[] = [
  { kind: 'ollama', baseUrl: 'http://localhost:11434/v1', baseUrlHint: '', modelHint: 'llama3.2:3b', needsKey: false },
  { kind: 'lm-studio', baseUrl: 'http://localhost:1234/v1', baseUrlHint: '', modelHint: '', needsKey: false },
  { kind: 'openai', baseUrl: '', baseUrlHint: 'https://api.openai.com/v1', modelHint: 'gpt-4o-mini', needsKey: true },
  { kind: 'anthropic', baseUrl: '', baseUrlHint: 'https://api.anthropic.com', modelHint: 'claude-3-5-haiku-latest', needsKey: true },
  { kind: 'gemini', baseUrl: '', baseUrlHint: 'https://generativelanguage.googleapis.com', modelHint: 'gemini-2.0-flash', needsKey: true },
]

export const embeddingPresets: readonly ProviderPreset[] = [
  { kind: 'ollama', baseUrl: 'http://localhost:11434/v1', baseUrlHint: '', modelHint: 'nomic-embed-text', needsKey: false },
  { kind: 'lm-studio', baseUrl: 'http://localhost:1234/v1', baseUrlHint: '', modelHint: '', needsKey: false },
  { kind: 'openai', baseUrl: 'https://api.openai.com/v1', baseUrlHint: '', modelHint: 'text-embedding-3-small', needsKey: true },
]

/** settingsAi.provider.kinds */
export const kindLabels: Record<ProviderKind, string> = {
  ollama: 'Ollama（本機）',
  'lm-studio': 'LM Studio（本機）',
  openai: 'OpenAI 或相容服務',
  anthropic: 'Anthropic',
  gemini: 'Google Gemini',
}

/** settingsAi.provider.defaultName: the name a service gets when the field is left empty. */
export const defaultNames: Record<ProviderKind, string> = {
  ollama: 'Ollama',
  'lm-studio': 'LM Studio',
  openai: 'OpenAI',
  anthropic: 'Anthropic',
  gemini: 'Gemini',
}

export function presetsFor(purpose: Purpose = 'llm') {
  return purpose === 'embedding' ? embeddingPresets : providerPresets
}

export function presetFor(kind: ProviderKind, purpose?: Purpose) {
  const presets = presetsFor(purpose)
  return presets.find((preset) => preset.kind === kind) ?? presets[0]
}

const LOCAL_HOSTS = new Set(['localhost', '127.0.0.1', '[::1]', '0.0.0.0'])

export function isLocalUrl(url: string | null | undefined) {
  try {
    return LOCAL_HOSTS.has(new URL(url ?? '').hostname)
  } catch {
    return false
  }
}

export interface ProviderConfig {
  id: string
  kind: ProviderKind
  name: string
  baseUrl: string | null
  apiKeySaved: boolean
  defaultModel: string
}

export function isLocalProvider(provider: ProviderConfig) {
  if (provider.kind === 'ollama' || provider.kind === 'lm-studio') return true
  return isLocalUrl(provider.baseUrl)
}

export function providerLabel(provider: ProviderConfig) {
  return `${provider.name} · ${provider.defaultModel}`
}

export interface ProviderDraft {
  kind: ProviderKind
  name: string
  baseUrl: string
  model: string
  apiKey: string
}

export function emptyDraft(kind: ProviderKind = 'ollama', purpose?: Purpose): ProviderDraft {
  return { kind, name: '', baseUrl: presetFor(kind, purpose).baseUrl, model: '', apiKey: '' }
}

export function draftFromProvider(provider: ProviderConfig): ProviderDraft {
  return { kind: provider.kind, name: provider.name, baseUrl: provider.baseUrl ?? '', model: provider.defaultModel, apiKey: '' }
}

export function draftToProvider(draft: ProviderDraft, id: string, purpose: Purpose = 'llm'): ProviderConfig {
  return {
    id,
    kind: draft.kind,
    name: draft.name.trim() || defaultNames[draft.kind],
    baseUrl: draft.baseUrl.trim() || null,
    apiKeySaved: draft.apiKey.trim().length > 0 || !presetFor(draft.kind, purpose).needsKey,
    defaultModel: draft.model.trim(),
  }
}

export function draftNeedsKey(draft: ProviderDraft, purpose?: Purpose) {
  return presetFor(draft.kind, purpose).needsKey && !isLocalUrl(draft.baseUrl)
}

export function draftIsComplete(draft: ProviderDraft, { purpose, keySaved = false }: { purpose?: Purpose; keySaved?: boolean } = {}) {
  if (!draft.model.trim()) return false
  if (purpose === 'embedding' && !draft.baseUrl.trim()) return false
  return keySaved || !draftNeedsKey(draft, purpose) || draft.apiKey.trim().length > 0
}

/** "8K", "256K", "1M". */
export function contextSize(tokens: number) {
  if (tokens >= 1 << 20) return `${Math.round(tokens / (1 << 20))}M`
  return `${Math.max(1, Math.round(tokens / 1024))}K`
}

/** The app's byte formatter (1024-based, like useFormat().bytes), so sizes read as the app prints them. */
export function appBytes(value: number) {
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let size = value
  let unit = 0
  while (size >= 1024 && unit < units.length - 1) {
    size /= 1024
    unit += 1
  }
  const digits = size >= 100 || unit === 0 ? 0 : size >= 10 ? 1 : 2
  return `${new Intl.NumberFormat('zh-TW', { maximumFractionDigits: digits }).format(size)} ${units[unit]}`
}

/** potion-multilingual-128M: a 512 MB matrix plus an 18.6 MB tokenizer. */
export const STATIC_MODEL_APPROX_BYTES = 531 * 1024 * 1024
