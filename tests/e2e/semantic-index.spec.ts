/**
 * Semantic search with an embeddings service of one's own: an index build
 * the user starts, pauses and continues, that survives the app being killed,
 * and that nothing else starts behind their back.
 *
 * Proves:
 * - Adding the service checks it by its model list only: no text is sent to
 *   it until semantic search is turned on.
 * - The build, interrupted by killing the app the way a crash would, carries
 *   on when the app starts again, without starting over.
 * - Paused half way, it stays paused through a restart and through a backup
 *   (the after-backup update does not carry an unfinished build on, which is
 *   what happened on 2026-10-15): the service gets no request. Continue goes
 *   on with it; cancelled half way, Continue starts a new build that reads
 *   only what the index lacks. It ends with every visit indexed and no page
 *   sent twice. Settings shows the progress while it runs.
 * - History's semantic search finds a page by its words, and its date filter
 *   applies to the matches themselves. After a later backup the index catches
 *   up by itself: a page visited again as it was is not sent again, a
 *   retitled one is, and is found by its new title. Storage counts the
 *   index's files.
 * - Changing the model builds nothing and search keeps the old index, until
 *   the user builds the new one; then the old index's files are gone.
 * - A build that ends while History is open makes semantic search available
 *   there, without a reload.
 * - The model is sent each page's own text, title first, without the visit's
 *   time, profile or field labels; a model that wants an instruction on
 *   searches (Qwen3-Embedding) gets it once, on searches only.
 * - An index made from the older page text (its file as PathKeep wrote it
 *   before) keeps working and keeps taking new visits after a backup, and
 *   Settings and History say so; Rebuild index reads every page again and
 *   the note goes away.
 *
 * The app is killed for real (`support/app.ts`; the E2E launcher starts it
 * again). The service runs on this computer (`support/fake-embeddings.ts`);
 * the in-app model is a 531 MB download. It answers the build's first
 * request and holds the next, so the kill lands half way every time, and is
 * told to hold again for the pause.
 */
import { createHash } from 'node:crypto'
import {
  readFileSync,
  readdirSync,
  renameSync,
  statSync,
  writeFileSync,
} from 'node:fs'
import path from 'node:path'
import { expect, test } from '@playwright/test'
import {
  appendChromeVisits,
  retitleChromePage,
} from '../../scripts/fixtures/synthetic-browsers.mjs'
import { killAndRestartApp, unlockIfLocked } from './support/app'
import { startFakeEmbeddings } from './support/fake-embeddings'
import {
  archivedVisits,
  backend,
  fixture,
  localDay,
  parseCount,
} from './support/fixture'
import {
  NEW_PAGE,
  RETITLED_TITLE,
  indexRevisitVisits,
  newPageVisit,
} from './support/index-revisit'

interface QueueStatus {
  queued: number
  running: number
  semanticIndex: {
    chosen: { providerId: string; model: string } | null
    serving: { providerId: string; model: string } | null
    chosenComplete: boolean
    build: { state: string; pausedBy?: string | null } | null
  }
}
const queueStatus = () => backend<QueueStatus>('load_ai_queue_status')

/** Settings as saved; the test puts their AI part back when it ends. */
interface Config {
  ai: unknown
}

const loadConfig = async () =>
  (await backend<{ config: Config }>('app_snapshot')).config

/** Bytes of every file under `dir`; 0 when it does not exist. */
function folderBytes(dir: string): number {
  try {
    return readdirSync(dir, { withFileTypes: true }).reduce(
      (sum, entry) =>
        sum +
        (entry.isDirectory()
          ? folderBytes(path.join(dir, entry.name))
          : statSync(path.join(dir, entry.name)).size),
      0,
    )
  } catch {
    return 0
  }
}

/**
 * Files of the index `model` made with service `providerId`, under
 * `derived/vectors`: named like `ai_sidecar::provider_table_name`.
 */
