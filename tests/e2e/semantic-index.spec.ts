/**
 * Semantic search with an embeddings service of one's own, and an index
 * build that survives the app being killed half way.
 *
 * Proves: an OpenAI-compatible embeddings service added in Settings → AI
 * passes its connection test, and turning semantic search on builds the
 * index with it. The build, interrupted by killing the app the way a crash
 * would, carries on when the app starts again and finishes with every page
 * indexed, without starting over: no page embedded before the kill is sent
 * to the service again. Settings shows the build's progress while it runs.
 * Then History's semantic search finds a page by its words, and its date
 * filter applies to the matches themselves: the page is listed for a day
 * exactly when it was visited that day. After a later backup, the index
 * catches up by itself: a page visited again as it was is not sent to the
 * service again, and a page whose title changed is, and is found by its new
 * title. Settings → Storage counts the index's files.
 *
 * The app is killed for real (`support/app.ts`; the E2E launcher starts it
 * again). The service runs on this computer (`support/fake-embeddings.ts`);
 * the in-app model is a 531 MB download. It answers the connection test and
 * the build's first request, and holds the next, so the kill lands half way
 * every time.
 */
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
  test.setTimeout(420_000)
  // The connection test, then the build's first batch.
  const service = await startFakeEmbeddings({ holdAfter: 2 })
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
    await expect(
      page.getByText(/^Connected to e2e-embed in \d+ ms\.$/),
    ).toBeVisible({ timeout: 30_000 })

    // Turning semantic search on builds the index with the service.
    await page.getByRole('switch', { name: 'Semantic search' }).click()

    // Half way: the first batch is saved and the build waits on the next.
    await record('queue after turning it on')
    await expect
      .poll(() => service.held(), { timeout: 120_000 })
      .toBeGreaterThan(0)
    const progress = page.getByText(/^[\d,]+ of [\d,]+ pages$/)
    let done = 0
    let total = 0
    await expect
      .poll(
        async () => {
          const counts = (await progress.innerText().catch(() => '')).match(
            /^([\d,]+) of ([\d,]+) pages$/,
          )
          if (!counts) return 0
          total = parseCount(counts[2])
          done = parseCount(counts[1])
          return done
        },
        { timeout: 30_000 },
      )
      .toBeGreaterThan(0)
    expect(done, 'not done yet').toBeLessThan(total)

    const embeddedBefore = new Set(pagesOf(service.inputs))
    const atKill = service.inputs.length
    await killAndRestartApp()
    service.release()

    await page.goto('/#/settings/ai')
    await page.reload()
    await unlockIfLocked(page)
    await expect(
      page.getByText(`${total.toLocaleString('en-US')} pages indexed`, {
        exact: false,
      }),
    ).toBeVisible({ timeout: 240_000 })
    const embeddedAfter = pagesOf(service.inputs.slice(atKill))
    expect(
      embeddedAfter.length,
      'the build went on after the restart',
    ).toBeGreaterThan(0)
    expect(
      embeddedAfter.filter((again) => embeddedBefore.has(again)),
      'and did not start over',
    ).toEqual([])

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
