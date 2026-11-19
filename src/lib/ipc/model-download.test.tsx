/**
 * How one attempt at the in-app model download ends, and which transfer a
 * window shows. Responsibilities: the orderings the desktop E2E cannot hold
 * still: a start reply that comes late, a refused start, a replaced
 * transfer's events arriving after its replacement's, a download that goes
 * quiet for longer than the idle window and then ends. Not responsible for the
 * backend's transfers or the Settings copy (`tests/e2e/builtin-model.spec.ts`).
 *
 * Failure modes, written before the code:
 * - a refused start (no transfer, so no terminal event) leaves the screen on
 *   "Downloading" for good;
 * - a start refused because the user's own save turned the model off is shown
 *   as a failure;
 * - events sent before the start answered are dropped, or a replaced
 *   transfer's events are taken for the new one's;
 * - waiting for an archive task is taken for silence and given up after the
 *   idle window;
 * - a start reply arriving after the idle window ended the attempt makes the
 *   window follow that transfer again;
 * - a replaced transfer's late progress or `cancelled` moves or clears the bar
 *   of the transfer that replaced it, in the window that started it and in
 *   one that only saw its events;
 * - late progress or the end of a transfer the window never followed moves or
 *   ends the one it shows;
 * - starting again while a download runs (the backend joins it) empties the bar;
 * - Cancel pressed before the start answered cancels nothing;
 * - a setup whose transfer another window replaced never learns that its own
 *   transfer was cancelled;
 * - a window opened while a download runs shows nothing until the next event
 *   (none comes while a response is held); leaving the screen and coming back
 *   while it runs shows it as not running, and ignores its next events;
 * - the backend's answer about the running transfer, arriving after that
 *   transfer ended, brings it back; or, older than the events heard
 *   meanwhile, moves the bar backwards; or takes over from this window's own
 *   start;
 * - an accepted Cancel, or a saved setting that no longer wants the model,
 *   leaves the screen saying "Downloading" until the transfer stops; a
 *   refused Cancel or a save that still wants it says "Stopping"; the
 *   stopping state outlives the transfer's end; starting again while it
 *   stops shows the stopping transfer's bar as the new download's;
 * - which transfer is newer is guessed from what its event says: an older
 *   transfer's `fileStarted`, sent late once its response arrives, takes the
 *   place of the newer one, which is then ignored to its end; or a newer
 *   transfer's progress is ignored because its first event was missed;
 * - with nothing followed, a late event of a transfer older than one already
 *   heard, or the backend's answer naming one, is taken up;
 * - a download given up as silent that resumes and finishes stays "did not
 *   finish", and its caller never hears that it finished (so its index is
 *   never built); or its later error or cancellation is not shown.
 *
 * Only the event channel and the three commands are stand-ins; the module and
 * the React hook are real.
 */
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import type {
  ModelDownloadProgressEvent,
  ModelDownloadState,
  ModelDownloadTransfer,
} from '@/lib/types'
import type * as ModelDownload from './model-download'

const channel = vi.hoisted(() => {
  const listeners = new Set<(payload: unknown) => void>()
  return {
    listeners,
    send(payload: unknown) {
      for (const listener of [...listeners]) listener(payload)
    },
  }
})

const commands = vi.hoisted(() => ({
  start: vi.fn<() => Promise<ModelDownloadTransfer>>(),
  cancel: vi.fn<(transferId: string) => Promise<boolean>>(),
  state: vi.fn<() => Promise<ModelDownloadState | null>>(),
}))

vi.mock('./events', () => ({
  listenDesktopEvent: (_name: string, listener: (payload: unknown) => void) => {
    channel.listeners.add(listener)
    return Promise.resolve(() => {
      channel.listeners.delete(listener)
    })
  },
}))

vi.mock('../backend-client/intelligence', () => ({
  intelligenceClient: {
    downloadStaticEmbeddingModel: commands.start,
    cancelStaticEmbeddingModelDownload: commands.cancel,
    getModelDownloadState: commands.state,
  },
}))

Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })

let md: typeof ModelDownload
const unmounts: (() => void)[] = []

beforeEach(async () => {
  vi.resetModules()
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
  channel.listeners.clear()
  commands.start.mockReset()
  commands.cancel.mockReset()
  commands.state.mockReset()
  commands.state.mockResolvedValue(null)
  md = await import('./model-download')
})