const indexFilesOf = (providerId: string, model: string) => {
  const dir = path.join(fixture().dirs.projectRoot, 'derived', 'vectors')
  const stem = `pathkeep_${providerId.replace(/[^A-Za-z0-9]/g, '_')}_${createHash(
    'sha256',
  )
    .update(`${providerId}::${model}`)
    .digest('hex')
    .slice(0, 12)}`
  try {
    return readdirSync(dir).filter((name) => name.startsWith(`${stem}.`))
  } catch {
    return []
  }
}

/**
 * The header of an index's vector file (`ai/vector_store.rs`): 8 bytes of
 * magic, the header's length as a little-endian u32, the header as JSON; the
 * vectors follow.
 */
function readStoreHeader(file: string) {
  const bytes = readFileSync(file)
  const length = bytes.readUInt32LE(8)
  return JSON.parse(bytes.subarray(12, 12 + length).toString('utf8')) as {
    embeddedText?: string
  } & Record<string, unknown>
}

/**
 * The text format a rebuild has stamped so far, or null while there is none to
 * read: a rebuild deletes the vector file and writes it again.
 */
function rebuiltTextFormat(file: string) {
  try {
    return readStoreHeader(file).embeddedText ?? null
  } catch {
    return null
  }
}

/** Puts `header` in place of the file's own, keeping its vectors. */
function writeStoreHeader(file: string, header: Record<string, unknown>) {
  const bytes = readFileSync(file)
  const json = Buffer.from(JSON.stringify(header), 'utf8')
  const length = Buffer.alloc(4)
  length.writeUInt32LE(json.length)
  const vectors = bytes.subarray(12 + bytes.readUInt32LE(8))
  writeFileSync(
    `${file}.e2e`,
    Buffer.concat([bytes.subarray(0, 8), length, json, vectors]),
  )
  renameSync(`${file}.e2e`, file)
}

/** The progress line's counts: "N of M visits", then pace once measured. */
const COUNTS = /^([\d,]+) of ([\d,]+) visits/

/** What the first test searches for; every other text sent is a page's. */
const SEARCHES = new Set(['afternoon run strava', 'quokka harbour regatta'])

/**
 * The pages behind the texts sent for embedding. A page's text starts with
 * its title, then its site and the words of its address: those two lines
 * tell the fixture's pages apart.
 */
const pagesOf = (texts: string[]) =>
  texts
    .filter((text) => !SEARCHES.has(text))
    .map((text) => text.split('\n').slice(0, 2).join('\n'))

/** Field labels the page text used to carry, with the visit's time and profile. */
const OLD_LABELS = /^(Profile|Visited at|URL|Domain|Title|Readable text):/m
const RUN_URL = 'https://www.strava.com/activities/1001'
const RUN_TITLE = 'Strava · Afternoon run'

