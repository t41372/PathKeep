/**
 * What Settings → AI's semantic search actions do when this window's copy of
 * the settings is not what is saved: another window changed them since. Only
 * that order is tested here; the actions themselves are proven by
 * `tests/e2e/builtin-model.spec.ts`.
 *
 * Failure modes, written before the code:
 * - Cancel, in a window whose copy says semantic search is off (another
 *   window turned it on and started the download), stops the download but
 *   leaves semantic search on: the off it saves matches its stale base, so
 *   the backend's merge changes nothing.
 * - The in-app model's Download (or turning semantic search on), in a window
 *   that still shows the in-app model after another window chose an
 *   embeddings service, builds that service's index and so sends it page
 *   text; or turns semantic search on for it; or says nothing and leaves the
 *   old choice on screen.
 * - The same when the other window's choice is saved between this window's
 *   read and its save.
 * - A download given up as silent that then finishes never gets its index
 *   build, and the screen keeps saying it did not finish.
 * - The screen catching up after a stopped download reads the settings
 *   before the off is saved and shows them after it: on again.
 *
 * Stand-ins: the backend (its settings, merged on save the way `save_config`
 * merges them, and its commands), the event channel and the toasts. The hook,
 * the download module and the save path are real.
 *
 * Every wait is for a signal (`until`), never a count of ticks: a screen
 * shows a saved change only once React Query's notice reaches it, and a test
 * that reads the screen sooner passes or fails on whatever else happened to
 * render it.
 */
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import type * as I18n from '@/lib/i18n/context'
import type {
  AiSettings,
  AppConfig,
  AppSnapshot,
  ModelDownloadTransfer,
} from '@/lib/types'
import type * as SemanticIndex from './use-semantic-index'

const IN_APP = 'static-in-app'
const SERVICE = 'e2e-embeddings'
const A = 'a'.repeat(32)

/** The backend: what is saved, and what the commands were asked. */
const backend = vi.hoisted(() => ({
  config: null as unknown as AppConfig,
  modelDownloaded: false,
  saves: [] as AppConfig[],
  /** Runs just before a save is merged: another window's save landing first. */
  beforeSave: null as (() => void) | null,
  /** Holds the next settings read: see `holdNextRead`. */
  nextRead: null as HeldRead | null,
}))

interface HeldRead {
  /** The read was asked for, and what it read then. */
  taken: boolean
  saw: AppSnapshot | null
  /** It answered (with what it read when asked). */
  answered: boolean
  released: Promise<void>
  answer: () => void
}

const channel = vi.hoisted(() => ({
  listeners: new Set<(payload: unknown) => void>(),
}))

const commands = vi.hoisted(() => ({
  start: vi.fn<() => Promise<ModelDownloadTransfer>>(),
  cancel: vi.fn<(transferId: string) => Promise<boolean>>(),
  build: vi.fn<(request: unknown) => Promise<{ jobId: number }>>(),
}))

const toasts = vi.hoisted(() => ({ info: [] as string[] }))

/** `save_config`'s merge: a leaf the save left as its base had it keeps the saved value. */
function merge(saved: unknown, base: unknown, proposed: unknown): unknown {
  const isObject = (value: unknown): value is Record<string, unknown> =>
    typeof value === 'object' && value !== null && !Array.isArray(value)
  if (isObject(saved) && isObject(base) && isObject(proposed)) {
    for (const [key, value] of Object.entries(proposed)) {
      if (!(key in base)) saved[key] = value
      else if (key in saved) saved[key] = merge(saved[key], base[key], value)
      else if (JSON.stringify(value) !== JSON.stringify(base[key]))
        saved[key] = value
    }
    return saved
  }
  return JSON.stringify(proposed) === JSON.stringify(base) ? saved : proposed
}

function snapshotOf(config: AppConfig): AppSnapshot {
  return {
    config: structuredClone(config),
    aiStatus: {
      state: 'idle',
      indexedItems: 0,
      warningCode: null,
      staticEmbedding: {
        modelDownloaded: backend.modelDownloaded,
        modelRepo: 'minishlab/potion-multilingual-128M',
        dimensions: 256,
        modelSizeBytes: 0,
      },
    },
  } as unknown as AppSnapshot
}

vi.mock('@/lib/backend-client/app', () => ({
  appClient: {
    getSnapshot: async () => {
      const read = snapshotOf(backend.config)
      const hold = backend.nextRead
      backend.nextRead = null
      if (hold) {
        hold.taken = true
        hold.saw = read
        await hold.released
        hold.answered = true
      }
      return read
    },
    saveConfig: (config: AppConfig, base: AppConfig) => {
      backend.beforeSave?.()
      backend.saves.push(structuredClone(config))
      backend.config = merge(
        structuredClone(backend.config),
        base,
        config,
      ) as AppConfig
      return Promise.resolve(snapshotOf(backend.config))
    },
  },
}))

