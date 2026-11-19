/**
 * What `builtin-model.spec` checks besides the screen: the built-in model's
 * files on disk, every download event the backend sends, the index jobs, and
 * the one page the test model has words for.
 *
 * The test model (`builtin-model-source.ts`) knows a few English words
 * (alpha, beta, gamma, browser, history, model, search, local, archive, page,
 * test); every other word means nothing to it. The spec adds a page titled
 * with them, so searching by meaning has something to find. `wipe.spec`
 * counts that visit.
 */
import { existsSync, readFileSync, readdirSync } from 'node:fs'
import path from 'node:path'
import { resolveDesktopBridgeEnv } from '../../../scripts/pathkeep-dev-desktop-bridge.mjs'
import type {
  AiQueueStatus,
  ModelDownloadProgressEvent,
} from '../../../src/lib/types'
import { backend, fixture, type FixtureVisit } from './fixture'

/** A page whose title and address are words the test model knows. */
export const MODEL_WORDS_PAGE = {
  url: 'https://glossary.example.org/alpha-gamma-archive',
  title: 'Alpha gamma archive notes',
}

/** What to search for to find `MODEL_WORDS_PAGE` by meaning. */
export const MODEL_WORDS = 'alpha gamma'

/** A visit a minute ago to `MODEL_WORDS_PAGE`. */
export function modelWordsVisit(): FixtureVisit {
  return {
    ...MODEL_WORDS_PAGE,
    at: Date.now() - 60_000,
    typed: true,
    duration: 30_000,
  }
}

/** Visits `builtin-model.spec` adds to Chrome. */
export function builtinModelVisitCount() {
  return 1
}

/** The built-in model's folder, as the backend names it (`model_dir_for_repo`). */
export function modelFolder() {
  return path.join(
    fixture().dirs.projectRoot,
    'models',
    'minishlab%2Fpotion-multilingual-128M',
  )
}

const MODEL_FILES = ['config.json', 'tokenizer.json', 'model.safetensors']

/**
 * What the model folder holds: the generation `current` names, every
 * generation folder (a download writes into one before it is published), and
 * model files kept flat in the folder itself, the layout before generations.
 */
export function modelBundles() {
  const folder = modelFolder()
  const entries = existsSync(folder)
    ? readdirSync(folder, { withFileTypes: true })
    : []
  return {
    current: existsSync(path.join(folder, 'current'))
      ? readFileSync(path.join(folder, 'current'), 'utf8')
      : null,
    generations: entries
      .filter(
        (entry) => entry.isDirectory() && /^gen-[0-9a-f]{32}$/.test(entry.name),
      )
      .map((entry) => entry.name)
      .sort(),
    flatFiles: entries
      .filter((entry) => entry.isFile() && MODEL_FILES.includes(entry.name))
      .map((entry) => entry.name)
      .sort(),
  }
}

export const queue = () => backend<AiQueueStatus>('load_ai_queue_status')

/** The newest job's ID now, to tell the jobs started after it. */
export async function newestJob() {
  return Math.max(0, ...(await queue()).recentJobs.map((job) => job.id))
}

/** Index builds queued after job `id`, oldest first. */
export async function indexBuildsAfter(id: number) {
  return (await queue()).recentJobs
    .filter((job) => job.jobType === 'index-build' && job.id > id)
    .sort((a, b) => a.id - b.id)
}

/** The backend's model download events (the bridge's `/events`), as they come. */
export interface DownloadEvents {
  all: ModelDownloadProgressEvent[]
  /** One transfer's events, in order. */
  of(transferId: string): ModelDownloadProgressEvent[]
  /** How that transfer ended: one terminal kind once it is over, none before. */
  ends(transferId: string): string[]
  close(): void
}

const CHANNEL = 'pathkeep://model-download-progress'

/**
 * Listens to the backend's events from now on, as a window does. A restarted
 * app is a new stream: listen again after a restart.
 */
export async function recordModelDownloads(): Promise<DownloadEvents> {
  const { devIpcUrl } = resolveDesktopBridgeEnv(process.env)
  const stop = new AbortController()
  const response = await fetch(`${devIpcUrl}/events`, {
    headers: { accept: 'text/event-stream' },
    signal: stop.signal,
  })
  if (!response.ok || !response.body)
    throw new Error(`The bridge's event stream answered ${response.status}.`)
  const all: ModelDownloadProgressEvent[] = []
  const reader = response.body.pipeThrough(new TextDecoderStream()).getReader()
  void (async () => {
    let buffer = ''
    try {
      for (;;) {
        const { value, done } = await reader.read()
        if (done) return
        buffer += value
        let end = buffer.indexOf('\n\n')
        while (end >= 0) {
          const block = buffer.slice(0, end)
          buffer = buffer.slice(end + 2)
          end = buffer.indexOf('\n\n')
          let name = ''
          let data = ''
          for (const line of block.split('\n')) {
            if (line.startsWith('event:')) name = line.slice(6).trim()
            else if (line.startsWith('data:')) data += line.slice(5).trim()
          }
          if (name === CHANNEL && data)
            all.push(JSON.parse(data) as ModelDownloadProgressEvent)
        }
      }
    } catch {
      // Closed, or the app went away.
    }
  })()
  const of = (transferId: string) =>
    all.filter((event) => event.transferId === transferId)
  return {
    all,
    of,
    ends: (transferId) =>
      of(transferId)
        .map((event) => event.kind)
        .filter(
          (kind) => kind === 'done' || kind === 'error' || kind === 'cancelled',
        ),
    close: () => stop.abort(),
  }
}