afterEach(() => {
  for (const unmount of unmounts.splice(0)) unmount()
  vi.useRealTimers()
})

/** Lets pending promise callbacks run. */
async function flush() {
  await act(async () => {
    for (let i = 0; i < 20; i++) await Promise.resolve()
  })
}

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((done) => {
    resolve = done
  })
  return { promise, resolve }
}

/** A screen that shows the download, as Settings → AI does. */
async function screen() {
  const root = createRoot(document.createElement('div'))
  let seen: ModelDownload.ModelDownloadProgress | null = null
  function Probe() {
    seen = md.useModelDownloadProgress(false)
    return null
  }
  act(() => root.render(<Probe />))
  unmounts.push(() => act(() => root.unmount()))
  await flush()
  return () => seen!
}

/** Starts the download the way a setup does. */
function start(wanted: () => Promise<boolean>) {
  let outcome!: Promise<ModelDownload.ModelDownloadOutcome>
  act(() => {
    outcome = md.downloadModel(wanted)
  })
  return outcome
}

const A = 'a'.repeat(32)
const B = 'b'.repeat(32)
/** The order the backend started them in: B after A. */
const SEQ: Record<string, number> = { [A]: 1, [B]: 2 }

type Unnumbered<T> = T extends unknown ? Omit<T, 'seq'> : never

/** Sends an event on the channel, numbered with its transfer's `seq`. */
async function send(event: Unnumbered<ModelDownloadProgressEvent>) {
  await act(async () => {
    channel.send({ seq: SEQ[event.transferId], ...event })
    await Promise.resolve()
  })
}
const stillWanted = () => Promise.resolve(true)
const noLongerWanted = () => Promise.resolve(false)

const started = (
  transferId: string,
  file = 'model.safetensors',
  total = 1000,
) =>
  ({
    transferId,
    kind: 'fileStarted',
    file,
    totalBytes: total,
  }) as const
const progress = (transferId: string, downloadedBytes: number) =>
  ({
    transferId,
    kind: 'fileProgress',
    file: 'model.safetensors',
    downloadedBytes,
    totalBytes: 1000,
  }) as const

it('a refused start shows why instead of downloading for good', async () => {
  const view = await screen()
  commands.start.mockRejectedValue(new Error('the archive is locked'))
  const outcome = start(stillWanted)
  expect(view().phase).toBe('downloading')
  expect(await outcome).toMatchObject({ kind: 'refused' })
  await flush()
  expect(view()).toMatchObject({
    phase: 'failed',
    failure: { kind: 'error', message: 'the archive is locked' },
  })
})

it('a start refused because a saved setting no longer wants the model ends as a cancellation', async () => {
  const view = await screen()
  commands.start.mockRejectedValue(new Error('semantic search is off'))
  expect(await start(noLongerWanted)).toEqual({ kind: 'cancelled' })
  await flush()
  expect(view()).toMatchObject({ phase: 'idle', failure: null })
})

it('keeps the events sent before the start answers, for that transfer only', async () => {
  const view = await screen()
  const reply = deferred<ModelDownloadTransfer>()
  commands.start.mockReturnValue(reply.promise)
  const outcome = start(stillWanted)
  await flush()
  await send(progress(A, 900))
  await send({ transferId: A, kind: 'cancelled' })
  await send(started(B))
  await send(progress(B, 400))
  await send({ transferId: B, kind: 'done' })
  reply.resolve({ transferId: B, seq: SEQ[B] })
  expect(await outcome).toEqual({ kind: 'done', transferId: B })
  await flush()
  expect(view()).toMatchObject({
    phase: 'ready',
    downloadedBytes: 400,
    totalBytes: 1000,
  })
})

it('a download waiting for an archive task is not given up as silent', async () => {
  const view = await screen()
  commands.start.mockResolvedValue({ transferId: A, seq: SEQ[A] })
  const outcome = start(stillWanted)
  await flush()
  await send(progress(A, 500))
  // The backend repeats `waiting` every 20 s, for longer than the idle window.
  for (let i = 0; i < 8; i++) {
    await act(async () => {
      vi.advanceTimersByTime(20_000)
      await Promise.resolve()
    })
    await send({ transferId: A, kind: 'waiting', reason: 'archive-busy' })
  }
  expect(view()).toMatchObject({
    phase: 'waiting',
    waitingFor: 'archive-busy',
    downloadedBytes: 500,
    totalBytes: 1000,
  })
  await send({ transferId: A, kind: 'done' })
  expect(await outcome).toEqual({ kind: 'done', transferId: A })
})

