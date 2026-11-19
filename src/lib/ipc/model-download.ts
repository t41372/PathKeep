/**
 * The in-app embedding model's download as this window sees it.
 *
 * Responsibilities:
 * - An attempt: subscribe to `pathkeep://model-download-progress`, start the
 *   download (or join the one running), and end exactly once: on its
 *   transfer's own terminal event, on a refused start (no transfer exists, so
 *   no terminal event will come), or when nothing is heard for
 *   `MODEL_DOWNLOAD_IDLE_TIMEOUT_MS`. Waiting is a sign of life. A start reply
 *   that arrives after its attempt ended changes nothing.
 * - What the screens show (`useModelDownloadProgress`): one transfer, the one
 *   this window follows. A transfer another window started is followed from
 *   its events. Once a newer transfer is followed, the older one's late
 *   events, its `cancelled` included, no longer move the bar.
 * - Cancel, by the followed transfer's ID.
 *
 * Not responsible for:
 * - Whether the saved settings still want the model (the caller's
 *   `stillWanted`), or the index build after it (`semantic-setup.ts`).
 * - Copy or rendering.
 *
 * Module state on purpose: it is the only state that outlives a remount, and
 * every screen in this window must show the same download.
 */

import { useEffect, useMemo, useSyncExternalStore } from 'react'
import { describeError } from '../errors'
import { intelligenceClient } from '../backend-client/intelligence'
import type { ModelDownloadProgressEvent, ModelDownloadWait } from '../types'
import { listenDesktopEvent } from './events'

/** Tauri event channel for in-app model download progress. Must match the Rust literal. */
export const MODEL_DOWNLOAD_PROGRESS_EVENT =
  'pathkeep://model-download-progress'

/**
 * How long an attempt may go without hearing anything before it is given up:
 * a dead event bridge or a dead download thread must not leave the screen
 * downloading forever. Every event resets it, `waiting` (sent every 20 s)
 * included, so a slow or waiting download is never given up.
 */
export const MODEL_DOWNLOAD_IDLE_TIMEOUT_MS = 120_000

/** How an attempt ended. */
export type ModelDownloadOutcome =
  | { kind: 'done'; transferId: string }
  /** Something the user asked for stopped it: Cancel, or a saved setting that no longer wants it. */
  | { kind: 'cancelled' }
  /** The transfer's own `error`. */
  | { kind: 'failed'; message: string }
  /** The start was refused while the saved settings still want the model; no transfer exists. */
  | { kind: 'refused'; error: unknown }
  /** Nothing was heard for `MODEL_DOWNLOAD_IDLE_TIMEOUT_MS`. */
  | { kind: 'silent' }

/** Why the download the screen shows did not finish. */
export type ModelDownloadFailure =
  { kind: 'error'; message: string } | { kind: 'silent' }

/**
 * What a screen shows of the download.
 *
 * - `idle`: no download (none started, or it was cancelled).
 * - `downloading`: started; bytes as far as known. `totalBytes` 0 means not
 *   known yet, and the screen shows an indeterminate bar, not a made-up percent.
 * - `waiting`: started but held up (`waitingFor`); the bytes stay as they were.
 * - `ready`: the model is here, from the last `done` or from the snapshot.
 * - `failed`: it did not finish (`failure`); never a cancellation.
 */
export interface ModelDownloadProgress {
  phase: 'idle' | 'downloading' | 'waiting' | 'ready' | 'failed'
  waitingFor: ModelDownloadWait | null
  downloadedBytes: number
  totalBytes: number
  currentFile: string | null
  failure: ModelDownloadFailure | null
}

interface Attempt {
  /** Known from the start's answer, or from the first event of a transfer another window started. */
  transferId: string | null
  /** Started here: settles the caller's promise. A transfer only seen in events has none. */
  settle: ((outcome: ModelDownloadOutcome) => void) | null
  /** The transfer the start named, or null when it was refused or the attempt had ended first. */
  answered: Promise<string | null>
  /** Events heard before the start answered; replayed for the transfer it names. */
  early: ModelDownloadProgressEvent[]
  ended: boolean
  watchdog: ReturnType<typeof setTimeout> | null
  /** Lets go of the channel this attempt keeps open while it runs. */
  release: (() => void) | null
}