vi.mock('@/lib/backend-client/intelligence', () => ({
  intelligenceClient: {
    getQueueStatus: () => Promise.resolve({ semanticIndex: null }),
    getModelDownloadState: () => Promise.resolve(null),
    downloadStaticEmbeddingModel: commands.start,
    cancelStaticEmbeddingModelDownload: commands.cancel,
    buildIndex: commands.build,
  },
}))

vi.mock('@/lib/ipc/events', () => ({
  listenDesktopEvent: (_name: string, listener: (payload: unknown) => void) => {
    channel.listeners.add(listener)
    return Promise.resolve(() => {
      channel.listeners.delete(listener)
    })
  },
}))

vi.mock('sonner', () => ({
  toast: Object.assign(() => undefined, {
    info: (message: string) => {
      toasts.info.push(message)
    },
    success: () => undefined,
    warning: () => undefined,
    error: () => undefined,
  }),
}))

Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })

const i18n = {
  t: (key: string, values?: Record<string, unknown>) =>
    values ? `${key} ${JSON.stringify(values)}` : key,
  language: 'en',
  locale: 'en',
} as unknown as I18n.I18nValue

let hook: typeof SemanticIndex
let I18nContext: typeof I18n.I18nContext
const unmounts: (() => void)[] = []

beforeEach(async () => {
  // Fresh module state: the download a window follows outlives a screen on purpose.
  vi.resetModules()
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
  backend.modelDownloaded = false
  backend.saves = []
  backend.beforeSave = null
  backend.nextRead = null
  channel.listeners.clear()
  commands.start.mockReset()
  commands.cancel.mockReset()
  commands.build.mockReset()
  commands.build.mockResolvedValue({ jobId: 7 })
  toasts.info = []
  hook = await import('./use-semantic-index')
  ;({ I18nContext } = await import('@/lib/i18n/context'))
})

afterEach(() => {
  for (const unmount of unmounts.splice(0)) unmount()
  vi.useRealTimers()
})

function config(ai: Partial<AiSettings>): AppConfig {
  return {
    ai: {
      enabled: false,
      semanticIndexEnabled: false,
      embeddingProviderId: IN_APP,
      autoIndexAfterBackup: false,
      assistantEnabled: false,
      mcpEnabled: false,
      skillEnabled: false,
      embeddingProviders: [
        { id: SERVICE, name: 'E2E embeddings', kind: 'openai-compatible' },
      ],
      ...ai,
    },
  } as unknown as AppConfig
}

/**
 * One turn of the event loop inside `act`: React Query's batched notices
 * (zero-delay timers, faked here) go out, every settled promise runs on, and
 * React renders.
 */
async function turn() {
  await act(async () => {
    vi.advanceTimersByTime(0)
    await new Promise<void>((resolve) => setImmediate(resolve))
  })
}

/** Turns until `check` holds; `what` names the signal when it never comes. */
async function until(check: () => boolean, what: string) {
  for (let i = 0; i < 500; i++) {
    await turn()
    if (check()) return
  }
  throw new Error(`never happened: ${what}`)
}

/** The next settings read waits until `answer()`. */
function holdNextRead(): HeldRead {
  let answer!: () => void
  const released = new Promise<void>((resolve) => {
    answer = resolve
  })
  const hold = { taken: false, saw: null, answered: false, released, answer }
  backend.nextRead = hold
  return hold
}

let client: QueryClient

/** The settings the screen holds now, whether or not it has rendered them yet. */
const cached = () => client.getQueryData<AppSnapshot>(['snapshot'])!

/** Settings → AI in a window whose copy of the settings is `seen`. */
async function open(seen: AppConfig) {
  client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  client.setQueryData(['snapshot'], snapshotOf(seen))
  let index: ReturnType<typeof hook.useSemanticIndex> | null = null
  function Probe() {
    index = hook.useSemanticIndex()
    return null
  }
  const root = createRoot(document.createElement('div'))
  act(() =>
    root.render(
      <QueryClientProvider client={client}>
        <I18nContext.Provider value={i18n}>
          <Probe />
        </I18nContext.Provider>
      </QueryClientProvider>,
    ),
  )
  unmounts.push(() => act(() => root.unmount()))
  await until(() => index !== null, 'the screen rendered')
  return () => index!
}

async function send(event: object) {
  await act(async () => {
    for (const listener of [...channel.listeners])
      listener({ seq: 1, ...event })
    await Promise.resolve()
  })
}

it('Cancel in a window whose copy says off still turns semantic search off', async () => {
  // Another window turned it on and started the download; this one still has off.
  backend.config = config({ enabled: true, semanticIndexEnabled: true })
  const index = await open(config({}))
  await send({
    transferId: A,
    kind: 'fileStarted',
    file: 'model.safetensors',
    totalBytes: 1000,
  })
  expect(index().download.phase).toBe('downloading')
  commands.cancel.mockResolvedValue(true)
  await act(() => index().cancelDownload())
  expect(commands.cancel).toHaveBeenCalledWith(A)
  expect(backend.config.ai.semanticIndexEnabled).toBe(false)
  expect(backend.config.ai.enabled).toBe(false)
})

