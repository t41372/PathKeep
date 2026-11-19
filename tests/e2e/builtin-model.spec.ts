/**
 * The built-in model's download as Settings → AI runs it, from a model served
 * on this computer (`support/builtin-model-source.ts`: two pinned versions of
 * a 1 MB test model). A held response stops a download half way for as long
 * as a test needs, without timing.
 *
 * Proves:
 * - A window opened while the download is held shows that download's bytes
 *   at once: a held download sends no events, so they come from asking the
 *   backend which transfer runs.
 * - Cancel stops the download: once the backend takes the cancel, the screen
 *   says it is stopping and Cancel is disabled until the download ends; then
 *   it goes back to off with no error, in both windows, the backend has no
 *   transfer and keeps no partial files, and nothing is built.
 * - Turning semantic search off does the same, saying it is stopping until
 *   it has. Turned on again, a new download runs (the screen shows its
 *   bytes) and finishes; the index is built once, for the built-in model and
 *   that download, and semantic search finds a page through it.
 * - Choosing an embeddings service while the model downloads stops the
 *   download; the window that chose it says so until it has stopped; the
 *   service is sent nothing and no error shows. The window downloading
 *   cannot change the model meanwhile, so a second window does, as with the
 *   app and a browser on the dev bridge. Going back to the built-in model
 *   downloads and builds nothing by itself.
 * - With a new version of the model pinned (the app restarted on v2), the
 *   installed one no longer counts: Settings offers the download, only the
 *   changed file is fetched, the new version is published beside the old one,
 *   which is then removed, and search finds the page at every step.
 * - A download whose last bytes arrive while a move export holds the archive
 *   says it waits for that, keeps its bar, and installs once the export is done.
 *
 * Runs in its own project after `restart`: it restarts the app to give it the
 * test model's address (`PATHKEEP_TEST_MODEL_SOURCE`, read by debug builds),
 * and leaves an index made with the built-in model. At the end the AI
 * settings are put back and the app restarted without the address.
 */
import { mkdirSync, rmSync } from 'node:fs'
import path from 'node:path'
import {
  expect,
  test,
  type Page,
  type Response,
  type TestInfo,
} from '@playwright/test'
import { appendChromeVisits } from '../../scripts/fixtures/synthetic-browsers.mjs'
import { resolveDesktopBridgeEnv } from '../../scripts/pathkeep-dev-desktop-bridge.mjs'
import type {
  AiSearchResponse,
  AiSettings,
  AppConfig,
  AppSnapshot,
  ModelDownloadState,
} from '../../src/lib/types'
import { addEmbeddingsService } from './support/ai-service'
import { killAndRestartApp, unlockIfLocked } from './support/app'
import {
  MODEL_WORDS,
  MODEL_WORDS_PAGE,
  indexBuildsAfter,
  modelBundles,
  modelWordsVisit,
  newestJob,
  queue,
  recordModelDownloads,
  type DownloadEvents,
} from './support/builtin-model'
import { startBuiltinModelSource } from './support/builtin-model-source'
import { startFakeEmbeddings } from './support/fake-embeddings'
import { backend, fixture } from './support/fixture'

test.describe.configure({ mode: 'serial' })

/** Where the weights are held: right after the first progress report (one per MB). */
const ONE_PROGRESS_STEP = 1024 * 1024
const EXPORT_PASSWORD = 'held while the model installs'

let source: Awaited<ReturnType<typeof startBuiltinModelSource>> | null = null
let before: AiSettings | null = null

const snapshot = () => backend<AppSnapshot>('app_snapshot')
const saveAi = async (ai: Partial<AiSettings>) => {
  const { config } = await snapshot()
  await backend('save_config', {
    config: { ...config, ai: { ...config.ai, ...ai } } satisfies AppConfig,
  })
}
const search = (query: string) =>
  backend<AiSearchResponse>('search_ai_history', {
    request: { query, limit: 20 },
  })

