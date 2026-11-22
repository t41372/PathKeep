/**
 * Prototype-only: the fake AI settings behind Settings › AI, standing in for
 * the app's snapshot, `useSemanticIndex`, `useAiProviders` and the MCP
 * queries. One context so the rows act on the same state; the prototype
 * switchers at the top of the section set it directly.
 */
import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from 'react'
import type { ModelListCase } from './model-field'
import { STATIC_MODEL_APPROX_BYTES, type ProviderConfig } from './providers'

export const IN_APP = 'built-in-static'
export const IN_APP_MODEL = 'potion-multilingual-128M'
/** Visits in the fake archive (the browsing profiles of the prototype's data). */
export const TOTAL_VISITS = 756_082

/**
 * Where semantic search stands. Each is a branch of the app's
 * `SemanticStatus` / `IndexState`.
 */
export type SemanticPhase =
  | 'off'
  | 'downloading'
  | 'downloadFailed'
  | 'modelMissing'
  | 'setupRunning'
  | 'needsBuild'
  | 'unfinished'
  | 'unfinishedOlderText'
  | 'running'
  | 'queued'
  | 'pausedUser'
  | 'pausedService'
  | 'pausedQueue'
  | 'ready'
  | 'readyEmpty'
  | 'olderText'
  | 'failed'
  | 'degraded'

export type TestCase =
  | 'ok'
  | 'secret-missing'
  | 'network-error'
  | 'rate-limited'
  | 'bad-model'
  | 'bad-endpoint'
  | 'unsupported-capability'
  | 'provider-disabled'
  | 'other'
  | 'thrown'

export type CheckCase = 'found' | 'missing' | 'unreachable'
export type ProviderCheck = { kind: 'checking' } | { kind: CheckCase }
export type TestReport = { ok: true; ms: number } | { ok: false; code: TestCase }
export type McpCase = 'requests' | 'empty' | 'loading' | 'failed'

const llmSeed: ProviderConfig[] = [
  { id: 'llm-ollama', kind: 'ollama', name: 'Ollama', baseUrl: 'http://localhost:11434/v1', apiKeySaved: false, defaultModel: 'llama3.2:3b' },
  { id: 'llm-openai', kind: 'openai', name: 'OpenAI', baseUrl: null, apiKeySaved: true, defaultModel: 'gpt-4o-mini' },
  { id: 'llm-anthropic', kind: 'anthropic', name: 'Anthropic', baseUrl: null, apiKeySaved: false, defaultModel: 'claude-3-5-haiku-latest' },
]

const embeddingSeed: ProviderConfig[] = [
  { id: 'embed-lm-studio', kind: 'lm-studio', name: 'LM Studio', baseUrl: 'http://localhost:1234/v1', apiKeySaved: false, defaultModel: 'text-embedding-nomic-embed-text-v1.5' },
  { id: 'embed-openai', kind: 'openai', name: 'OpenAI', baseUrl: 'https://api.openai.com/v1', apiKeySaved: true, defaultModel: 'text-embedding-3-small' },
]

export interface AiState {
  phase: SemanticPhase
  /** The in-app model's files are on disk. */
  modelDownloaded: boolean
  downloaded: number
  /** IN_APP or an embedding service id. */
  modelId: string
  /** The complete index search reads while another is built; null when the chosen one is it. */
  servingOther: string | null
  /** The chosen model's index is complete (a rebuild writes its replacement beside it). */
  chosenComplete: boolean
  done: number
  autoUpdate: boolean
  embeddingServices: ProviderConfig[]
  llmServices: ProviderConfig[]
  llmSelected: string | null
  mcpOn: boolean
  mcpCase: McpCase
  testCase: TestCase
  checkCase: CheckCase
  listCase: ModelListCase
  reports: Record<string, TestReport>
  checks: Record<string, ProviderCheck>
  testing: string | null
}

const initial: AiState = {
  phase: 'ready',
  modelDownloaded: true,
  downloaded: 0,
  modelId: IN_APP,
  servingOther: null,
  chosenComplete: true,
  done: 0,
  autoUpdate: true,
  embeddingServices: embeddingSeed,
  llmServices: llmSeed,
  llmSelected: 'llm-ollama',
  mcpOn: true,
  mcpCase: 'requests',
  testCase: 'ok',
  checkCase: 'found',
  listCase: 'found',
  reports: {},
  checks: {},
  testing: null,
}

interface Ctx extends AiState {
  set: (patch: Partial<AiState> | ((s: AiState) => Partial<AiState>)) => void
  enabled: boolean
  service: ProviderConfig | null
  /** The chosen model's short name. */
  modelName: string
  test: (id: string) => void
  check: (id: string) => void
}

const AiContext = createContext<Ctx | null>(null)

export function AiProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState(initial)
  const set: Ctx['set'] = (patch) => setState((s) => ({ ...s, ...(typeof patch === 'function' ? patch(s) : patch) }))

  // The download and the index build move on their own, as the app's do.
  useEffect(() => {
    if (state.phase !== 'downloading' && state.phase !== 'running') return
    const timer = window.setInterval(() => {
      setState((s) => {
        if (s.phase === 'downloading') {
          const downloaded = Math.min(STATIC_MODEL_APPROX_BYTES, s.downloaded + STATIC_MODEL_APPROX_BYTES / 16)
          return downloaded >= STATIC_MODEL_APPROX_BYTES ? { ...s, downloaded, modelDownloaded: true, phase: 'running', done: 0, chosenComplete: false } : { ...s, downloaded }
        }
        if (s.phase === 'running') {
          const done = Math.min(TOTAL_VISITS, s.done + Math.round(TOTAL_VISITS / 70))
          return done >= TOTAL_VISITS ? { ...s, done, phase: 'ready', servingOther: null, chosenComplete: true } : { ...s, done }
        }
        return s
      })
    }, 450)
    return () => window.clearInterval(timer)
  }, [state.phase])

  const value = useMemo<Ctx>(() => {
    const service = state.modelId === IN_APP ? null : (state.embeddingServices.find((p) => p.id === state.modelId) ?? null)
    return {
      ...state,
      set,
      enabled: state.phase !== 'off' && state.phase !== 'downloading',
      service,
      modelName: service ? service.defaultModel : IN_APP_MODEL,
      test: (id) => {
        setState((s) => ({ ...s, testing: id }))
        window.setTimeout(() => {
          setState((s) => ({
            ...s,
            testing: null,
            reports: { ...s.reports, [id]: s.testCase === 'ok' ? { ok: true, ms: 412 } : { ok: false, code: s.testCase } },
          }))
        }, 900)
      },
      check: (id) => {
        setState((s) => {
          const reports = { ...s.reports }
          delete reports[id]
          return { ...s, reports, checks: { ...s.checks, [id]: { kind: 'checking' } } }
        })
        window.setTimeout(() => setState((s) => ({ ...s, checks: { ...s.checks, [id]: { kind: s.checkCase } } })), 1000)
      },
    }
    // `set` is stable in behaviour; the memo follows the state.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state])

  return <AiContext.Provider value={value}>{children}</AiContext.Provider>
}

export function useAi() {
  const ctx = useContext(AiContext)
  if (!ctx) throw new Error('useAi outside AiProvider')
  return ctx
}