it('gives up after the idle window, and a start reply that comes later changes nothing', async () => {
  const view = await screen()
  const reply = deferred<ModelDownloadTransfer>()
  commands.start.mockReturnValue(reply.promise)
  const outcome = start(stillWanted)
  await flush()
  await act(async () => {
    vi.advanceTimersByTime(md.MODEL_DOWNLOAD_IDLE_TIMEOUT_MS)
    await Promise.resolve()
  })
  expect(await outcome).toEqual({ kind: 'silent' })
  expect(view()).toMatchObject({ phase: 'failed', failure: { kind: 'silent' } })
  reply.resolve({ transferId: A, seq: SEQ[A] })
  await flush()
  await send(started(A))
  await send(progress(A, 300))
  expect(view()).toMatchObject({ phase: 'failed', failure: { kind: 'silent' } })
  expect(await md.cancelModelDownload()).toBe(false)
  expect(commands.cancel).not.toHaveBeenCalled()
})

it('a replaced transfer’s late progress and cancellation leave the newer bar alone', async () => {
  const view = await screen()
  commands.start
    .mockResolvedValueOnce({ transferId: A, seq: SEQ[A] })
    .mockResolvedValueOnce({ transferId: B, seq: SEQ[B] })
  const first = start(stillWanted)
  await flush()
  await send(progress(A, 600))
  // Turned off and on again: the backend cancelled A and started B.
  const second = start(stillWanted)
  await flush()
  await send({ transferId: B, kind: 'waiting', reason: 'previous-transfer' })
  await send(progress(A, 700))
  await send({ transferId: A, kind: 'cancelled' })
  expect(await first).toEqual({ kind: 'cancelled' })
  expect(view()).toMatchObject({
    phase: 'waiting',
    waitingFor: 'previous-transfer',
    downloadedBytes: 0,
  })
  await send(started(B))
  await send(progress(B, 100))
  expect(view()).toMatchObject({
    phase: 'downloading',
    downloadedBytes: 100,
    totalBytes: 1000,
    failure: null,
  })
  await send({ transferId: B, kind: 'done' })
  expect(await second).toEqual({ kind: 'done', transferId: B })
})

it('a window that did not start the download follows the newer transfer and ignores the older one’s late cancellation', async () => {
  const view = await screen()
  await send(started(A))
  await send(progress(A, 600))
  expect(view()).toMatchObject({ phase: 'downloading', downloadedBytes: 600 })
  await send({ transferId: B, kind: 'waiting', reason: 'previous-transfer' })
  expect(view()).toMatchObject({
    phase: 'waiting',
    waitingFor: 'previous-transfer',
    downloadedBytes: 0,
  })
  await send(progress(A, 700))
  await send({ transferId: A, kind: 'cancelled' })
  expect(view()).toMatchObject({ phase: 'waiting', downloadedBytes: 0 })
  await send(started(B))
  await send(progress(B, 50))
  expect(view()).toMatchObject({ phase: 'downloading', downloadedBytes: 50 })
  commands.cancel.mockResolvedValue(true)
  expect(await md.cancelModelDownload()).toBe(true)
  expect(commands.cancel).toHaveBeenCalledWith(B)
})

it('late progress or the end of a transfer this window never followed leaves the one it shows alone', async () => {
  const view = await screen()
  commands.start.mockResolvedValue({ transferId: B, seq: SEQ[B] })
  const outcome = start(stillWanted)
  await flush()
  await send(progress(B, 200))
  await send(progress(A, 900))
  await send({ transferId: A, kind: 'cancelled' })
  await send({ transferId: A, kind: 'error', message: 'disk full' })
  expect(view()).toMatchObject({
    phase: 'downloading',
    downloadedBytes: 200,
    failure: null,
  })
  await send({ transferId: B, kind: 'done' })
  expect(await outcome).toEqual({ kind: 'done', transferId: B })
})