test('a search index build killed half way finishes after a restart, without starting over', async ({
  page,
}, testInfo) => {
  test.setTimeout(600_000)
  // The build's first batch; saving the service sends nothing.
  const service = await startFakeEmbeddings({
    holdAfter: 1,
    models: [
      {
        id: 'e2e-embed',
        type: 'embeddings',
        state: 'loaded',
        max_context_length: 2048,
        loaded_context_length: 2048,
      },
      {
        id: 'e2e-embed-next',
        type: 'embeddings',
        state: 'not-loaded',
        max_context_length: 2048,
      },
    ],
  })
  const before = await loadConfig()
  // What the queue and the service saw, kept with the report whatever happens.
  const record = async (name: string) =>
    testInfo.attach(name, {
      body: JSON.stringify(
        {
          queue: await backend('load_ai_queue_status'),
          index: (await backend<{ aiStatus: unknown }>('app_snapshot'))
            .aiStatus,
          embedded: service.inputs.length,
          held: service.held(),
        },
        null,
        2,
      ),
      contentType: 'application/json',
    })
  let finished = false
  try {
    // Settings → AI: an embeddings service on this computer makes the index.
    await page.goto('/#/settings/ai')
    await unlockIfLocked(page)
    await page.getByRole('combobox', { name: 'Model' }).click()
    await page
      .getByRole('option', { name: 'Add an embeddings service…' })
      .click()
    const dialog = page.getByRole('dialog', {
      name: 'Add an embeddings service',
    })
    await dialog.getByRole('combobox', { name: 'Service' }).click()
    await page.getByRole('option', { name: 'OpenAI or compatible' }).click()
    await dialog.getByRole('textbox', { name: 'Address' }).fill(service.url)
    await dialog.getByRole('textbox', { name: 'Model' }).fill('e2e-embed')
    await dialog.getByRole('textbox', { name: 'Name' }).fill('E2E embeddings')
    // Adding it switches semantic search to it: the dialog says what that means.
    const adding = dialog.getByRole('note')
    await expect(adding).toContainText(
      'Saving switches semantic search to this model:',
    )
    await expect(adding).toContainText(
      /e2e-embed needs its own index: [\d,]+ visits, read in the background once semantic search is on\./,
    )
    await expect(adding).toContainText(
      'From then on every semantic search goes to E2E embeddings. When it isn’t running or can’t be reached, semantic search doesn’t work and no other model stands in; keyword search still works.',
    )
    await dialog.getByRole('button', { name: 'Add service' }).click()
    await expect(page.getByText('E2E embeddings is ready.')).toBeVisible()
    await expect(page.getByRole('combobox', { name: 'Model' })).toContainText(
      'E2E embeddings',
    )
    // Saving checks the model list; the model itself does not run.
    await expect(
      page.getByText(
        'E2E embeddings answers and has e2e-embed. Test connection runs the model once.',
      ),
    ).toBeVisible({ timeout: 30_000 })
    expect(service.requests(), 'saving sends nothing to embed').toBe(0)

    // Turning semantic search on builds the index with the service.
    await page.getByRole('switch', { name: 'Semantic search' }).click()

    // Half way: the first batch is saved and the build waits on the next.
    await record('queue after turning it on')
    await expect
      .poll(() => service.held(), { timeout: 120_000 })
      .toBeGreaterThan(0)
    const progress = page.getByText(COUNTS)
    const readCounts = async () => {
      const counts = (await progress.innerText().catch(() => '')).match(COUNTS)
      return counts
        ? { done: parseCount(counts[1]), total: parseCount(counts[2]) }
        : { done: 0, total: 0 }
    }
    await expect
      .poll(async () => (await readCounts()).done, { timeout: 30_000 })
      .toBeGreaterThan(0)
    const { done, total } = await readCounts()
    expect(done, 'not done yet').toBeLessThan(total)

    // Killed like a crash: it carries on after the restart.
    const embeddedBefore = new Set(pagesOf(service.inputs))
    const atKill = service.inputs.length
    await killAndRestartApp()
    // Answers one more batch, then holds again, so the pause lands half way.
    service.release()
    service.holdAfter(1)

    await page.goto('/#/settings/ai')
    await page.reload()
    await unlockIfLocked(page)
    await expect
      .poll(() => service.held(), { timeout: 120_000 })
      .toBeGreaterThan(0)
    expect(
      pagesOf(service.inputs.slice(atKill)).length,
      'the build went on after the restart',
    ).toBeGreaterThan(0)

    // Paused half way, it stays paused through a restart and a backup.
    await page.getByRole('button', { name: 'Pause' }).click()
    await expect(
      page.getByText(/^Paused at [\d,]+ of [\d,]+ visits\.$/),
    ).toBeVisible({ timeout: 30_000 })
    await record('queue after pausing')
    await killAndRestartApp()
    const atPause = service.requests()
    await page.goto('/#/settings/ai')
    await page.reload()
    await unlockIfLocked(page)
    await expect(
      page.getByText(/^Paused at [\d,]+ of [\d,]+ visits\.$/),
    ).toBeVisible({ timeout: 60_000 })
    await page.goto('/#/backup')
    await page.getByRole('button', { name: 'Back up now' }).first().click()
    await expect(
      page
        .getByRole('heading', { name: 'Recent runs' })
        .locator('xpath=ancestor::*[.//li][1]')
        .getByRole('listitem')
        .first(),
    ).toContainText(/\b0 new visits/, { timeout: 60_000 })
    // Long enough for an after-backup update to have sent its first batch.
    await page.waitForTimeout(5_000)
    await record('queue after the backup while paused')
    expect(service.requests(), 'paused: nothing sent').toBe(atPause)
    expect((await queueStatus()).semanticIndex.build).toMatchObject({
      state: 'paused',
      pausedBy: 'user',
    })

    // Continue goes on with the paused build; cancelled half way, a new
    // build carries the index on: nothing is wiped, no page is sent again.
    service.release()
    service.holdAfter(1)
    await page.goto('/#/settings/ai')
    await page.getByRole('button', { name: 'Continue' }).click()
    await expect
      .poll(() => service.held(), { timeout: 120_000 })
      .toBeGreaterThan(0)
    await page.getByRole('button', { name: 'Cancel', exact: true }).click()
    await expect(
      page.getByText(
        /^The index made with e2e-embed is not finished: [\d,]+ visits? so far\.$/,
      ),
    ).toBeVisible({ timeout: 30_000 })
    await record('queue after cancelling')
    service.release()
    await page.getByRole('button', { name: 'Continue' }).click()
    await expect(
      page.getByText(`${total.toLocaleString('en-US')} visits indexed`, {
        exact: false,
      }),
    ).toBeVisible({ timeout: 240_000 })
    const sentTwice = Object.entries(
      pagesOf(service.inputs).reduce<Record<string, number>>((seen, page) => {
        seen[page] = (seen[page] ?? 0) + 1
        return seen
      }, {}),
    ).filter(([, times]) => times > 1)
    expect(sentTwice, 'no page sent twice').toEqual([])
    expect(
      [...embeddedBefore].every((page) =>
        pagesOf(service.inputs).includes(page),
      ),
    ).toBe(true)

    // The search modes show once there is something to search for.
    await page.goto('/#/history')
    await page
      .getByRole('searchbox', { name: 'Search history' })
      .fill('afternoon run strava')
    await page.getByRole('radio', { name: 'Semantic' }).click()
    const rows = page
      .getByRole('listbox', { name: 'History results' })
      .getByRole('option')
    await expect(rows.first()).toContainText(RUN_TITLE, { timeout: 60_000 })
    const day = 86_400_000
    for (const [label, at] of [
      ['Today', Date.now()],
      ['Yesterday', Date.now() - day],
    ] as const) {
      await page.getByRole('button', { name: /^Date filter:/ }).click()
      await page.getByRole('menuitemradio', { name: label }).click()
      const visited = archivedVisits().some(
        (visit) => visit.url === RUN_URL && localDay(visit.at) === localDay(at),
      )
      const run = rows.filter({ hasText: RUN_TITLE })
      if (visited) await expect(run.first()).toBeVisible({ timeout: 30_000 })
      else await expect(run).toHaveCount(0, { timeout: 30_000 })
    }

    // A later backup: one indexed page visited again as it was, another
    // under a new title (Chrome keeps one title per address).
    const chromeFile = fixture().files['chrome:Default']
    const pages = fixture().visitsByProfile['chrome:Default'].filter(
      (visit) => !visit.term,
    )
    const same = pages[0]
    const retitled = pages.find((visit) => visit.url !== same.url)!
    const atBackup = service.inputs.length
    retitleChromePage(chromeFile, retitled.url, RETITLED_TITLE)
    appendChromeVisits(chromeFile, indexRevisitVisits(same, retitled))
    await page.goto('/#/backup')
    await page.getByRole('button', { name: 'Back up now' }).first().click()
    const titleOf = (page: string) => page.split('\n')[0]
    await expect
      .poll(() => pagesOf(service.inputs.slice(atBackup)).map(titleOf), {
        timeout: 120_000,
      })
      .toContain(RETITLED_TITLE)
    await expect
      .poll(
        async () => {
          const queue = await backend<{ queued: number; running: number }>(
            'load_ai_queue_status',
          )
          return queue.queued + queue.running
        },
        { timeout: 120_000 },
      )
      .toBe(0)
    // Storage counts the index where it is, and anything left in the old
    // sidecar folder.
    const root = fixture().dirs.projectRoot
    const indexBytes =
      folderBytes(path.join(root, 'derived', 'vectors')) +
      folderBytes(path.join(root, 'sidecars', 'semantic-index'))
    expect(indexBytes, 'the index is on disk').toBeGreaterThan(0)
    const { storage } = await backend<{
      storage: { semanticSidecarBytes: number }
    }>('load_dashboard_snapshot')
    expect(storage.semanticSidecarBytes).toBe(indexBytes)
    const sent = pagesOf(service.inputs.slice(atBackup))
    await testInfo.attach('pages sent after the backup', {
      body: JSON.stringify(sent, null, 2),
      contentType: 'application/json',
    })
    // Only the retitled page: the one visited again as it was is mapped to
    // what the index already holds.
    expect([...new Set(sent)].map(titleOf)).toEqual([RETITLED_TITLE])
    expect(sent[0].split('\n')[1].split(' · ')[0], 'its site').toBe(
      new URL(retitled.url).hostname.replace(/^www\./, ''),
    )
    await page.goto('/#/history')
    await page
      .getByRole('searchbox', { name: 'Search history' })
      .fill('quokka harbour regatta')
    await page.getByRole('radio', { name: 'Semantic' }).click()
    await expect(rows.first()).toContainText(RETITLED_TITLE, {
      timeout: 60_000,
    })

    // Another model: nothing is built, and search keeps the old index.
    const atSwitch = service.requests()
    await page.goto('/#/settings/ai')
    // The semantic search row's service, not the assistant's below it.
    await page.getByRole('button', { name: 'Edit' }).first().click()
    const edit = page.getByRole('dialog', { name: 'Edit E2E embeddings' })
    await edit.getByRole('textbox', { name: 'Model' }).fill('e2e-embed-next')
    // Before saving: what the switch keeps, what it deletes and when.
    const switching = edit.getByRole('note')
    await expect(switching).toContainText(
      /e2e-embed-next needs its own index: [\d,]+ visits, read in the background once you choose Build index\. Nothing starts before that\./,
    )
    await expect(switching).toContainText(
      'Until it is complete, semantic search keeps using e2e-embed.',
    )
    await expect(switching).toContainText(
      'Once it is complete, the index made with e2e-embed is deleted. Going back to e2e-embed means building that index again.',
    )
    await expect(switching).toContainText(
      'From then on every semantic search goes to E2E embeddings.',
    )
    await edit.getByRole('button', { name: 'Save', exact: true }).click()
    await expect(
      page.getByText(/^e2e-embed-next has no index yet\./),
    ).toBeVisible({ timeout: 30_000 })
    // Said again next to the button that starts the build.
    await expect(
      page.getByText(
        'Until it is complete, semantic search keeps using e2e-embed. Once it is complete, the index made with e2e-embed is deleted. Going back to e2e-embed means building that index again.',
      ),
    ).toBeVisible()
    await page.waitForTimeout(3_000)
    expect(service.requests(), 'changing the model builds nothing').toBe(
      atSwitch,
    )
    expect((await queueStatus()).semanticIndex).toMatchObject({
      chosenComplete: false,
      serving: { model: 'e2e-embed' },
      build: null,
    })
    await page.goto('/#/history')
    await page
      .getByRole('searchbox', { name: 'Search history' })
      .fill('afternoon run strava')
    await page.getByRole('radio', { name: 'Semantic' }).click()
    await expect(rows.first()).toContainText(RUN_TITLE, { timeout: 60_000 })

    // Built: search reads the new index and the old one's files are gone.
    const { providerId } = (await queueStatus()).semanticIndex.chosen!
    expect(indexFilesOf(providerId, 'e2e-embed').length).toBeGreaterThan(0)
    expect(indexFilesOf(providerId, 'e2e-embed-next')).toEqual([])
    await page.goto('/#/settings/ai')
    await page.getByRole('button', { name: 'Build index' }).click()
    await expect
      .poll(async () => (await queueStatus()).semanticIndex.chosenComplete, {
        timeout: 240_000,
      })
      .toBe(true)
    await expect
      .poll(() => indexFilesOf(providerId, 'e2e-embed'), { timeout: 60_000 })
      .toEqual([])
    expect(indexFilesOf(providerId, 'e2e-embed-next').length).toBeGreaterThan(0)
    expect((await queueStatus()).semanticIndex.serving).toMatchObject({
      model: 'e2e-embed-next',
    })

    // Back to the in-app model from the picker: nothing changes until the
    // user has read what the switch does and accepted it.
    const picker = page.getByRole('combobox', { name: 'Model' })
    const choose = async () => {
      await picker.click()
      await page.getByRole('option', { name: 'In-app static model' }).click()
      return page.getByRole('alertdialog', {
        name: 'Switch semantic search to the in-app static model?',
      })
    }
    let confirm = await choose()
    await expect(confirm).toContainText(
      /potion-multilingual-128M needs its own index: [\d,]+ visits, read in the background once you choose Build index\./,
    )
    await expect(confirm).toContainText(
      'Until it is complete, semantic search keeps using e2e-embed-next.',
    )
    await expect(confirm).toContainText(
      'Once it is complete, the index made with e2e-embed-next is deleted. Going back to e2e-embed-next means building that index again.',
    )
    await expect(confirm).toContainText(
      'It is a static embedding model: very fast, but it finds pages by meaning less well than the usual contextual embedding models.',
    )
    await expect(confirm).toContainText(
      /Building its index first downloads it once, about [\d.]+ MB\./,
    )
    await testInfo.attach('switching back to the in-app model', {
      body: await confirm.screenshot(),
      contentType: 'image/png',
    })
    await confirm.getByRole('button', { name: 'Cancel' }).click()
    await expect(confirm).toBeHidden()
    await expect(picker).toContainText('E2E embeddings')
    expect(
      (await queueStatus()).semanticIndex.chosen,
      'cancelling keeps the choice',
    ).toMatchObject({ providerId, model: 'e2e-embed-next' })

    const atStatic = service.requests()
    confirm = await choose()
    await confirm.getByRole('button', { name: 'Switch model' }).click()
    await expect(picker).toContainText('In-app static model')
    await expect
      .poll(async () => (await queueStatus()).semanticIndex.chosen?.providerId)
      .toBe('static-in-app')
    // Choosing deletes and builds nothing: search keeps the complete index.
    expect((await queueStatus()).semanticIndex).toMatchObject({
      chosenComplete: false,
      serving: { providerId, model: 'e2e-embed-next' },
      build: null,
    })
    expect(indexFilesOf(providerId, 'e2e-embed-next').length).toBeGreaterThan(0)
    expect(service.requests(), 'switching sends nothing').toBe(atStatic)
    finished = true
  } finally {
    service.release()
    // The restarted app starts behind App Lock; the specs after this one need
    // it open, with the config as it was. If the test already failed, a
    // failing cleanup must not hide why.
    const restore = async () => {
      await unlockIfLocked(page)
      await record('queue at the end')
      const current = await loadConfig()
      await backend('save_config', { config: { ...current, ai: before.ai } })
    }
    await restore().catch(async (error: unknown) => {
      if (finished) throw error
      await testInfo.attach('cleanup failed too', { body: String(error) })
    })
    await service.close()
  }
})