it('the in-app model’s Download, after another window chose a service, saves, downloads and builds nothing, and says so', async () => {
  backend.config = config({
    enabled: true,
    semanticIndexEnabled: true,
    embeddingProviderId: SERVICE,
  })
  const index = await open(
    config({ enabled: true, semanticIndexEnabled: true }),
  )
  expect(index().modelId).toBe(IN_APP)
  await act(() => index().enable(IN_APP))
  await until(
    () => index().modelId === SERVICE,
    'the screen shows the saved choice',
  )
  expect(backend.saves).toEqual([])
  expect(commands.start).not.toHaveBeenCalled()
  expect(commands.build).not.toHaveBeenCalled()
  expect(toasts.info).toEqual([
    'settingsAi.semantic.choiceChanged.service {"name":"E2E embeddings"}',
  ])
  expect(index().modelId).toBe(SERVICE)
})

it('turning semantic search on builds nothing when a service is chosen between the read and the save', async () => {
  backend.config = config({})
  const index = await open(config({}))
  backend.beforeSave = () => {
    backend.config.ai.embeddingProviderId = SERVICE
  }
  await act(() => index().enable(IN_APP))
  await until(
    () => index().modelId === SERVICE,
    'the screen shows the saved choice',
  )
  expect(commands.start).not.toHaveBeenCalled()
  expect(commands.build).not.toHaveBeenCalled()
  expect(toasts.info).toEqual([
    'settingsAi.semantic.choiceChanged.service {"name":"E2E embeddings"}',
  ])
})

it('turning semantic search on with a service shown and chosen builds its index, naming it', async () => {
  backend.config = config({ embeddingProviderId: SERVICE })
  const index = await open(config({ embeddingProviderId: SERVICE }))
  await act(() => index().enable(SERVICE))
  expect(commands.build).toHaveBeenCalledWith({
    providerId: SERVICE,
    fullRebuild: false,
    clearOnly: false,
    scope: 'incremental',
  })
  expect(toasts.info).toEqual([])
})

it('a download given up as silent that then finishes gets its build, and the stall is cleared', async () => {
  backend.config = config({})
  const index = await open(config({}))
  commands.start.mockResolvedValue({ transferId: A, seq: 1 })
  let enabling: Promise<void> | undefined
  act(() => {
    enabling = index().enable(IN_APP)
  })
  await until(() => commands.start.mock.calls.length === 1, 'the start')
  await send({
    transferId: A,
    kind: 'fileProgress',
    file: 'model.safetensors',
    downloadedBytes: 500,
    totalBytes: 1000,
  })
  // Shown only once the start's answer named the transfer.
  await until(
    () => index().download.downloadedBytes === 500,
    'the download followed',
  )
  await act(async () => {
    vi.advanceTimersByTime(120_000)
    await Promise.resolve()
  })
  await act(async () => {
    await enabling
  })
  await until(() => index().error !== null, 'the stall shown')
  expect(index().error).toBe('settingsAi.semantic.downloadStalled {"count":2}')
  expect(commands.build).not.toHaveBeenCalled()

  backend.modelDownloaded = true
  await send({ transferId: A, kind: 'done' })
  await until(
    () => index().modelDownloaded && index().error === null,
    'the screen caught up with the late end',
  )
  expect(commands.build).toHaveBeenCalledWith({
    providerId: IN_APP,
    transferId: A,
    fullRebuild: false,
    clearOnly: false,
    scope: 'incremental',
  })
  expect(index().error).toBeNull()
  expect(index().modelDownloaded).toBe(true)
  expect(index().download.phase).toBe('ready')
})

it('the screen catching up after a stopped download does not put back settings older than the off', async () => {
  backend.config = config({})
  const index = await open(config({}))
  commands.start.mockResolvedValue({ transferId: A, seq: 1 })
  act(() => {
    void index().enable(IN_APP)
  })
  await until(
    () => commands.start.mock.calls.length === 1 && index().enabled,
    'semantic search on and the download started',
  )
  // The download stops. The screen catches up: it reads the settings, still
  // on, and the read waits...
  const read = holdNextRead()
  await send({ transferId: A, kind: 'cancelled' })
  await until(() => read.taken, 'the catch-up read')
  expect(read.saw?.config.ai.semanticIndexEnabled).toBe(true)
  // ...while the off is saved and shown.
  await act(() => index().disable())
  expect(backend.config.ai.semanticIndexEnabled).toBe(false)
  await until(() => !index().enabled, 'the off shown')
  // The read answers now, with the settings from before the off.
  read.answer()
  await until(() => read.answered, 'the catch-up read answered')
  expect(cached().config.ai.semanticIndexEnabled).toBe(false)
  await turn()
  expect(index().enabled).toBe(false)
})