const IDLE: ModelDownloadProgress = {
  phase: 'idle',
  waitingFor: null,
  downloadedBytes: 0,
  totalBytes: 0,
  currentFile: null,
  failure: null,
}

/** Attempts that have not ended, shown or not: a setup waits on its own transfer either way. */
const live = new Set<Attempt>()
/** The attempt the screens show. */
let shown: Attempt | null = null
/** Transfers this window stopped following; their late events change nothing. */
const leftBehind = new Set<string>()

let view = IDLE
/** The transfer whose bytes `view` holds, and its files' bytes. */
let viewTransfer: string | null = null
const files = new Map<string, { done: number; total: number }>()
const viewers = new Set<() => void>()

function publish(next: Partial<ModelDownloadProgress>) {
  view = { ...view, ...next }
  for (const viewer of viewers) viewer()
}

function resetView(
  phase: ModelDownloadProgress['phase'],
  transferId: string | null,
) {
  files.clear()
  viewTransfer = transferId
  view = IDLE
  publish({ phase })
}

function byteTotals() {
  let downloadedBytes = 0
  let totalBytes = 0
  for (const file of files.values()) {
    downloadedBytes += file.done
    totalBytes += file.total
  }
  return { downloadedBytes, totalBytes }
}

/** Moves the bar for an event of the transfer the screens show. */
function showEvent(event: ModelDownloadProgressEvent) {
  switch (event.kind) {
    case 'waiting':
      publish({ phase: 'waiting', waitingFor: event.reason })
      return
    case 'fileStarted':
      files.set(event.file, { done: 0, total: event.totalBytes })
      publish({
        phase: 'downloading',
        waitingFor: null,
        currentFile: event.file.split('/').pop() ?? event.file,
        failure: null,
        ...byteTotals(),
      })
      return
    case 'fileProgress': {
      const known = files.get(event.file)?.total ?? 0
      files.set(event.file, {
        done: event.downloadedBytes,
        total: event.totalBytes > 0 ? event.totalBytes : known,
      })
      publish({ phase: 'downloading', waitingFor: null, ...byteTotals() })
      return
    }
    case 'fileFinished': {
      const file = files.get(event.file)
      if (file && file.total > 0) file.done = file.total
      publish({ currentFile: null, ...byteTotals() })
      return
    }
    default:
      // Terminal events end the attempt; `showOutcome` says how.
      return
  }
}

function showOutcome(outcome: ModelDownloadOutcome) {
  const settled = { waitingFor: null, currentFile: null }
  switch (outcome.kind) {
    case 'done':
      publish({ ...settled, phase: 'ready', failure: null })
      return
    case 'cancelled':
      resetView('idle', null)
      return
    case 'failed':
      publish({
        ...settled,
        phase: 'failed',
        failure: { kind: 'error', message: outcome.message },
      })
      return
    case 'refused':
      publish({
        ...settled,
        phase: 'failed',
        failure: { kind: 'error', message: describeError(outcome.error) },
      })
      return
    case 'silent':
      publish({ ...settled, phase: 'failed', failure: { kind: 'silent' } })
      return
  }
}

function newAttempt(
  transferId: string | null,
  settle: Attempt['settle'],
  answered: Promise<string | null>,
): Attempt {
  const attempt: Attempt = {
    transferId,
    settle,
    answered,
    early: [],
    ended: false,
    watchdog: null,
    release: null,
  }
  live.add(attempt)
  armWatchdog(attempt)
  return attempt
}

function armWatchdog(attempt: Attempt) {
  if (attempt.watchdog !== null) clearTimeout(attempt.watchdog)
  attempt.watchdog = setTimeout(() => {
    attempt.watchdog = null
    // Given up here only says something when this window asked for the download; a transfer
    // only seen in events may well be fine, so the screen just stops showing it.
    if (attempt.settle) end(attempt, { kind: 'silent' })
    else stopFollowing(attempt)
  }, MODEL_DOWNLOAD_IDLE_TIMEOUT_MS)
}