it('starting again while a download runs joins it and keeps the bar', async () => {
  const view = await screen()
  commands.start.mockResolvedValue({ transferId: A, seq: SEQ[A] })
  const first = start(stillWanted)
  await flush()
  await send(started(A, 'tokenizer.json', 100))
  await send({ transferId: A, kind: 'fileFinished', file: 'tokenizer.json' })
  await send(started(A))
  await send(progress(A, 500))
  const second = start(stillWanted)
  expect(view()).toMatchObject({ downloadedBytes: 600, totalBytes: 1100 })
  await flush()
  await send(progress(A, 900))
  expect(view()).toMatchObject({
    phase: 'downloading',
    downloadedBytes: 1000,
    totalBytes: 1100,
  })
  await send({ transferId: A, kind: 'done' })
  expect(await first).toEqual({ kind: 'done', transferId: A })
  expect(await second).toEqual({ kind: 'done', transferId: A })
})

it('Cancel pressed before the start answers cancels the transfer it names', async () => {
  await screen()
  const reply = deferred<ModelDownloadTransfer>()
  commands.start.mockReturnValue(reply.promise)
  commands.cancel.mockResolvedValue(true)
  void start(stillWanted)
  await flush()
  const cancelled = md.cancelModelDownload()
  reply.resolve({ transferId: A, seq: SEQ[A] })
  expect(await cancelled).toBe(true)
  expect(commands.cancel).toHaveBeenCalledWith(A)
})

it('a setup whose transfer another window replaced still learns it was cancelled', async () => {
  const view = await screen()
  commands.start.mockResolvedValue({ transferId: A, seq: SEQ[A] })
  const outcome = start(stillWanted)
  await flush()
  await send(progress(A, 300))
  // Another window turned semantic search off and on again.
  await send({ transferId: B, kind: 'waiting', reason: 'previous-transfer' })
  await send({ transferId: A, kind: 'cancelled' })
  expect(await outcome).toEqual({ kind: 'cancelled' })
  expect(view()).toMatchObject({
    phase: 'waiting',
    waitingFor: 'previous-transfer',
  })
})

/** The backend's answer about a running transfer: the weights at `done` of 1000, the tokenizer done. */
const runningState = (
  transferId: string,
  done: number,
): ModelDownloadState => ({
  transferId,
  seq: SEQ[transferId],
  phase: 'downloading',
  currentFile: 'model.safetensors',
  downloadedBytes: 100 + done,
  totalBytes: 1100,
  files: {
    'tokenizer.json': { downloadedBytes: 100, totalBytes: 100 },
    'model.safetensors': { downloadedBytes: done, totalBytes: 1000 },
  },
})

const wantsIt = {
  enabled: true,
  semanticIndexEnabled: true,
  embeddingProviderId: 'static-in-app',
}

it('a window opened while a download runs shows it at once, and follows it', async () => {
  commands.state.mockResolvedValue(runningState(A, 500))
  const view = await screen()
  expect(view()).toMatchObject({
    phase: 'downloading',
    downloadedBytes: 600,
    totalBytes: 1100,
  })
  await send(progress(A, 700))
  expect(view()).toMatchObject({ downloadedBytes: 800, totalBytes: 1100 })
  commands.cancel.mockResolvedValue(true)
  expect(await md.cancelModelDownload()).toBe(true)
  expect(commands.cancel).toHaveBeenCalledWith(A)
})

it('leaving the screen and coming back while a download runs shows it again', async () => {
  commands.state.mockResolvedValue(runningState(A, 500))
  await screen()
  // The only screen unmounts: nobody listens any more.
  for (const unmount of unmounts.splice(0)) unmount()
  await flush()
  commands.state.mockResolvedValue(runningState(A, 700))
  const view = await screen()
  expect(view()).toMatchObject({
    phase: 'downloading',
    downloadedBytes: 800,
    totalBytes: 1100,
  })
  await send(progress(A, 900))
  expect(view()).toMatchObject({ downloadedBytes: 1000 })
})

it('a transfer found waiting says what it waits for, with its bytes', async () => {
  commands.state.mockResolvedValue({
    ...runningState(A, 1000),
    phase: 'waiting',
    reason: 'archive-busy',
  })
  const view = await screen()
  expect(view()).toMatchObject({
    phase: 'waiting',
    waitingFor: 'archive-busy',
    downloadedBytes: 1100,
    totalBytes: 1100,
  })
})

it('a transfer that ended while the window asked about it is not brought back', async () => {
  const answer = deferred<ModelDownloadState | null>()
  commands.state.mockReturnValue(answer.promise)
  const view = await screen()
  await send({ transferId: A, kind: 'cancelled' })
  answer.resolve(runningState(A, 500))
  await flush()
  expect(view()).toMatchObject({ phase: 'idle', downloadedBytes: 0 })
})