/** Semantic search off with the built-in model chosen, and nothing else of this spec's left. */
const offWithBuiltIn = (): Partial<AiSettings> => ({
  enabled: false,
  semanticIndexEnabled: false,
  embeddingProviderId: 'static-in-app',
  autoIndexAfterBackup: false,
  embeddingProviders: before!.embeddingProviders,
})

test.beforeAll(async ({ browser }, info) => {
  info.setTimeout(300_000)
  source = await startBuiltinModelSource({ chunkBytes: 8192 })
  const page = await browser.newPage({
    baseURL: resolveDesktopBridgeEnv(process.env).devServerUrl,
  })
  try {
    await openSettings(page)
    before = (await snapshot()).config.ai
  } finally {
    await page.close()
  }
  await killAndRestartApp({ PATHKEEP_TEST_MODEL_SOURCE: source.url('v1') })
})

test.afterAll(async ({ browser }, info) => {
  info.setTimeout(300_000)
  await source?.close()
  if (!before) return
  // The settings as they were, then the app without the test model's address.
  const page = await browser.newPage({
    baseURL: resolveDesktopBridgeEnv(process.env).devServerUrl,
  })
  try {
    await openSettings(page)
    await saveAi(before)
  } finally {
    await page.close()
  }
  await killAndRestartApp()
})

/** Settings → AI in a window loaded now, so it reads the saved settings. */
async function openSettings(page: Page) {
  await page.goto('/#/settings/ai')
  await page.reload()
  await unlockIfLocked(page)
  await expect(semanticSwitch(page)).toBeVisible({ timeout: 60_000 })
}

const semanticSwitch = (page: Page) =>
  page.getByRole('switch', { name: 'Semantic search' })
const downloading = (page: Page) =>
  page
    .getByRole('status')
    .filter({ hasText: 'Downloading the language model…' })
const stopping = (page: Page) =>
  page.getByRole('status').filter({ hasText: 'Stopping the download…' })
/** "1.0 MB of 1.0 MB": the bytes a download line shows. */
const BYTES = /[\d.,]+ [KMG]?B of [\d.,]+ [KMG]?B/
const modelPicker = (page: Page) =>
  page.getByRole('combobox', { name: 'Model' })

/** The transfer the next download start in `page` names. Arm it before the click. */
function nextTransfer(page: Page) {
  return page
    .waitForResponse((response) =>
      response.url().endsWith('/commands/download_static_embedding_model'),
    )
    .then(async (response) => {
      expect(response.ok(), 'the start was refused').toBe(true)
      return ((await response.json()) as { transferId: string }).transferId
    })
}

/** Every `command` request `page` sends from now on, by its payload. */
function sent<T>(page: Page, command: string) {
  const payloads: T[] = []
  page.on('request', (request) => {
    if (request.url().endsWith(`/commands/${command}`))
      payloads.push(request.postDataJSON() as T)
  })
  return payloads
}

/** The jobs the index builds `page` asks for come back as. */
function builtJobs(page: Page) {
  const jobs: number[] = []
  page.on('response', (response) => {
    if (!response.url().endsWith('/commands/build_ai_index') || !response.ok())
      return
    void response
      .json()
      .then((report: { jobId?: number | null }) => {
        if (report.jobId) jobs.push(report.jobId)
      })
      .catch(() => undefined)
  })
  return jobs
}

async function expectNoError(page: Page) {
  await expect(page.getByText(/^The download did not finish/)).toHaveCount(0)
  await expect(page.getByText(/^Could not do that/)).toHaveCount(0)
}

/** The transfer is over in the backend and left nothing on disk. */
async function expectNothingInstalled(transferId: string) {
  expect(
    await backend<boolean>('cancel_ai_embedding_model_download', {
      transferId,
    }),
    'the backend has no such transfer any more',
  ).toBe(false)
  await expect
    .poll(modelBundles, { timeout: 15_000 })
    .toEqual({ current: null, generations: [], flatFiles: [] })
  expect((await snapshot()).aiStatus.staticEmbedding?.modelDownloaded).not.toBe(
    true,
  )
}

