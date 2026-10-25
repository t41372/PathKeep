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
 *   the note goes away. Paused half way, the old index still answers
 *   searches, pages the rebuild has not read yet included, and Settings says
 *   so; continued, the new index replaces it and only its files remain. Rebuild
 *   search index does the same for an index made the current way.
 *
 * The app is killed for real (`support/app.ts`; the E2E launcher starts it
 * again). The service runs on this computer (`support/fake-embeddings.ts`);
 * the in-app model is a 531 MB download. It answers the build's first
 * request and holds the next, so the kill lands half way every time, and is
 * told to hold again for the pause.
 */
import { createHash } from 'node:crypto'
import {
  mkdirSync,
  readFileSync,
  readdirSync,
  renameSync,
  statSync,
  writeFileSync,
} from 'node:fs'
import path from 'node:path'
import { expect, test, type Page } from '@playwright/test'
import {
  appendChromeVisits,
  retitleChromePage,
  writeChromeHistory,
} from '../../scripts/fixtures/synthetic-browsers.mjs'
import { killAndRestartApp, unlockIfLocked } from './support/app'
import { addLmStudioService } from './support/ai-service'
import {
  lastUserText,
  offersTools,
  startFakeModel,
  toolResults,
} from './support/fake-model'
import {
  fakeEmbeddingVector,
  startFakeEmbeddings,
} from './support/fake-embeddings'
import {
  archivedVisits,
  backend,
  fixture,
  localDay,
  parseCount,
  type FixtureVisit,
} from './support/fixture'
import {
  NEW_PAGE,
  PAUSED_REBUILD_PAGE,
  RETITLED_TITLE,
  indexRevisitVisits,
  newPageVisit,
  pausedRebuildPageVisit,
} from './support/index-revisit'

import {
  RELATED_SOURCE,
  RELATED_LATE,
  RELATED_HIDDEN,
  relatedVisits,
  relatedVector,
  importRelatedFile,
  undoRelatedFile,
} from './support/related-pages'
import { searchGroupsScenario } from './support/search-groups'
import { IN_TITLE } from './support/ranking-pages'
import { insightsSearchGroupsScenario } from './support/insights-search-groups'
import type { RelatedPages } from '../../src/lib/backend-client/related-pages'

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

/** An index's name on disk, as `ai_sidecar::provider_table_name` writes it. */
const indexStem = (providerId: string, model: string) =>
  `pathkeep_${providerId.replace(/[^A-Za-z0-9]/g, '_')}_${createHash('sha256')
    .update(`${providerId}::${model}`)
    .digest('hex')
    .slice(0, 12)}`

/** The folder of an index in `derived/vectors`, one folder per lineage in it. */
const indexDir = (providerId: string, model: string) =>
  path.join(
    fixture().dirs.projectRoot,
    'derived',
    'vectors',
    indexStem(providerId, model),
  )

const subfolders = (dir: string) => {
  try {
    return readdirSync(dir, { withFileTypes: true })
      .filter((entry) => entry.isDirectory())
      .map((entry) => entry.name)
  } catch {
    return []
  }
}

/**
 * The lineages of an index (`semantic-index-control.md`): one while it is
 * idle, two while a full rebuild builds beside it.
 */
const lineagesOf = (providerId: string, model: string) =>
  subfolders(indexDir(providerId, model))

/** A lineage's generations, `g<run>` folders, oldest first. */
const generationsOf = (providerId: string, model: string, lineage: string) =>
  subfolders(path.join(indexDir(providerId, model), lineage))
    .filter((name) => /^g\d+$/.test(name))
    .sort((a, b) => Number(a.slice(1)) - Number(b.slice(1)))

/**
 * The index search reads: the newest generation of any lineage (a rebuild's
 * lineage has none until it is done), its `serving.json`, and its lineage's
 * page vector file.
 */
function servingOf(providerId: string, model: string) {
  const [newest] = lineagesOf(providerId, model)
    .flatMap((lineage) =>
      generationsOf(providerId, model, lineage).map((generation) => ({
        lineage,
        generation,
      })),
    )
    .sort(
      (a, b) => Number(b.generation.slice(1)) - Number(a.generation.slice(1)),
    )
  if (!newest) return null
  const dir = path.join(indexDir(providerId, model), newest.lineage)
  return {
    ...newest,
    store: path.join(dir, `${indexStem(providerId, model)}.pkvec`),
    serving: path.join(dir, newest.generation, 'serving.json'),
  }
}

/** Adds an OpenAI-compatible embeddings service in Settings → AI and picks it. */
async function addEmbeddingsService(
  page: Page,
  url: string,
  model: string,
  name: string,
) {
  await page.goto('/#/settings/ai')
  await unlockIfLocked(page)
  await page.getByRole('combobox', { name: 'Model' }).click()
  await page.getByRole('option', { name: 'Add an embeddings service…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Add an embeddings service' })
  await dialog.getByRole('combobox', { name: 'Service' }).click()
  await page.getByRole('option', { name: 'OpenAI or compatible' }).click()
  await dialog.getByRole('textbox', { name: 'Address' }).fill(url)
  await dialog.getByRole('textbox', { name: 'Model' }).fill(model)
  await dialog.getByRole('textbox', { name: 'Name' }).fill(name)
  await dialog.getByRole('button', { name: 'Add service' }).click()
  await expect(page.getByRole('combobox', { name: 'Model' })).toContainText(
    name,
  )
}

/** Files of the index `model` made with service `providerId`, in every lineage and generation. */
const indexFilesOf = (providerId: string, model: string) => {
  const list = (dir: string): string[] => {
    try {
      return readdirSync(dir, { withFileTypes: true }).flatMap((entry) =>
        entry.isDirectory()
          ? list(path.join(dir, entry.name)).map((name) =>
              path.join(entry.name, name),
            )
          : [entry.name],
      )
    } catch {
      return []
    }
  }
  return list(indexDir(providerId, model))
}

