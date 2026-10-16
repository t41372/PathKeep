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
 *
 * The app is killed for real (`support/app.ts`; the E2E launcher starts it
 * again). The service runs on this computer (`support/fake-embeddings.ts`);
 * the in-app model is a 531 MB download. It answers the build's first
 * request and holds the next, so the kill lands half way every time, and is
 * told to hold again for the pause.
 */
import { createHash } from 'node:crypto'
import { readdirSync, statSync } from 'node:fs'
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
import { RETITLED_TITLE, indexRevisitVisits } from './support/index-revisit'

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

/** The progress line's counts: "N of M visits", then pace once measured. */
const COUNTS = /^([\d,]+) of ([\d,]+) visits/

/** The pages behind texts sent for embedding; each text also names one visit. */
const pagesOf = (texts: string[]) =>
  texts
    .filter((text) => /^URL: /m.test(text))
    .map(
      (text) =>
        `${/^URL: (.*)$/m.exec(text)?.[1]} · ${/^Title: (.*)$/m.exec(text)?.[1]}`,
    )
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
    const newTitle = `${retitled.url} · ${RETITLED_TITLE}`
    await expect
      .poll(() => pagesOf(service.inputs.slice(atBackup)), {
        timeout: 120_000,
      })
      .toContain(newTitle)
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
    expect([...new Set(sent)]).toEqual([newTitle])
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
    await expect(edit.getByRole('note')).toContainText(
      'until it is complete, semantic search keeps using e2e-embed.',
    )
    await edit.getByRole('button', { name: 'Save', exact: true }).click()
    await expect(
      page.getByText(/^e2e-embed-next has no index yet\./),
    ).toBeVisible({ timeout: 30_000 })
    await expect(
      page.getByText(
        'Until it is complete, semantic search keeps using e2e-embed.',
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