async function jobSucceeded(jobId: number) {
  await expect
    .poll(
      async () =>
        (await queue()).recentJobs.find((job) => job.id === jobId)?.state,
      { timeout: 300_000 },
    )
    .toBe('succeeded')
}

async function backUp(page: Page) {
  await page.goto('/#/browsing/backup')
  await unlockIfLocked(page)
  const button = page.getByRole('button', { name: 'Back up now' }).first()
  const finished = page.waitForResponse((response) =>
    response.url().endsWith('/commands/run_backup_now'),
  )
  await button.click()
  expect((await finished).ok()).toBe(true)
  await expect(button).toBeEnabled({ timeout: 120_000 })
}

/** History's semantic search, as a user runs it, lists the page. */
async function historyFindsPage(page: Page) {
  await page.goto('/#/browsing/history')
  await unlockIfLocked(page)
  await page
    .getByRole('searchbox', { name: 'Search history' })
    .fill(MODEL_WORDS)
  await page.getByRole('radio', { name: 'Semantic' }).click()
  await expect(page.getByText(/Semantic search with /)).toBeVisible({
    timeout: 60_000,
  })
  await expect(
    page
      .getByRole('listbox', { name: 'History results' })
      .getByRole('option')
      .filter({ hasText: MODEL_WORDS_PAGE.title })
      .first(),
  ).toBeVisible({ timeout: 60_000 })
}

async function attachEvidence(
  info: TestInfo,
  events: DownloadEvents | null,
  extra: object = {},
) {
  await info.attach('model download evidence', {
    body: JSON.stringify(
      {
        events: events?.all ?? [],
        requests: source?.requests ?? [],
        bundles: modelBundles(),
        ...extra,
      },
      null,
      2,
    ),
    contentType: 'application/json',
  })
}

test('a window opened mid-download shows it at once; Cancel says it is stopping, then stops it: off, no error, nothing installed, nothing built', async ({
  page,
}, info) => {
  test.setTimeout(300_000)
  await openSettings(page)
  await saveAi(offWithBuiltIn())
  await openSettings(page)
  expect(modelBundles().current, 'no model before this spec').toBeNull()
  const events = await recordModelDownloads()
  const builds = sent(page, 'build_ai_index')
  const firstJob = await newestJob()
  const hold = source!.holdNextChunk('model.safetensors', ONE_PROGRESS_STEP)
  const other = await page.context().newPage()
  let found: ModelDownloadState | null = null
  try {
    const transfer = nextTransfer(page)
    await semanticSwitch(page).click()
    const id = await transfer
    await hold.held
    const status = downloading(page)
    // The 1 MB report is the last thing the held download sends; the screen
    // shows it as MB, the bytes before it as KB.
    await expect(status).toContainText(/MB of [\d.,]+ MB/)
    await expect
      .poll(() =>
        events
          .of(id)
          .some(
            (event) =>
              event.kind === 'fileProgress' &&
              event.file.endsWith('model.safetensors') &&
              event.downloadedBytes >= ONE_PROGRESS_STEP,
          ),
      )
      .toBe(true)
    const bytes = (await status.textContent())?.match(BYTES)?.[0] ?? ''
    expect(bytes).toMatch(BYTES)
    const percent = await status
      .getByRole('progressbar')
      .getAttribute('aria-valuenow')
    const heard = events.of(id).length

    // A window opened now: the held download sends nothing, so what it shows
    // comes from asking the backend.
    const answers: Response[] = []
    other.on('response', (response) => {
      if (response.url().endsWith('/commands/get_model_download_state'))
        answers.push(response)
    })
    await openSettings(other)
    const otherStatus = downloading(other)
    await expect(otherStatus).toContainText(bytes)
    await expect(otherStatus.getByRole('progressbar')).toHaveAttribute(
      'aria-valuenow',
      percent ?? '',
    )
    expect(answers.length, 'the window asked the backend').toBeGreaterThan(0)
    found = (await answers.at(-1)!.json()) as ModelDownloadState | null
    expect(found?.transferId, 'the backend names the held download').toBe(id)
    expect(events.of(id).length, 'no event while it was held').toBe(heard)
    await info.attach('a window opened mid-download', {
      body: await otherStatus.screenshot(),
      contentType: 'image/png',
    })

    const cancelled = page.waitForResponse((response) =>
      response.url().endsWith('/commands/cancel_ai_embedding_model_download'),
    )
    await status.getByRole('button', { name: 'Cancel' }).click()
    expect(await (await cancelled).json(), 'the cancel was taken').toBe(true)
    // Taken, but the held read has not returned, so the download has not
    // stopped yet: the screen says it is stopping, and there is nothing left
    // to cancel.
    const ending = stopping(page)
    await expect(ending).toBeVisible()
    await expect(ending).toContainText(bytes)
    await expect(ending.getByRole('button', { name: 'Cancel' })).toBeDisabled()
    expect(events.ends(id), 'not stopped yet').toEqual([])
    await info.attach('stopping', {
      body: await ending.screenshot(),
      contentType: 'image/png',
    })
    // The backend sees the cancel once the held read returns.
    hold.release()
    await expect
      .poll(() => events.ends(id), { timeout: 60_000 })
      .toEqual(['cancelled'])

    await expect(semanticSwitch(page)).not.toBeChecked()
    for (const window of [page, other]) {
      await expect(
        window.getByText('Downloading the language model…'),
      ).toHaveCount(0)
      await expect(stopping(window)).toHaveCount(0)
      await expectNoError(window)
    }
    await expectNothingInstalled(id)
    expect((await snapshot()).config.ai.semanticIndexEnabled).toBe(false)
    expect(builds, 'no build asked for').toEqual([])
    expect(await indexBuildsAfter(firstJob), 'no index job').toEqual([])
  } finally {
    hold.release()
    events.close()
    await attachEvidence(info, events, { stateFound: found })
    await other.close()
  }
})