function close(attempt: Attempt) {
  attempt.ended = true
  if (attempt.watchdog !== null) clearTimeout(attempt.watchdog)
  attempt.watchdog = null
  live.delete(attempt)
  if (attempt.transferId !== null) leftBehind.add(attempt.transferId)
  attempt.release?.()
  attempt.release = null
}

/** Ends an attempt once: the screens show how (if they show it) and its caller hears it. */
function end(attempt: Attempt, outcome: ModelDownloadOutcome) {
  if (attempt.ended) return
  close(attempt)
  if (attempt === shown) showOutcome(outcome)
  attempt.settle?.(outcome)
}

/**
 * Stops following a transfer this window only saw in events, saying nothing about how it went.
 * `replaced`: a newer transfer is shown right after, so the bar is not emptied first.
 */
function stopFollowing(attempt: Attempt, replaced = false) {
  if (attempt.ended) return
  close(attempt)
  if (attempt === shown && !replaced) resetView('idle', null)
}

/**
 * The screens stop showing `attempt` because a newer transfer is followed. One this window
 * started stays live: its caller still learns how its own transfer ended.
 */
function leave(attempt: Attempt) {
  if (attempt.transferId !== null) leftBehind.add(attempt.transferId)
  if (!attempt.settle) stopFollowing(attempt, true)
}

/** An event of `attempt`'s own transfer. */
function hear(attempt: Attempt, event: ModelDownloadProgressEvent) {
  if (attempt.ended) return
  armWatchdog(attempt)
  if (attempt === shown) showEvent(event)
  switch (event.kind) {
    case 'done':
      end(attempt, { kind: 'done', transferId: event.transferId })
      return
    case 'cancelled':
      end(attempt, { kind: 'cancelled' })
      return
    case 'error':
      end(attempt, { kind: 'failed', message: event.message })
      return
    default:
      return
  }
}

const isTerminal = (event: ModelDownloadProgressEvent) =>
  event.kind === 'done' || event.kind === 'cancelled' || event.kind === 'error'

/** A transfer's first sign: a newer transfer than one already followed starts with one of these. */
const isBeginning = (event: ModelDownloadProgressEvent) =>
  event.kind === 'waiting' || event.kind === 'fileStarted'

function receive(event: ModelDownloadProgressEvent) {
  let owned = false
  for (const attempt of [...live]) {
    if (attempt.transferId === null) {
      // Whose it is is known once the start answers. Hearing anything shows the channel works.
      attempt.early.push(event)
      armWatchdog(attempt)
    } else if (attempt.transferId === event.transferId) {
      owned = true
      hear(attempt, event)
    }
  }
  if (owned || leftBehind.has(event.transferId)) return
  const following = shown && !shown.ended ? shown : null
  // The start this window sent decides which transfer it shows, when it answers.
  if (following?.transferId === null) return
  // The end of a transfer never followed says nothing about the one shown.
  if (isTerminal(event)) return
  // Another transfer while one is followed: only a newer one starting replaces it. A late
  // event of an older one this window never saw does not.
  if (following && !isBeginning(event)) return
  if (following) leave(following)
  const attempt = newAttempt(
    event.transferId,
    null,
    Promise.resolve(event.transferId),
  )
  shown = attempt
  resetView('downloading', event.transferId)
  hear(attempt, event)
}

/** The start answered: `attempt` follows `transferId`. */
function follow(attempt: Attempt, transferId: string) {
  attempt.transferId = transferId
  // A start joins a transfer this window may have stopped following; it follows it again.
  leftBehind.delete(transferId)
  const early = attempt.early.splice(0)
  if (attempt === shown) {
    for (const other of [...live]) {
      if (other !== attempt && other.transferId !== transferId) leave(other)
    }
    if (viewTransfer !== transferId) resetView('downloading', transferId)
  }
  for (const event of early) {
    if (event.transferId === transferId) hear(attempt, event)
    // Another transfer's events from before the answer: an older one, done with.
    else if (![...live].some((other) => other.transferId === event.transferId))
      leftBehind.add(event.transferId)
  }
}

// ─── The channel ──────────────────────────────────────────────────────────────

let channel: { users: number; ready: Promise<() => void> } | null = null

/**
 * One subscription for the window, open while a screen shows the download or an attempt runs.
 * Resolves once it listens; the returned function lets go of it.
 */