it('events heard while the window asked keep the bar where they put it', async () => {
  const answer = deferred<ModelDownloadState | null>()
  commands.state.mockReturnValue(answer.promise)
  const view = await screen()
  await send(started(A))
  await send(progress(A, 800))
  answer.resolve(runningState(A, 500))
  await flush()
  // The tokenizer only the answer knows of is added; the weights keep 800.
  expect(view()).toMatchObject({
    phase: 'downloading',
    downloadedBytes: 900,
    totalBytes: 1100,
  })
})

it('the answer does not take over from a start this window sent', async () => {
  const answer = deferred<ModelDownloadState | null>()
  commands.state.mockReturnValue(answer.promise)
  const view = await screen()
  const reply = deferred<ModelDownloadTransfer>()
  commands.start.mockReturnValue(reply.promise)
  void start(stillWanted)
  await flush()
  answer.resolve(runningState(A, 500))
  await flush()
  expect(view()).toMatchObject({ phase: 'downloading', downloadedBytes: 0 })
  reply.resolve({ transferId: B, seq: SEQ[B] })
  await flush()
  await send(progress(B, 100))
  expect(view()).toMatchObject({ downloadedBytes: 100, totalBytes: 1000 })
})

it('an accepted Cancel says the download is stopping until it ends; a refused one does not', async () => {
  const view = await screen()
  commands.start.mockResolvedValue({ transferId: A, seq: SEQ[A] })
  const outcome = start(stillWanted)
  await flush()
  await send(progress(A, 500))
  commands.cancel.mockResolvedValueOnce(false)
  await act(async () => {
    await md.cancelModelDownload()
  })
  expect(view()).toMatchObject({ phase: 'downloading', stopping: false })
  commands.cancel.mockResolvedValueOnce(true)
  await act(async () => {
    await md.cancelModelDownload()
  })
  expect(view()).toMatchObject({
    phase: 'downloading',
    stopping: true,
    downloadedBytes: 500,
  })
  await send({ transferId: A, kind: 'cancelled' })
  expect(await outcome).toEqual({ kind: 'cancelled' })
  expect(view()).toMatchObject({ phase: 'idle', stopping: false })
})

it('a saved setting that no longer wants the model says the download is stopping; one that still wants it does not', async () => {
  const view = await screen()
  commands.start.mockResolvedValue({ transferId: A, seq: SEQ[A] })
  const outcome = start(stillWanted)
  await flush()
  await send(progress(A, 500))
  act(() => md.settingsSaved(wantsIt))
  expect(view()).toMatchObject({ stopping: false })
  act(() => md.settingsSaved({ ...wantsIt, embeddingProviderId: 'lm-studio' }))
  expect(view()).toMatchObject({ phase: 'downloading', stopping: true })
  // Publication was decided before the save: the transfer still finishes.
  await send({ transferId: A, kind: 'done' })
  expect(await outcome).toEqual({ kind: 'done', transferId: A })
  expect(view()).toMatchObject({ phase: 'ready', stopping: false })
})

it('starting again while a download stops shows the new one from nothing', async () => {
  const view = await screen()
  commands.start.mockResolvedValueOnce({ transferId: A, seq: SEQ[A] })
  void start(stillWanted)
  await flush()
  await send(progress(A, 500))
  commands.cancel.mockResolvedValueOnce(true)
  await act(async () => {
    await md.cancelModelDownload()
  })
  const reply = deferred<ModelDownloadTransfer>()
  commands.start.mockReturnValueOnce(reply.promise)
  void start(stillWanted)
  expect(view()).toMatchObject({
    phase: 'downloading',
    stopping: false,
    downloadedBytes: 0,
  })
  reply.resolve({ transferId: B, seq: SEQ[B] })
  await flush()
  await send({ transferId: B, kind: 'waiting', reason: 'previous-transfer' })
  await send({ transferId: A, kind: 'cancelled' })
  expect(view()).toMatchObject({
    phase: 'waiting',
    waitingFor: 'previous-transfer',
    downloadedBytes: 0,
  })
})