test('choosing an embeddings service while the model downloads stops it; the service gets nothing and no error shows', async ({
  page,
}, info) => {
  test.setTimeout(300_000)
  await openSettings(page)
  await saveAi(offWithBuiltIn())
  await openSettings(page)
  const events = await recordModelDownloads()
  const service = await startFakeEmbeddings()
  const other = await page.context().newPage()
  const builds = [page, other].map((window) => sent(window, 'build_ai_index'))
  const otherStarts = sent(other, 'download_static_embedding_model')
  const firstJob = await newestJob()
  const hold = source!.holdNextChunk('model.safetensors', ONE_PROGRESS_STEP)
  try {
    const transfer = nextTransfer(page)
    await semanticSwitch(page).click()
    const id = await transfer
    await hold.held
    await expect(downloading(page)).toContainText(/MB of [\d.,]+ MB/)
    // This window cannot change the model while it downloads.
    await expect(modelPicker(page)).toBeDisabled()

    // The second window adds the service, which chooses it; saved, it stops
    // the download. The backend sees that once the held read returns.
    await addEmbeddingsService(
      other,
      service.url,
      'e2e-embed',
      'E2E embeddings',
    )
    await expect
      .poll(async () => (await snapshot()).config.ai.embeddingProviderId)
      .not.toBe('static-in-app')
    // The second window follows the held download (it asked what runs), so
    // its save says the download is stopping until it does.
    await expect(stopping(other)).toBeVisible()
    await expect(
      stopping(other).getByRole('button', { name: 'Cancel' }),
    ).toBeDisabled()
    hold.release()
    await expect
      .poll(() => events.ends(id), { timeout: 60_000 })
      .toEqual(['cancelled'])

    await expect(page.getByText('Downloading the language model…')).toHaveCount(
      0,
    )
    await expect(modelPicker(other)).toContainText('E2E embeddings')
    for (const window of [page, other]) await expectNoError(window)
    await expectNothingInstalled(id)
    // Long enough for a build nobody asked for to reach the service.
    await page.waitForTimeout(5_000)
    expect(service.requests(), 'nothing sent to the service').toBe(0)
    expect(builds.flat(), 'no build asked for').toEqual([])
    expect(await indexBuildsAfter(firstJob), 'no index job').toEqual([])

    // Back to the built-in model: chosen, not downloaded or built.
    await modelPicker(other).click()
    await other.getByRole('option', { name: 'In-app static model' }).click()
    await other
      .getByRole('alertdialog', {
        name: 'Switch semantic search to the in-app static model?',
      })
      .getByRole('button', { name: 'Switch model' })
      .click()
    await expect(modelPicker(other)).toContainText('In-app static model')
    await expect(
      other.getByText('The model is not downloaded yet.'),
    ).toBeVisible()
    await other.waitForTimeout(3_000)
    expect(otherStarts, 'choosing it starts no download').toEqual([])
    expect(
      events.all.filter((event) => event.transferId !== id),
      'no other transfer',
    ).toEqual([])
    expect(builds.flat()).toEqual([])
    expect(service.requests()).toBe(0)
  } finally {
    hold.release()
    events.close()
    await attachEvidence(info, events, { serviceRequests: service.requests() })
    await other.close()
    await service.close()
  }
})