async function openChannel(): Promise<() => void> {
  const current = (channel ??= {
    users: 0,
    ready: listenDesktopEvent<ModelDownloadProgressEvent | null>(
      MODEL_DOWNLOAD_PROGRESS_EVENT,
      (payload) => {
        if (payload) receive(payload)
      },
    ),
  })
  current.users += 1
  let open = true
  await current.ready
  return () => {
    if (!open) return
    open = false
    current.users -= 1
    if (current.users > 0) return
    if (channel === current) channel = null
    void current.ready.then((stop) => stop())
    // Nobody hears how the transfers seen in events end any more: stop showing them.
    for (const attempt of [...live]) if (!attempt.settle) stopFollowing(attempt)
  }
}

// ─── Starting and cancelling ──────────────────────────────────────────────────

/**
 * Starts the built-in model's download, or joins the one running, and resolves with how it
 * ended. The screens show it from this call on. `stillWanted` reads the saved settings when a
 * start is refused: refused because they no longer want the model, it is a cancellation.
 */
export function downloadModel(
  stillWanted: () => Promise<boolean>,
): Promise<ModelDownloadOutcome> {
  let settle!: (outcome: ModelDownloadOutcome) => void
  const outcome = new Promise<ModelDownloadOutcome>((resolve) => {
    settle = resolve
  })
  let answer!: (transferId: string | null) => void
  const answered = new Promise<string | null>((resolve) => {
    answer = resolve
  })
  const previous = shown && !shown.ended ? shown : null
  const attempt = newAttempt(null, settle, answered)
  shown = attempt
  // A start while a download runs joins it: keep its bar until the answer says which it is.
  if (!previous) resetView('downloading', null)
  void begin(attempt, answer, stillWanted)
  return outcome
}

async function begin(
  attempt: Attempt,
  answer: (transferId: string | null) => void,
  stillWanted: () => Promise<boolean>,
) {
  // Listen before starting: even a model already here can finish before the answer arrives.
  const release = await openChannel()
  if (attempt.ended) {
    answer(null)
    release()
    return
  }
  // Kept open until the attempt ends.
  attempt.release = release
  await intelligenceClient.downloadStaticEmbeddingModel().then(
    ({ transferId }) => {
      answer(attempt.ended ? null : transferId)
      // Given up already (the idle window passed): the attempt is over, and so is this
      // transfer for this window. Only a new start follows it again.
      if (attempt.ended) leftBehind.add(transferId)
      else follow(attempt, transferId)
    },
    async (error: unknown) => {
      answer(null)
      attempt.early.length = 0
      if (attempt.ended) return
      const wanted = await stillWanted().catch(() => true)
      end(attempt, wanted ? { kind: 'refused', error } : { kind: 'cancelled' })
    },
  )
}

/**
 * Cancels the download the screens show. Before its start has answered, waits for the answer, so
 * a quick click still names the right transfer. Answers whether the backend took the cancel.
 */
export async function cancelModelDownload(): Promise<boolean> {
  const attempt = shown
  if (!attempt || attempt.ended) return false
  const transferId = attempt.transferId ?? (await attempt.answered)
  if (transferId === null) return false
  return intelligenceClient.cancelStaticEmbeddingModelDownload(transferId)
}

// ─── useModelDownloadProgress ─────────────────────────────────────────────────

function watch(viewer: () => void) {
  viewers.add(viewer)
  return () => {
    viewers.delete(viewer)
  }
}

const read = () => view

/**
 * The download as the screens show it. Listens while mounted, so a transfer another window
 * started shows here from its next event. `modelDownloaded` is the snapshot's word that the
 * model is here: the phase is then `ready` whatever the events said.
 */
export function useModelDownloadProgress(
  modelDownloaded: boolean,
): ModelDownloadProgress {
  useEffect(() => {
    let mounted = true
    let release: (() => void) | null = null
    void openChannel().then((close) => {
      if (mounted) release = close
      else close()
    })
    return () => {
      mounted = false
      release?.()
    }
  }, [])
  const current = useSyncExternalStore(watch, read)
  return useMemo(
    () => (modelDownloaded ? { ...current, phase: 'ready' as const } : current),
    [current, modelDownloaded],
  )
}