test('the model gets each page’s own text and searches in its own form; an index made from the older text keeps working until it is rebuilt', async ({
  page,
}, testInfo) => {
  test.setTimeout(600_000)
  // LM Studio's name for Qwen3-Embedding, which wants an instruction on
  // searches and nothing on pages.
  const MODEL = 'text-embedding-qwen3-embedding-0.6b'
  // Holds the build after its first batch, to end it while History is open.
  const service = await startFakeEmbeddings({
    dimensions: 256,
    holdAfter: 1,
    models: [
      {
        id: MODEL,
        type: 'embeddings',
        state: 'loaded',
        max_context_length: 32768,
        loaded_context_length: 32768,
      },
    ],
  })
  const before = await loadConfig()
  const busy = async () => {
    const queue = await queueStatus()
    return queue.queued + queue.running
  }
  const rows = page
    .getByRole('listbox', { name: 'History results' })
    .getByRole('option')
  // These words find the same page by full text too: wait for the line that
  // says the results on screen came from semantic search.
  const searchByMeaning = async (words: string) => {
    await page.goto('/#/history')
    await page.getByRole('searchbox', { name: 'Search history' }).fill(words)
    await page.getByRole('radio', { name: 'Semantic' }).click()
    await expect(page.getByText(/Semantic search with /)).toBeVisible({
      timeout: 60_000,
    })
  }
  const olderText = page.getByText(
    /^This index was made from an older version of the page text/,
  )
  let finished = false
  try {
    await page.goto('/#/settings/ai')
    await unlockIfLocked(page)
    await page.getByRole('combobox', { name: 'Model' }).click()
    await page
      .getByRole('option', { name: 'Add an embeddings service…' })
      .click()
    const dialog = page.getByRole('dialog', {
      name: 'Add an embeddings service',
    })
    await dialog.getByRole('combobox', { name: 'Service' }).click()
    await page.getByRole('option', { name: 'OpenAI or compatible' }).click()
    await dialog.getByRole('textbox', { name: 'Address' }).fill(service.url)
    await dialog.getByRole('textbox', { name: 'Model' }).fill(MODEL)
    await dialog.getByRole('textbox', { name: 'Name' }).fill('E2E Qwen3')
    await dialog.getByRole('button', { name: 'Add service' }).click()
    await expect(page.getByRole('combobox', { name: 'Model' })).toContainText(
      'E2E Qwen3',
    )
    await page.getByRole('switch', { name: 'Semantic search' }).click()
    await expect
      .poll(() => service.held(), { timeout: 120_000 })
      .toBeGreaterThan(0)

    // The build ends while History is open: semantic search becomes
    // available there without a reload.
    await page.goto('/#/history')
    // The search modes show once there is something to search for.
    await page
      .getByRole('searchbox', { name: 'Search history' })
      .fill('afternoon run strava')
    const semanticMode = page.getByRole('radio', { name: 'Semantic' })
    await expect(semanticMode).toBeDisabled()
    service.release()
    await expect(semanticMode).toBeEnabled({ timeout: 240_000 })
    expect((await queueStatus()).semanticIndex.chosenComplete).toBe(true)
    await expect.poll(busy, { timeout: 120_000 }).toBe(0)

    // Each page's own text, title first: no visit time, profile or labels,
    // and no instruction (Qwen3 reads pages as they are).
    const pages = [...service.inputs]
    await testInfo.attach('texts sent to build the index', {
      body: JSON.stringify(pages, null, 2),
      contentType: 'application/json',
    })
    expect(pages.length).toBeGreaterThan(0)
    expect(
      pages.filter(
        (text) => OLD_LABELS.test(text) || text.startsWith('Instruct:'),
      ),
    ).toEqual([])
    expect(pages).toContainEqual(
      expect.stringMatching(
        /^Strava · Afternoon run\nstrava\.com · activities(\n|$)/,
      ),
    )

    // A search carries the instruction, once.
    const atSearch = service.inputs.length
    await searchByMeaning('afternoon run strava')
    await expect(rows.first()).toContainText(RUN_TITLE, { timeout: 60_000 })
    await expect
      .poll(() => service.inputs.length - atSearch, { timeout: 30_000 })
      .toBeGreaterThan(0)
    const searches = service.inputs.slice(atSearch)
    for (const text of searches)
      expect(text).toMatch(/^Instruct: [^\n]+\nQuery:afternoon run strava$/)

    // The index file as PathKeep wrote it before the page text changed.
    const { providerId } = (await queueStatus()).semanticIndex.chosen!
    const [vectors] = indexFilesOf(providerId, MODEL).filter((name) =>
      name.endsWith('.pkvec'),
    )
    const store = path.join(
      fixture().dirs.projectRoot,
      'derived',
      'vectors',
      vectors,
    )
    const { embeddedText, ...older } = readStoreHeader(store)
    expect(embeddedText, 'the index names the text it was made from').toEqual(
      expect.any(String),
    )
    writeStoreHeader(store, older)

    // It still works, and both places say what it was made from.
    await page.goto('/#/settings/ai')
    await page.reload()
    await unlockIfLocked(page)
    await expect(olderText).toBeVisible({ timeout: 30_000 })
    await searchByMeaning('afternoon run strava')
    await expect(rows.first()).toContainText(RUN_TITLE, { timeout: 60_000 })
    await expect(
      page.getByText('it was made from an older version of the page text', {
        exact: false,
      }),
    ).toBeVisible()

    // A backup's new page is still added to it.
    const atBackup = service.inputs.length
    appendChromeVisits(fixture().files['chrome:Default'], [newPageVisit()])
    await page.goto('/#/backup')
    await page.getByRole('button', { name: 'Back up now' }).first().click()
    await expect
      .poll(
        () =>
          service.inputs
            .slice(atBackup)
            .some((text) =>
              text.startsWith(
                `${NEW_PAGE.title}\nbakery.example.org · notes sourdough starter feeding schedule`,
              ),
            ),
        { timeout: 120_000 },
      )
      .toBe(true)
    await expect.poll(busy, { timeout: 120_000 }).toBe(0)
    expect(
      readStoreHeader(store).embeddedText,
      'adding pages does not make it current',
    ).toBeUndefined()
    await searchByMeaning('sourdough starter feeding')
    await expect(rows.first()).toContainText(NEW_PAGE.title, {
      timeout: 60_000,
    })

    // Rebuilt, every page is read again the current way and the note goes.
    await page.goto('/#/settings/ai')
    await expect(olderText).toBeVisible({ timeout: 30_000 })
    const atRebuild = service.inputs.length
    await page.getByRole('button', { name: 'Rebuild index' }).click()
    await page
      .getByRole('alertdialog', { name: 'Rebuild the search index?' })
      .getByRole('button', { name: 'Rebuild' })
      .click()
    await expect
      .poll(() => rebuiltTextFormat(store), { timeout: 240_000 })
      .toBe(embeddedText)
    await expect
      .poll(
        async () =>
          (await queueStatus()).semanticIndex.chosenComplete &&
          (await busy()) === 0,
        { timeout: 240_000 },
      )
      .toBe(true)
    const rebuilt = service.inputs.slice(atRebuild)
    expect(pages.filter((text) => !rebuilt.includes(text))).toEqual([])
    expect(rebuilt.filter((text) => OLD_LABELS.test(text))).toEqual([])
    await page.reload()
    await unlockIfLocked(page)
    await expect(page.getByText(/ visits? indexed/)).toBeVisible({
      timeout: 30_000,
    })
    await expect(olderText).toHaveCount(0)
    finished = true
  } finally {
    const restore = async () => {
      await unlockIfLocked(page)
      const current = await loadConfig()
      await backend('save_config', { config: { ...current, ai: before.ai } })
    }
    await restore().catch(async (error: unknown) => {
      if (finished) throw error
      await testInfo.attach('cleanup failed too', { body: String(error) })
    })
    await service.close()
  }
})