test('turning semantic search off stops the download; on again, a new one finishes, the index is built once and search uses the model', async ({
  page,
}, info) => {
  test.setTimeout(600_000)
  await openSettings(page)
  await saveAi(offWithBuiltIn())
  // A page the test model has words for, in the archive before the index is built.
  appendChromeVisits(fixture().files['chrome:Default'], [modelWordsVisit()])
  await backUp(page)
  await openSettings(page)
  const events = await recordModelDownloads()
  const builds = sent<{ request: unknown }>(page, 'build_ai_index')
  const jobs = builtJobs(page)
  const firstJob = await newestJob()
  let hold = source!.holdNextChunk('model.safetensors', ONE_PROGRESS_STEP)
  try {
    let transfer = nextTransfer(page)
    await semanticSwitch(page).click()
    const first = await transfer
    await hold.held
    await expect(downloading(page)).toContainText(/MB of [\d.,]+ MB/)

    // Off instead of Cancel: the saved setting stops it, which the screen
    // says until the held read returns.
    await semanticSwitch(page).click()
    await expect(stopping(page)).toBeVisible()
    await expect(
      stopping(page).getByRole('button', { name: 'Cancel' }),
    ).toBeDisabled()
    hold.release()
    await expect
      .poll(() => events.ends(first), { timeout: 60_000 })
      .toEqual(['cancelled'])
    await expect(semanticSwitch(page)).not.toBeChecked()
    await expect(page.getByText('Downloading the language model…')).toHaveCount(
      0,
    )
    await expectNoError(page)
    await expectNothingInstalled(first)
    expect(builds, 'no build for the stopped download').toEqual([])

    // On again: a new transfer, which the screen follows from its first bytes.
    hold = source!.holdNextChunk('model.safetensors', ONE_PROGRESS_STEP)
    transfer = nextTransfer(page)
    await semanticSwitch(page).click()
    const second = await transfer
    expect(second).not.toBe(first)
    await hold.held
    const status = downloading(page)
    await expect(status).toContainText(/MB of [\d.,]+ MB/)
    await expect(status.getByRole('progressbar')).toHaveAttribute(
      'aria-valuenow',
      /^[1-9]\d*$/,
    )
    await info.attach('downloading', {
      body: await status.screenshot(),
      contentType: 'image/png',
    })
    hold.release()
    await expect
      .poll(() => events.ends(second), { timeout: 120_000 })
      .toEqual(['done'])

    // The build that download allows, asked for once: the built-in model, that transfer.
    await expect.poll(() => jobs.length, { timeout: 60_000 }).toBe(1)
    expect(builds).toEqual([
      {
        request: {
          providerId: 'static-in-app',
          transferId: second,
          fullRebuild: false,
          clearOnly: false,
          scope: 'incremental',
        },
      },
    ])
    await jobSucceeded(jobs[0])
    expect(
      (await indexBuildsAfter(firstJob)).map((job) => job.id),
      'one index build',
    ).toEqual(jobs)
    await expect
      .poll(async () => (await queue()).semanticIndex.chosenComplete, {
        timeout: 300_000,
      })
      .toBe(true)
    expect((await queue()).semanticIndex.serving?.providerId).toBe(
      'static-in-app',
    )
    const installed = modelBundles()
    expect(installed.current).not.toBeNull()
    expect(installed.generations).toEqual([installed.current])
    await expect(page.getByText(/[\d,]+ visits indexed/).first()).toBeVisible({
      timeout: 60_000,
    })
    await expectNoError(page)

    // Search uses the model: the page is matched by meaning, through its index.
    const answer = await search(MODEL_WORDS)
    expect(answer.providerId).toBe('static-in-app')
    expect(
      answer.items.find((item) => item.url === MODEL_WORDS_PAGE.url)
        ?.matchReason,
    ).toMatch(/semantic/)
    expect(answer.noteCodes ?? []).not.toContainEqual(
      expect.objectContaining({ code: 'previousModel' }),
    )
    await historyFindsPage(page)
  } finally {
    hold.release()
    events.close()
    await attachEvidence(info, events, { builds, jobs })
  }
})