/**
 * Makes the serving index look like one made from the older page text, as an
 * index PathKeep wrote before the text changed: its vector file's header and
 * the generation's copy of it name no text format.
 */
function markOlderText(serving: { store: string; serving: string }) {
  const { embeddedText, ...older } = readStoreHeader(serving.store)
  writeStoreHeader(serving.store, older)
  const metadata = JSON.parse(readFileSync(serving.serving, 'utf8')) as {
    pages?: Record<string, unknown>
  }
  if (metadata.pages) delete metadata.pages.embeddedText
  writeFileSync(serving.serving, JSON.stringify(metadata))
  return embeddedText
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
 * The text format the index search reads was made from, or null when its
 * header names none (or there is no file to read).
 */
function rebuiltTextFormat(file: string) {
  try {
    return readStoreHeader(file).embeddedText ?? null
  } catch {
    return null
  }
}

/**
 * Once a build has ended, the index keeps one lineage with one generation:
 * its sources beside the generation search reads, nothing else.
 */
async function expectOneGeneration(providerId: string, model: string) {
  await expect
    .poll(() => lineagesOf(providerId, model), { timeout: 30_000 })
    .toHaveLength(1)
  const [lineage] = lineagesOf(providerId, model)
  await expect
    .poll(() => generationsOf(providerId, model, lineage), { timeout: 30_000 })
    .toHaveLength(1)
  const files = indexFilesOf(providerId, model)
  const stem = indexStem(providerId, model)
  const [generation] = generationsOf(providerId, model, lineage)
  for (const name of [`${stem}.pkvec`, `${stem}.pkmap`])
    expect(files).toContain(path.join(lineage, name))
  for (const extension of ['.pkbin', '.pki8', '.pkrev', '.pkfwd'])
    expect(files).toContain(
      path.join(lineage, generation, `${stem}${extension}`),
    )
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

/**
 * The page the later backup gives a new title: not the first page (that one
 * is visited again as it was), and never the run page, which the searches
 * after the backup look for by its old text and title. Which page comes first
 * depends on the fixture's visits, so the run page could otherwise be picked.
 */
function retitledPage(pages: FixtureVisit[]) {
  return pages.find(
    (visit) => visit.url !== pages[0].url && visit.url !== RUN_URL,
  )!
}

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
    const retitled = retitledPage(pages)
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
    await unlockIfLocked(page)
    await page.getByRole('searchbox', { name: 'Search history' }).fill(words)
    await page.getByRole('radio', { name: 'Semantic' }).click()
    await expect(page.getByText(/Semantic search with /)).toBeVisible({
      timeout: 60_000,
    })
  }
  // Semantic search merges in full-text results, so a page listed for words
  // it holds proves nothing about the index: ask why it matched. `semantic`
  // or `lexical+semantic` means the index found it.
  const matchedByMeaning = async (words: string, url: string) => {
    const { items } = await backend<{
      items: { url: string; matchReason: string }[]
    }>('search_ai_history', { request: { query: words, limit: 20 } })
    return items.find((item) => item.url === url)?.matchReason ?? 'not found'
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

    // The index as PathKeep wrote it before the page text changed.
    const { providerId } = (await queueStatus()).semanticIndex.chosen!
    const store = () => servingOf(providerId, MODEL)?.store ?? ''
    const embeddedText = markOlderText(servingOf(providerId, MODEL)!)
    expect(embeddedText, 'the index names the text it was made from').toEqual(
      expect.any(String),
    )

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
      readStoreHeader(store()).embeddedText,
      'adding pages does not make it current',
    ).toBeUndefined()
    await searchByMeaning('sourdough starter feeding')
    await expect(rows.first()).toContainText(NEW_PAGE.title, {
      timeout: 60_000,
    })

    // Rebuilt, every page is read again the current way and the note goes.
    // It reads the visits oldest first, so the backup's new page comes last:
    // paused after the first batch, the rebuild has not read it.
    await page.goto('/#/settings/ai')
    await expect(olderText).toBeVisible({ timeout: 30_000 })
    const atRebuild = service.inputs.length
    service.holdAfter(1)
    await page.getByRole('button', { name: 'Rebuild index' }).click()
    await page
      .getByRole('alertdialog', { name: 'Rebuild the search index?' })
      .getByRole('button', { name: 'Rebuild' })
      .click()
    await expect
      .poll(() => service.held(), { timeout: 120_000 })
      .toBeGreaterThan(0)
    await page.getByRole('button', { name: 'Pause' }).click()
    await expect(
      page.getByText(/^Paused at [\d,]+ of [\d,]+ visits\.$/),
    ).toBeVisible({ timeout: 30_000 })
    service.release()

    // Meanwhile search reads the old index, which the rebuild writes beside,
    // never into, and Settings says so.
    await expect(
      page.getByText(
        /^Until this finishes, search uses the index you have now: [\d,]+ pages?\.$/,
      ),
    ).toBeVisible()
    expect(rebuiltTextFormat(store()), 'the old index is still in place').toBe(
      null,
    )
    expect(
      lineagesOf(providerId, MODEL),
      'the rebuild writes a lineage of its own',
    ).toHaveLength(2)
    expect((await queueStatus()).semanticIndex).toMatchObject({
      chosenComplete: true,
      build: { state: 'paused', pausedBy: 'user' },
    })
    await searchByMeaning('sourdough starter feeding')
    await expect(rows.first()).toContainText(NEW_PAGE.title, {
      timeout: 60_000,
    })
    // Found by the old index, not only by its words: a rebuild that emptied
    // the index first would list it by full text alone.
    expect(
      await matchedByMeaning('sourdough starter feeding', NEW_PAGE.url),
    ).toMatch(/semantic/)

    // Continued to the end, the new index takes the old one's place.
    await page.goto('/#/settings/ai')
    await unlockIfLocked(page)
    await page.getByRole('button', { name: 'Continue' }).click()
    await expect
      .poll(() => rebuiltTextFormat(store()), { timeout: 240_000 })
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
    // One lineage with one generation left once the job has ended.
    await expectOneGeneration(providerId, MODEL)
    await searchByMeaning('sourdough starter feeding')
    await expect(rows.first()).toContainText(NEW_PAGE.title, {
      timeout: 60_000,
    })
    expect(
      await matchedByMeaning('sourdough starter feeding', NEW_PAGE.url),
    ).toMatch(/semantic/)
    await page.goto('/#/settings/ai')
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

test('a browser-filtered semantic search pages through to its least similar page', async ({
  page,
}, testInfo) => {
  test.setTimeout(600_000)
  const dimensions = 256
  const service = await startFakeEmbeddings({ dimensions })
  const before = await loadConfig()
  const words = 'first seen here'
  const outsideTitle = 'First seen here'
  const firefox = fixture().visitsByProfile['firefox:k3x9.default-release']
  const newest = new Map<string, number>()
  for (const visit of firefox)
    newest.set(visit.title, Math.max(newest.get(visit.title) ?? 0, visit.at))
  expect(
    newest.size,
    'the filtered result spans more than one 50-row page',
  ).toBeGreaterThan(50)
  expect(newest.has(outsideTitle), 'the closest page is outside Firefox').toBe(
    false,
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
    await dialog.getByRole('textbox', { name: 'Model' }).fill('e2e-embed')
    await dialog
      .getByRole('textbox', { name: 'Name' })
      .fill('E2E filtered embeddings')
    await dialog.getByRole('button', { name: 'Add service' }).click()
    await expect(page.getByRole('combobox', { name: 'Model' })).toContainText(
      'E2E filtered embeddings',
    )
    const enabled = page.getByRole('switch', { name: 'Semantic search' })
    if (!(await enabled.isChecked())) await enabled.click()
    else
      await page
        .getByRole('button', { name: 'Build index', exact: true })
        .click()
    await expect
      .poll(async () => (await queueStatus()).semanticIndex.chosenComplete, {
        timeout: 240_000,
      })
      .toBe(true)

    // Compute the tail from the service's word counts, independently of search.
    // Zero overlap is exactly zero after int8 quantisation too. Among ties,
    // choose the oldest page so reaching it requires following the cursor.
    const query = fakeEmbeddingVector(words, dimensions)
    const documents = service.inputs.flatMap((text) => {
      const title = text.split('\n')[0]
      if (!newest.has(title)) return []
      const vector = fakeEmbeddingVector(text, dimensions)
      return [
        {
          title,
          score: vector.reduce((sum, value, i) => sum + value * query[i], 0),
          newest: newest.get(title)!,
        },
      ]
    })
    documents.sort((a, b) => a.score - b.score || a.newest - b.newest)
    const tail = documents[0]
    expect(tail.score, 'the tail shares no query words').toBe(0)
    expect(
      service.inputs.some((text) => text.startsWith(`${outsideTitle}\n`)),
    ).toBe(true)

    await page.goto('/#/history')
    await page.getByRole('searchbox', { name: 'Search history' }).fill(words)
    await page.getByRole('radio', { name: 'Semantic' }).click()
    await expect(page.getByText(/Semantic search with /)).toBeVisible({
      timeout: 60_000,
    })
    const list = page.getByRole('listbox', { name: 'History results' })
    const rows = list.getByRole('option')
    await expect(rows.first()).toContainText(outsideTitle, { timeout: 60_000 })
    await page.getByRole('button', { name: /^Browser filter: / }).click()
    await page
      .getByRole('menuitemradio', { name: 'Firefox', exact: true })
      .click()
    await expect(
      page.getByRole('button', { name: 'Browser filter: Firefox' }),
    ).toBeVisible()
    await expect(rows.filter({ hasText: outsideTitle })).toHaveCount(0, {
      timeout: 60_000,
    })
    const seen = new Set<string>()
    let reached = false
    for (let step = 0; step < 300; step++) {
      await expect(rows.filter({ hasText: outsideTitle })).toHaveCount(0)
      const texts = await rows.allInnerTexts()
      texts.forEach((text) => seen.add(text))
      if (texts.some((text) => text.includes(tail.title))) {
        reached = true
        break
      }
      await list.evaluate((element) => {
        element.parentElement!.scrollTop += 300
      })
      await page.waitForTimeout(100)
    }
    await testInfo.attach('filtered semantic pagination', {
      body: JSON.stringify(
        { words, tail, seen: [...seen], outsideTitle },
        null,
        2,
      ),
      contentType: 'application/json',
    })
    await testInfo.attach('least similar eligible page', {
      body: await page.screenshot(),
      contentType: 'image/png',
    })
    expect(reached, 'the least similar eligible page is reachable').toBe(true)
    expect(
      seen.size,
      'History followed at least one semantic cursor',
    ).toBeGreaterThan(50)
    expect([...seen].some((text) => text.includes(outsideTitle))).toBe(false)
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

interface TopicPage {
  historyId: number
  title: string
  url: string
  visits: number
  lastVisitLocal: string
}
interface PeriodTopic {
  id: string
  label: string
  pages: number
  visits: number
  firstVisitLocal: string
  lastVisitLocal: string
  examples: TopicPage[]
}
interface TopicsReport {
  window: { startDate: string; endDate: string; timeZone: string }
  model: string
  pages: number
  visits: number
  notIndexed: number
  topics: PeriodTopic[]
}

/**
 * The index and archive stay real: only the embeddings and chat transports are
 * scripted. Follow every text cursor and every topic, so a correct first page
 * cannot hide dropped pages, double membership, or wrong visit weights.
 */
test('Ask groups the period by meaning and every indexed page can be reached exactly once', async ({
  page,
}, testInfo) => {
  test.setTimeout(600_000)
  const before = await loadConfig()
  const dayMs = 86_400_000
  const endMs =
    new Date(`${localDay(fixture().now)}T00:00:00Z`).getTime() - 7 * dayMs
  const startMs = endMs - 6 * dayMs
  const start_date = localDay(startMs)
  const end_date = localDay(endMs)
  const visits = archivedVisits().filter(
    (visit) => visit.at >= startMs && visit.at < endMs + dayMs,
  )
  const expected = new Map<string, typeof visits>()
  for (const visit of visits) {
    const rows = expected.get(visit.url) ?? []
    rows.push(visit)
    expected.set(visit.url, rows)
  }
  // The earlier test changes Chrome's title of one page. Use a different,
  // unambiguous title for the unusable vector in this service's response.
  const chrome = fixture().visitsByProfile['chrome:Default'].filter(
    (v) => !v.term,
  )
  const retitledUrl = retitledPage(chrome).url
  const missing = [...expected].find(
    ([url, rows]) =>
      url !== retitledUrl &&
      rows.length > 1 &&
      archivedVisits()
        .filter((v) => v.title === rows[0].title)
        .every((v) => v.url === url),
  )!
  expect(missing, 'a repeated page has no usable vector').toBeTruthy()
  const [missingUrl, missingVisits] = missing
  const embeddings = await startFakeEmbeddings({
    dimensions: 256,
    vectorFor: (text, dim) =>
      text.startsWith(`${missingVisits[0].title}\n`)
        ? new Array<number>(dim).fill(0)
        : fakeEmbeddingVector(text, dim),
  })
  type Request = {
    report: string
    start_date: string
    end_date: string
    topic?: string
    output_cursor?: string
  }
  const base: Request = { report: 'topics', start_date, end_date }
  const results: { request: Request; text: string; value?: unknown }[] = []
  type Task = { request: Request; fragments: string }
  let pending: Task[] = []
  let active: Task[] = []
  let continued = 0
  const answer = 'The period topics and their pages are ready.'
  const model = await startFakeModel({
    apiKey: null,
    models: [{ id: 'e2e-topics-chat', loaded_context_length: 262_144 }],
    script: (body) => {
      if (!offersTools(body)) return { content: 'OK' }
      if (body.messages?.at(-1)?.role !== 'tool') {
        active = [{ request: base, fragments: '' }]
        pending = []
        if (lastUserText(body) !== 'Try topics before the index exists') {
          pending.push({
            request: { ...base, start_date: localDay(endMs - 39 * dayMs) },
            fragments: '',
          })
          pending.push({
            request: {
              ...base,
              start_date: '1900-01-01',
              end_date: '1900-01-07',
            },
            fragments: '',
          })
        }
      } else {
        const texts = toolResults(body).slice(-active.length)
        for (const [index, task] of active.entries()) {
          const text = texts[index]
          const json = text.slice(text.indexOf('\n') + 1)
          let value: Record<string, unknown> | undefined
          try {
            value = JSON.parse(json) as Record<string, unknown>
          } catch {
            /* A refusal remains plain text. */
          }
          if (value && 'jsonFragment' in value) {
            task.fragments += value.jsonFragment as string
            if (value.nextOutputCursor) {
              continued += 1
              task.request = {
                ...task.request,
                output_cursor: value.nextOutputCursor as string,
              }
              pending.unshift(task)
              continue
            }
            value = JSON.parse(task.fragments) as Record<string, unknown>
          }
          results.push({ request: task.request, text, value })
          if (value && Array.isArray(value.topics) && value.topics.length > 0) {
            pending.unshift(
              ...(value.topics as PeriodTopic[]).map(({ id }) => ({
                request: { ...base, topic: id },
                fragments: '',
              })),
            )
          }
        }
        active = pending.splice(0, 6)
        if (!active.length) return { content: answer }
      }
      return {
        toolCalls: active.map(({ request }) => ({
          name: 'intelligence_report',
          arguments: request,
        })),
      }
    },
  })
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
    await dialog.getByRole('textbox', { name: 'Address' }).fill(embeddings.url)
    await dialog.getByRole('textbox', { name: 'Model' }).fill('e2e-embed')
    await dialog
      .getByRole('textbox', { name: 'Name' })
      .fill('E2E topic embeddings')
    await dialog.getByRole('button', { name: 'Add service' }).click()
    await expect(page.getByRole('combobox', { name: 'Model' })).toContainText(
      'E2E topic embeddings',
    )
    await addLmStudioService(page, {
      name: 'E2E topic chat',
      address: model.url.replace(/\/v1$/, ''),
      model: 'e2e-topics-chat',
    })
    const ask = async (question: string) => {
      await page.goto('/#/ask')
      const composer = page.getByRole('textbox', { name: 'Message' })
      await composer.fill(question)
      await composer.press('Enter')
      await expect(
        page.getByRole('main').locator('aside + div').getByText(answer).last(),
      ).toBeVisible({ timeout: 120_000 })
      await expect(
        page.getByRole('button', { name: 'Send', exact: true }),
      ).toBeVisible()
    }
    await ask('Try topics before the index exists')
    expect(results[0].text).toMatch(
      /no embedding provider|semantic index is empty/,
    )
    expect(results[0].text).toContain('search_history')
    expect(results[0].text).toContain('sessions')

    await page.goto('/#/settings/ai')
    const enabled = page.getByRole('switch', { name: 'Semantic search' })
    if (!(await enabled.isChecked())) await enabled.click()
    else
      await page
        .getByRole('button', { name: 'Build index', exact: true })
        .click()
    await expect
      .poll(async () => (await queueStatus()).semanticIndex.chosenComplete, {
        timeout: 240_000,
      })
      .toBe(true)
    await expect
      .poll(
        async () => {
          const queue = await queueStatus()
          return queue.running + queue.queued
        },
        { timeout: 120_000 },
      )
      .toBe(0)
    expect(
      embeddings.inputs.some((text) =>
        text.startsWith(`${missingVisits[0].title}\n`),
      ),
    ).toBe(true)
    const embeddingRequests = embeddings.requests()
    await ask('Summarise this period and read every topic before answering')
    expect(
      embeddings.requests(),
      'topics reuses vectors without embedding again',
    ).toBe(embeddingRequests)

    const report = results.find(
      (r) => (r.value as TopicsReport | undefined)?.topics?.length,
    )?.value as TopicsReport
    expect(report, 'the model received the topic list').toBeTruthy()
    expect(report.window).toEqual({
      startDate: start_date,
      endDate: end_date,
      timeZone: 'UTC',
    })
    expect(report.model).toBe('e2e-embed')
    expect(report.pages).toBe(expected.size)
    expect(report.visits).toBe(visits.length)
    expect(report.notIndexed).toBe(1)
    expect(
      report.topics.reduce((sum, t) => sum + t.pages, report.notIndexed),
    ).toBe(expected.size)
    expect(
      report.topics.reduce((sum, t) => sum + t.visits, missingVisits.length),
    ).toBe(visits.length)
    expect(report.topics.length).toBeGreaterThan(1)
    expect(report.topics.map((t) => t.visits)).toEqual(
      report.topics.map((t) => t.visits).sort((a, b) => b - a),
    )
    expect(
      continued,
      'the model followed oversized reports to their end',
    ).toBeGreaterThan(0)
    const seen = new Set<string>()
    const weekday = (at: number) =>
      new Date(at).toLocaleDateString('en-US', {
        timeZone: 'UTC',
        weekday: 'short',
      })
    for (const topic of report.topics) {
      const drill = results.find((r) => r.request.topic === topic.id)
        ?.value as { topic: string; topicPages: TopicPage[] }
      expect(drill?.topic).toBe(topic.id)
      expect(drill.topicPages).toHaveLength(topic.pages)
      expect(drill.topicPages.map((p) => p.title)).toContain(topic.label)
      expect(drill.topicPages.map((p) => p.visits)).toEqual(
        drill.topicPages.map((p) => p.visits).sort((a, b) => b - a),
      )
      expect(drill.topicPages.reduce((sum, p) => sum + p.visits, 0)).toBe(
        topic.visits,
      )
      expect(topic.examples).toEqual(drill.topicPages.slice(0, 5))
      for (const item of drill.topicPages) {
        expect(seen.has(item.url), `${item.url} is in only one topic`).toBe(
          false,
        )
        seen.add(item.url)
        const rows = expected.get(item.url)
        expect(rows, `${item.url} belongs to the fixture window`).toBeTruthy()
        expect(item.visits).toBe(rows!.length)
        expect(item.historyId).toBeGreaterThan(0)
        const last = Math.max(...rows!.map((v) => v.at))
        const local = new Date(last)
          .toISOString()
          .slice(0, 16)
          .replace('T', ` ${weekday(last)} `)
        expect(item.lastVisitLocal).toBe(local)
      }
    }
    expect([...seen].sort()).toEqual(
      [...expected.keys()].filter((url) => url !== missingUrl).sort(),
    )
    const long = results.find(
      (r) => r.request.start_date === localDay(endMs - 39 * dayMs),
    )!
    expect(long.text).toContain('31 days')
    expect(long.text).toContain('month by month')
    const empty = results.find((r) => r.request.start_date === '1900-01-01')!
      .value as TopicsReport
    expect({
      pages: empty.pages,
      visits: empty.visits,
      notIndexed: empty.notIndexed,
      topics: empty.topics,
    }).toEqual({ pages: 0, visits: 0, notIndexed: 0, topics: [] })
    finished = true
  } finally {
    await testInfo.attach('period topic coverage', {
      body: JSON.stringify(
        { expected: [...expected], missingUrl, continued, actual: results },
        null,
        2,
      ),
      contentType: 'application/json',
    })
    const restore = async () => {
      await unlockIfLocked(page)
      const currentConfig = await loadConfig()
      await backend('save_config', {
        config: { ...currentConfig, ai: before.ai },
      })
    }
    await restore().catch(async (error: unknown) => {
      if (finished) throw error
      await testInfo.attach('cleanup failed too', { body: String(error) })
    })
    await model.close()
    await embeddings.close()
  }
})

/** Related pages reuse the published vectors, with current archive visibility and panel selection. */
test('History finds related pages across sites, folds titles and keeps the list while navigating', async ({
  page,
}, testInfo) => {
  test.setTimeout(600_000)
  const before = await loadConfig()
  const embeddings = await startFakeEmbeddings({
    dimensions: 64,
    vectorFor: relatedVector,
  })
  const folder = path.join(fixture().root, 'related-pages')
  mkdirSync(folder, { recursive: true })
  const corpus = relatedVisits(Date.now() - 120_000)
  const mainFile = path.join(folder, 'related-main.sqlite')
  const hiddenFile = path.join(folder, 'related-hidden.sqlite')
  const lateFile = path.join(folder, 'related-late.sqlite')
  const extraVisit = (item: { url: string; title: string }) => ({
    ...item,
    at: Date.now() - 1000,
    typed: true,
    duration: 1000,
  })
  writeChromeHistory(mainFile, corpus)
  writeChromeHistory(hiddenFile, [extraVisit(RELATED_HIDDEN)])
  const imported: string[] = []
  let completed = false
  try {
    await page.goto('/#/settings/ai')
    await unlockIfLocked(page)
    // Finish any previous test's index work before replacing its service.
    await expect
      .poll(async () => (await queueStatus()).running, { timeout: 120_000 })
      .toBe(0)
    // Imports must not be indexed on their own: the late page has to stay unindexed.
    const noAutoIndex = async () => {
      const current = await loadConfig()
      await backend('save_config', {
        config: {
          ...current,
          ai: {
            ...(current.ai as Record<string, unknown>),
            autoIndexAfterBackup: false,
          },
        },
      })
    }
    await noAutoIndex()
    await importRelatedFile(page, mainFile, corpus.length)
    imported.push(path.basename(mainFile))
    await importRelatedFile(page, hiddenFile, 1)
    imported.push(path.basename(hiddenFile))

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
    await dialog.getByRole('textbox', { name: 'Address' }).fill(embeddings.url)
    await dialog.getByRole('textbox', { name: 'Model' }).fill('e2e-embed')
    await dialog
      .getByRole('textbox', { name: 'Name' })
      .fill('E2E related embeddings')
    await dialog
      .getByRole('button', { name: 'Add service', exact: true })
      .click()
    await expect(page.getByRole('combobox', { name: 'Model' })).toContainText(
      'E2E related embeddings',
    )
    const enabled = page.getByRole('switch', {
      name: 'Semantic search',
      exact: true,
    })
    if (!(await enabled.isChecked())) await enabled.click()
    else
      await page
        .getByRole('button', { name: 'Build index', exact: true })
        .click()
    await expect
      .poll(async () => (await queueStatus()).semanticIndex.chosenComplete, {
        timeout: 240_000,
      })
      .toBe(true)
    await expect
      .poll(
        async () => {
          const q = await queueStatus()
          return q.running + q.queued
        },
        { timeout: 120_000 },
      )
      .toBe(0)
    expect(
      embeddings.inputs.some((text) => text.startsWith(RELATED_HIDDEN.title)),
    ).toBe(true)
    await undoRelatedFile(page, path.basename(hiddenFile))
    imported.splice(imported.indexOf(path.basename(hiddenFile)), 1)
    await noAutoIndex()
    writeChromeHistory(lateFile, [extraVisit(RELATED_LATE)])
    await importRelatedFile(page, lateFile, 1)
    imported.push(path.basename(lateFile))
    const calls = embeddings.requests()
    const read = (url: string) =>
      backend<RelatedPages>('get_related_pages', { url })

    await page.goto('/#/history')
    await unlockIfLocked(page)
    const search = page.getByRole('searchbox', { name: 'Search history' })
    await search.fill(RELATED_SOURCE.title)
    const list = page.getByRole('listbox', { name: 'History results' })
    await page
      .getByRole('main')
      .getByText(RELATED_SOURCE.title, { exact: true })
      .first()
      .click()
    const panel = page.getByRole('complementary', { name: 'Page details' })
    const related = panel.getByRole('region', { name: 'Related pages' })
    await expect(related.getByRole('button')).toHaveCount(6)
    const beforeList = await list.innerText()
    const rows = related.locator('button[data-related-url]')
    const urls = await rows.evaluateAll((nodes) =>
      nodes.map((node) => node.getAttribute('data-related-url')),
    )
    const expected = [
      'https://alpha.example.org/related/two',
      'https://alpha.example.org/related/three',
      'https://alpha.example.org/related/four',
      'https://beta.example.net/related/toolkit?page=2',
      'https://beta.example.net/related/library',
      'https://gamma.example.com/related/docs',
    ]
    expect([...urls].sort()).toEqual([...expected].sort())
    expect(
      urls.filter((url) => url?.includes('alpha.example.org')),
    ).toHaveLength(3)
    expect(new Set(urls).size).toBe(6)
    const first = await read(RELATED_SOURCE.url)
    expect(first.state).toBe('ready')
    if (first.state !== 'ready') throw new Error('Related pages unavailable')
    expect(first.items.map((row) => row.url).sort()).toEqual(
      [...expected].sort(),
    )
    await testInfo.attach('related-pages.json', {
      body: JSON.stringify(
        { source: RELATED_SOURCE, expected, actual: first },
        null,
        2,
      ),
      contentType: 'application/json',
    })
    await related.scrollIntoViewIfNeeded()
    await testInfo.attach('related-pages-panel.png', {
      body: await panel.screenshot(),
      contentType: 'image/png',
    })

    await related
      .getByRole('button', {
        name: 'Show details for Related subject A toolkit | Board',
        exact: true,
      })
      .click()
    await expect(panel.getByRole('heading', { level: 2 })).toHaveText(
      'Related subject A toolkit | Board',
    )
    await expect(
      related.getByRole('button', {
        name: `Show details for ${RELATED_SOURCE.title}`,
        exact: true,
      }),
    ).toBeVisible()
    await expect(search).toHaveValue(RELATED_SOURCE.title)
    expect(await list.innerText()).toBe(beforeList)
    await related
      .getByRole('button', {
        name: `Show details for ${RELATED_SOURCE.title}`,
        exact: true,
      })
      .click()
    await expect(panel.getByRole('heading', { level: 2 })).toHaveText(
      RELATED_SOURCE.title,
    )

    await search.fill(RELATED_LATE.title)
    await page
      .getByRole('main')
      .getByText(RELATED_LATE.title, { exact: true })
      .first()
      .click()
    await expect(
      related.getByText('This page hasn’t been indexed yet.', { exact: true }),
    ).toBeVisible()
    expect((await read(RELATED_LATE.url)).state).toBe('notIndexed')
    expect(
      embeddings.requests(),
      'opening related pages never sends text to the service',
    ).toBe(calls)

    await page.goto('/#/settings/ai')
    await unlockIfLocked(page)
    const details = page.getByRole('button', { name: 'Remove', exact: true })
    // There is one Remove button for the selected embedding service; LLM controls live elsewhere.
    await details.first().click()
    const remove = page.getByRole('alertdialog', {
      name: 'Remove E2E related embeddings?',
    })
    await remove.getByRole('button', { name: 'Remove', exact: true }).click()
    await expect(remove).toBeHidden()
    await page.goto('/#/history')
    await unlockIfLocked(page)
    await page
      .getByRole('searchbox', { name: 'Search history' })
      .fill(RELATED_SOURCE.title)
    await page
      .getByRole('main')
      .getByText(RELATED_SOURCE.title, { exact: true })
      .first()
      .click()
    await expect(
      page
        .getByRole('complementary', { name: 'Page details' })
        .getByRole('region', { name: 'Related pages' }),
    ).toHaveCount(0)
    expect((await read(RELATED_SOURCE.url)).state).toBe('unavailable')
    completed = true
  } finally {
    if (completed) {
      for (const filename of imported.reverse())
        await undoRelatedFile(page, filename)
      const current = await loadConfig()
      await backend('save_config', { config: { ...current, ai: before.ai } })
    }
    await embeddings.close()
  }
})

/** Searches use their own vectors and actual trail pages, through both Ask and the drill-in. */
test('Ask groups searches by meaning and the drill-in links to similar searches', async ({
  page,
}, testInfo) => {
  test.setTimeout(600_000)
  await searchGroupsScenario(page, testInfo)
})

test('Insights shows things looked into by day and month with profile-scoped drill-ins', async ({
  page,
}, testInfo) => {
  test.setTimeout(600_000)
  await insightsSearchGroupsScenario(page, testInfo)
})

/**
 * Rebuild search index writes the new index beside the one search reads:
 * paused half way, search still finds the pages the rebuild has not read yet,
 * and once it is done the new index takes the old one's place.
 */
test('a rebuild leaves the complete index answering searches until it replaces it', async ({
  page,
}) => {
  test.setTimeout(600_000)
  const MODEL = 'e2e-rebuild-embed'
  const service = await startFakeEmbeddings({
    dimensions: 256,
    models: [
      {
        id: MODEL,
        type: 'embeddings',
        state: 'loaded',
        max_context_length: 8192,
        loaded_context_length: 8192,
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
  const findsByMeaning = async (words: string, title: string) => {
    await page.goto('/#/history')
    await unlockIfLocked(page)
    await page.getByRole('searchbox', { name: 'Search history' }).fill(words)
    await page.getByRole('radio', { name: 'Semantic' }).click()
    await expect(page.getByText(/Semantic search with /)).toBeVisible({
      timeout: 60_000,
    })
    await expect(rows.first()).toContainText(title, { timeout: 60_000 })
  }
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
    await dialog.getByRole('textbox', { name: 'Name' }).fill('E2E Rebuild')
    await dialog.getByRole('button', { name: 'Add service' }).click()
    await expect(page.getByRole('combobox', { name: 'Model' })).toContainText(
      'E2E Rebuild',
    )
    await page.getByRole('switch', { name: 'Semantic search' }).click()
    await expect
      .poll(
        async () =>
          (await queueStatus()).semanticIndex.chosenComplete &&
          (await busy()) === 0,
        { timeout: 240_000 },
      )
      .toBe(true)
    const { providerId } = (await queueStatus()).semanticIndex.chosen!
    const complete = servingOf(providerId, MODEL)!
    // Search ranking's pages were visited last, so a rebuild reads them last.
    await findsByMeaning('lighthouse keeper journal', IN_TITLE.title)

    // Rebuilt, paused after its first batch.
    await page.goto('/#/settings/ai')
    await unlockIfLocked(page)
    service.holdAfter(1)
    await page.getByRole('button', { name: 'Rebuild', exact: true }).click()
    await page
      .getByRole('alertdialog', { name: 'Rebuild the search index?' })
      .getByRole('button', { name: 'Rebuild' })
      .click()
    await expect
      .poll(() => service.held(), { timeout: 120_000 })
      .toBeGreaterThan(0)
    await page.getByRole('button', { name: 'Pause' }).click()
    await expect(
      page.getByText(/^Paused at [\d,]+ of [\d,]+ visits\.$/),
    ).toBeVisible({ timeout: 30_000 })
    service.release()

    // Search still reads the complete index, which the rebuild writes beside,
    // never into, and Settings says so.
    await expect(
      page.getByText(
        /^Until this finishes, search uses the index you have now: [\d,]+ pages?\.$/,
      ),
    ).toBeVisible()
    expect(servingOf(providerId, MODEL), 'the same index serves').toEqual(
      complete,
    )
    expect(
      lineagesOf(providerId, MODEL),
      'the rebuild writes a lineage of its own',
    ).toHaveLength(2)
    expect((await queueStatus()).semanticIndex).toMatchObject({
      chosenComplete: true,
      build: { state: 'paused', pausedBy: 'user' },
    })
    await findsByMeaning('lighthouse keeper journal', IN_TITLE.title)

    // Continued to the end, the new index takes the old one's place, and
    // only its files remain.
    await page.goto('/#/settings/ai')
    await unlockIfLocked(page)
    await page.getByRole('button', { name: 'Continue' }).click()
    await expect
      .poll(
        async () =>
          servingOf(providerId, MODEL)?.lineage !== complete.lineage &&
          (await busy()) === 0,
        { timeout: 240_000 },
      )
      .toBe(true)
    await expectOneGeneration(providerId, MODEL)
    await findsByMeaning('lighthouse keeper journal', IN_TITLE.title)
    finished = true
  } finally {
    const restore = async () => {
      await unlockIfLocked(page)
      const current = await loadConfig()
      await backend('save_config', { config: { ...current, ai: before.ai } })
    }
    await restore().catch((error: unknown) => {
      if (finished) throw error
    })
    await service.close()
  }
})

/**
 * A rebuild cancelled half way leaves the index it would have replaced as it
 * was: it still answers by meaning and still counts every visit it holds. A
 * page backed up while the rebuild was paused is added by the next update,
 * and nothing that index already had is sent again. Nothing of the rebuild
 * stays on disk.
 */
test('a cancelled rebuild leaves the index it would have replaced, and the next update adds what was backed up meanwhile', async ({
  page,
}) => {
  test.setTimeout(600_000)
  const MODEL = 'e2e-cancelled-rebuild'
  const NEW = PAUSED_REBUILD_PAGE
  const service = await startFakeEmbeddings({
    dimensions: 256,
    models: [
      {
        id: MODEL,
        type: 'embeddings',
        state: 'loaded',
        max_context_length: 8192,
        loaded_context_length: 8192,
      },
    ],
  })
  const before = await loadConfig()
  const busy = async () => {
    const queue = await queueStatus()
    return queue.queued + queue.running
  }
  // Semantic search merges in full-text results: ask why a page matched.
  const matchReason = async (words: string, url: string) => {
    const { items } = await backend<{
      items: { url: string; matchReason: string }[]
    }>('search_ai_history', { request: { query: words, limit: 20 } })
    return items.find((item) => item.url === url)?.matchReason ?? 'not found'
  }
  const indexedVisits = async () =>
    (await backend<{ aiStatus: { indexedItems: number } }>('app_snapshot'))
      .aiStatus.indexedItems
  const backUp = async () => {
    await page.goto('/#/backup')
    await unlockIfLocked(page)
    await page.getByRole('button', { name: 'Back up now' }).first().click()
  }
  let finished = false
  try {
    await addEmbeddingsService(page, service.url, MODEL, 'E2E Cancelled')
    await page.getByRole('switch', { name: 'Semantic search' }).click()
    await expect
      .poll(
        async () =>
          (await queueStatus()).semanticIndex.chosenComplete &&
          (await busy()) === 0,
        { timeout: 240_000 },
      )
      .toBe(true)
    const { providerId } = (await queueStatus()).semanticIndex.chosen!
    const indexed = await indexedVisits()
    expect(
      await matchReason('lighthouse keeper journal', IN_TITLE.url),
    ).toMatch(/semantic/)

    // Rebuilt, paused after its first batch.
    await page.goto('/#/settings/ai')
    await unlockIfLocked(page)
    service.holdAfter(1)
    await page.getByRole('button', { name: 'Rebuild', exact: true }).click()
    await page
      .getByRole('alertdialog', { name: 'Rebuild the search index?' })
      .getByRole('button', { name: 'Rebuild' })
      .click()
    await expect
      .poll(() => service.held(), { timeout: 120_000 })
      .toBeGreaterThan(0)
    await page.getByRole('button', { name: 'Pause' }).click()
    await expect(
      page.getByText(/^Paused at [\d,]+ of [\d,]+ visits\.$/),
    ).toBeVisible({ timeout: 30_000 })
    service.release()
    expect(
      lineagesOf(providerId, MODEL),
      'the rebuild builds beside it',
    ).toHaveLength(2)

    // A page backed up meanwhile is in the archive, found by its words only:
    // the paused rebuild holds the update back.
    appendChromeVisits(fixture().files['chrome:Default'], [
      pausedRebuildPageVisit(),
    ])
    await backUp()
    await expect
      .poll(() => matchReason('anemone survey tide pool', NEW.url), {
        timeout: 120_000,
      })
      .toBe('lexical')

    // Cancelled, the index it would have replaced answers as before, counts
    // what it counted, and the rebuild's files are gone.
    await page.goto('/#/settings/ai')
    await unlockIfLocked(page)
    await page.getByRole('button', { name: 'Cancel', exact: true }).click()
    await expect
      .poll(async () => (await queueStatus()).semanticIndex.build, {
        timeout: 60_000,
      })
      .toBeNull()
    expect((await queueStatus()).semanticIndex.chosenComplete).toBe(true)
    expect(
      await matchReason('lighthouse keeper journal', IN_TITLE.url),
    ).toMatch(/semantic/)
    expect(await indexedVisits()).toBe(indexed)
    // Its files are gone at once; the folder keeps only the note that names
    // its rows until the next pass deletes them.
    const serving = servingOf(providerId, MODEL)!.lineage
    await expect
      .poll(
        () =>
          indexFilesOf(providerId, MODEL).filter(
            (file) =>
              !file.startsWith(`${serving}${path.sep}`) &&
              !file.startsWith(`trash${path.sep}`) &&
              path.basename(file) !== 'identity.json',
          ),
        { timeout: 60_000 },
      )
      .toEqual([])

    // The next update adds the new page, and sends nothing else that index had.
    const sentBefore = service.inputs.length
    await backUp()
    await expect
      .poll(() => matchReason('anemone survey tide pool', NEW.url), {
        timeout: 120_000,
      })
      .toMatch(/semantic/)
    await expect.poll(busy, { timeout: 120_000 }).toBe(0)
    const sent = service.inputs.slice(sentBefore)
    expect(sent.filter((text) => text.startsWith(NEW.title))).toHaveLength(1)
    expect(sent.some((text) => text.startsWith(IN_TITLE.title))).toBe(false)
    expect(await indexedVisits()).toBe(indexed + 1)
    await expect
      .poll(
        () => lineagesOf(providerId, MODEL).filter((name) => name !== 'trash'),
        { timeout: 60_000 },
      )
      .toEqual([serving])
    finished = true
  } finally {
    const restore = async () => {
      await unlockIfLocked(page)
      const current = await loadConfig()
      await backend('save_config', { config: { ...current, ai: before.ai } })
    }
    await restore().catch((error: unknown) => {
      if (finished) throw error
    })
    await service.close()
  }
})