it('a window that knows the newest transfer ignores an older one’s late first event, and follows the newest to its end', async () => {
  // B waits for A, which was cancelled while it waited for its response.
  commands.state.mockResolvedValue({
    transferId: B,
    seq: SEQ[B],
    phase: 'waiting',
    reason: 'previous-transfer',
    currentFile: null,
    downloadedBytes: 0,
    totalBytes: 0,
    files: {},
  })
  const view = await screen()
  expect(view()).toMatchObject({
    phase: 'waiting',
    waitingFor: 'previous-transfer',
  })
  // A's response arrives at last: it announces its file before it sees the cancel.
  await send(started(A))
  await send(progress(A, 10))
  await send({ transferId: A, kind: 'cancelled' })
  expect(view()).toMatchObject({
    phase: 'waiting',
    waitingFor: 'previous-transfer',
    downloadedBytes: 0,
  })
  await send(started(B))
  await send(progress(B, 300))
  expect(view()).toMatchObject({
    phase: 'downloading',
    downloadedBytes: 300,
    totalBytes: 1000,
  })
  await send({ transferId: B, kind: 'done' })
  expect(view().phase).toBe('ready')
})

it('a newer transfer takes the place of the one followed whatever its first event heard here is', async () => {
  const view = await screen()
  await send(started(A))
  await send(progress(A, 600))
  // B's first events were missed.
  await send(progress(B, 50))
  expect(view()).toMatchObject({ phase: 'downloading', downloadedBytes: 50 })
  await send(progress(A, 700))
  expect(view()).toMatchObject({ downloadedBytes: 50 })
})

it('with nothing followed, a late event of a transfer older than one heard is not taken up', async () => {
  const view = await screen()
  await send(started(B))
  await send(progress(B, 100))
  await send({ transferId: B, kind: 'done' })
  expect(view()).toMatchObject({ phase: 'ready', downloadedBytes: 100 })
  await send(progress(A, 900))
  expect(view()).toMatchObject({ phase: 'ready', downloadedBytes: 100 })
})

/**
 * Starts a download of `transferId`, hears its first bytes, then lets the idle window pass.
 * Returns how the transfer ends after all, wrapped: awaiting the bare promise would wait for it.
 */
async function stall(transferId: string) {
  commands.start.mockResolvedValueOnce({ transferId, seq: SEQ[transferId] })
  const outcome = start(stillWanted)
  await flush()
  await send(progress(transferId, 500))
  await act(async () => {
    vi.advanceTimersByTime(md.MODEL_DOWNLOAD_IDLE_TIMEOUT_MS)
    await Promise.resolve()
  })
  const given = await outcome
  if (given.kind !== 'silent' || !given.late)
    throw new Error(`not given up with its end to come: ${given.kind}`)
  return { late: given.late }
}

it('a download given up as silent that resumes and finishes is shown ready, and its caller hears it', async () => {
  const view = await screen()
  const { late } = await stall(A)
  expect(view()).toMatchObject({ phase: 'failed', failure: { kind: 'silent' } })
  // The connection comes back.
  await send(progress(A, 1000))
  expect(view()).toMatchObject({
    phase: 'downloading',
    downloadedBytes: 1000,
    failure: null,
  })
  await send({ transferId: A, kind: 'done' })
  expect(await late).toEqual({ kind: 'done', transferId: A })
  expect(view()).toMatchObject({ phase: 'ready', failure: null })
})

it('a download given up as silent that then fails, or is cancelled, shows that', async () => {
  const view = await screen()
  const { late: failed } = await stall(A)
  await send({ transferId: A, kind: 'error', message: 'connection reset' })
  expect(await failed).toEqual({ kind: 'failed', message: 'connection reset' })
  expect(view()).toMatchObject({
    phase: 'failed',
    failure: { kind: 'error', message: 'connection reset' },
  })

  const { late: cancelled } = await stall(B)
  await send({ transferId: B, kind: 'cancelled' })
  expect(await cancelled).toEqual({ kind: 'cancelled' })
  expect(view()).toMatchObject({ phase: 'idle', failure: null })
})

it('an answer about a transfer older than one heard meanwhile is not taken up', async () => {
  const answer = deferred<ModelDownloadState | null>()
  commands.state.mockReturnValue(answer.promise)
  const view = await screen()
  await send(started(B))
  await send({ transferId: B, kind: 'done' })
  // Asked before B began; A is the transfer B replaced, still finishing.
  answer.resolve(runningState(A, 500))
  await flush()
  expect(view()).toMatchObject({ phase: 'ready' })
})