test('a new version of the model installs beside the old one, and search finds the page throughout', async ({
  page,
}, info) => {
  test.setTimeout(600_000)
  const steps: object[] = []
  /** Search finds the page by its words; how it matched is kept for the report. */
  const findsPage = async (step: string) => {
    const answer = await search(MODEL_WORDS)
    const item = answer.items.find(
      (entry) => entry.url === MODEL_WORDS_PAGE.url,
    )
    steps.push({
      step,
      matchReason: item?.matchReason,
      notes: answer.noteCodes,
    })
    expect(item, `search finds the page ${step}`).toBeDefined()
    return answer
  }
  // From the test before: v1 installed and its index complete.
  await openSettings(page)
  const old = modelBundles()
  expect(old.current).not.toBeNull()
  await findsPage('with v1')

  await killAndRestartApp({ PATHKEEP_TEST_MODEL_SOURCE: source!.url('v2') })
  await openSettings(page)
  await findsPage('after the restart with v2 pinned')
  // The weights on disk are not v2's: Settings offers the download.
  await expect(page.getByText('The model is not downloaded yet.')).toBeVisible()
  const events = await recordModelDownloads()
  const builds = sent<{ request: Record<string, unknown> }>(
    page,
    'build_ai_index',
  )
  const jobs = builtJobs(page)
  const fetchedBefore = source!.requests.length
  const hold = source!.holdNextChunk('model.safetensors', ONE_PROGRESS_STEP)
  try {
    const transfer = nextTransfer(page)
    await page.getByRole('button', { name: 'Download', exact: true }).click()
    const id = await transfer
    await hold.held
    await expect(downloading(page)).toContainText(/MB of [\d.,]+ MB/)
    await findsPage('while v2 downloads')
    expect(modelBundles().current, 'v1 stays until v2 is complete').toBe(
      old.current,
    )

    hold.release()
    await expect
      .poll(() => events.ends(id), { timeout: 120_000 })
      .toEqual(['done'])
    // Only the changed file came over the network.
    expect(source!.requests.slice(fetchedBefore)).toEqual([
      '/v2/model.safetensors',
    ])
    // v2 is published, and v1's folder is removed.
    await expect
      .poll(
        () => {
          const now = modelBundles()
          return (
            now.current !== old.current &&
            now.generations.length === 1 &&
            now.generations[0] === now.current
          )
        },
        { timeout: 30_000 },
      )
      .toBe(true)
    await expect.poll(() => jobs.length, { timeout: 60_000 }).toBe(1)
    expect(builds).toHaveLength(1)
    expect(builds[0].request).toMatchObject({
      providerId: 'static-in-app',
      transferId: id,
    })
    await jobSucceeded(jobs[0])
    await openSettings(page)
    await expect(
      page.getByText('The model is not downloaded yet.'),
    ).toHaveCount(0)
    await expectNoError(page)
    const after = await findsPage('with v2 installed')
    expect(after.noteCodes ?? []).not.toContainEqual(
      expect.objectContaining({ code: 'providerResolutionFailed' }),
    )
    await historyFindsPage(page)
  } finally {
    hold.release()
    events.close()
    await attachEvidence(info, events, { steps, builds, jobs })
  }
})

test('a download that finishes while a move export holds the archive waits for it, then installs', async ({
  page,
}, info) => {
  test.setTimeout(600_000)
  // v1 pinned again: the v2 weights on disk no longer count.
  await killAndRestartApp({ PATHKEEP_TEST_MODEL_SOURCE: source!.url('v1') })
  await openSettings(page)
  await expect(page.getByText('The model is not downloaded yet.')).toBeVisible()
  const events = await recordModelDownloads()
  const builds = sent<{ request: Record<string, unknown> }>(
    page,
    'build_ai_index',
  )
  const jobs = builtJobs(page)
  const exporting = await page.context().newPage()
  const file = path.join(fixture().root, 'builtin-model', 'moved.pathkeep')
  mkdirSync(path.dirname(file), { recursive: true })
  const hold = source!.holdNextChunk('model.safetensors', ONE_PROGRESS_STEP)
  try {
    const transfer = nextTransfer(page)
    await page.getByRole('button', { name: 'Download', exact: true }).click()
    const id = await transfer
    await hold.held
    await expect(downloading(page)).toContainText(/MB of [\d.,]+ MB/)

    // A second window moves the archive out: the export holds the archive.
    await exporting.goto('/#/settings/storage')
    await unlockIfLocked(exporting)
    await exporting
      .getByText('Move to another computer', { exact: true })
      .locator('xpath=../..')
      .getByRole('button', { name: 'Export…' })
      .click()
    const dialog = exporting.getByRole('dialog')
    await dialog.getByLabel('Save to').fill(file)
    await dialog.getByLabel('Password for this file').fill(EXPORT_PASSWORD)
    await dialog.getByLabel('Type it again').fill(EXPORT_PASSWORD)
    await dialog.getByRole('button', { name: 'Export', exact: true }).click()
    await expect(dialog.getByText(/Copying the archive/)).toBeVisible({
      timeout: 120_000,
    })

    // The last bytes arrive; installing them needs the archive.
    hold.release()
    const waiting = page
      .getByRole('status')
      .filter({ hasText: 'Waiting for another archive task to finish…' })
    await expect(waiting).toBeVisible({ timeout: 60_000 })
    // The bar keeps what it had: every byte arrived.
    await expect(waiting).toContainText(/MB of [\d.,]+ MB/)
    await expect(waiting.getByRole('progressbar')).toHaveAttribute(
      'aria-valuenow',
      '100',
    )
    await info.attach('waiting for the export', {
      body: await waiting.screenshot(),
      contentType: 'image/png',
    })
    expect(
      events
        .of(id)
        .some(
          (event) =>
            event.kind === 'waiting' && event.reason === 'archive-busy',
        ),
    ).toBe(true)
    expect(events.ends(id), 'not installed while the export runs').toEqual([])

    await expect(exporting.getByText(/^Saved /)).toBeVisible({
      timeout: 300_000,
    })
    await expect
      .poll(() => events.ends(id), { timeout: 120_000 })
      .toEqual(['done'])
    await expect(waiting).toHaveCount(0)
    await expect.poll(() => jobs.length, { timeout: 60_000 }).toBe(1)
    expect(builds).toHaveLength(1)
    expect(builds[0].request).toMatchObject({
      providerId: 'static-in-app',
      transferId: id,
    })
    await jobSucceeded(jobs[0])
    await expect(
      page.getByText('The model is not downloaded yet.'),
    ).toHaveCount(0)
    await expectNoError(page)
    const installed = modelBundles()
    expect(installed.generations).toEqual([installed.current])
  } finally {
    hold.release()
    events.close()
    await attachEvidence(info, events, { builds, jobs })
    await exporting.close()
    rmSync(path.dirname(file), { recursive: true, force: true })
  }
})
