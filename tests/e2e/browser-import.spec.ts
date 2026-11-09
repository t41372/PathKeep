/**
 * Browser Direct: import another browser's history file into the archive.
 *
 * Proves: the preview counts new vs already-archived visits exactly (all
 * new the first time; after more browsing in the same file, only the added
 * visits are new), the import adds exactly the new ones (search finds them,
 * no duplicates), undoing an import hides exactly what it added, and
 * restoring brings exactly those back. Each undo / restore shows the batch
 * before acting, and the restore lists the undo before it. A Safari
 * History.db puts each visit on its own day (Safari counts seconds from
 * 2001), names each page by the newest title Safari saved for it (an
 * untitled latest visit keeps the one before), and Insights says Safari
 * keeps no opener. An import whose visits all came from it,
 * once undone, leaves Insights with no rebuild asked for: its profile shows
 * no visits, and its day and Top sites over all profiles no longer count them;
 * a page it alone brought that was starred stays in Starred, with its site.
 *
 * The files are written for this test (see `support/browser-file.ts` and
 * `support/safari-file.ts`), not profiles the archive backs up, so nothing in
 * them is in the archive before the first import.
 */
import { mkdirSync } from 'node:fs'
import path from 'node:path'
import { expect, test, type Page } from '@playwright/test'
import {
  appendFirefoxVisits,
  writeChromeHistory,
  writeFirefoxPlaces,
  writeSafariHistory,
} from '../../scripts/fixtures/synthetic-browsers.mjs'
import {
  BROWSER_FILE_TITLE,
  UNDONE_FILE_SITE,
  UNDONE_FILE_TITLE,
  browserFilePath,
  firstBrowserFileVisits,
  laterBrowserFileVisits,
  undoneBrowserFilePath,
  undoneBrowserFileVisits,
} from './support/browser-file'
import {
  archivedVisits,
  expectCount,
  localDay as fixtureDay,
  parseCount,
} from './support/fixture'
import {
  SAFARI_TITLE,
  localDay,
  safariFilePath,
  safariVisits,
} from './support/safari-file'

/**
 * Searches History for the file's pages and waits for exactly `expected`
 * visits. Search lists each page once; the header counts their visits
 * ("4 pages · 14 visits · Full text").
 */
async function expectSearchHits(page: Page, expected: number) {
  await page.goto('/#/history')
  await page
    .getByRole('searchbox', { name: 'Search history' })
    .fill(BROWSER_FILE_TITLE)
  const header = page.getByText(/visits? · Full text$|^Nothing found$/).first()
  await expect
    .poll(
      async () => {
        const text = await header.innerText().catch(() => '')
        if (text === 'Nothing found') return 0
        const counts = text.match(
          /^[\d,]+ pages? · ([\d,]+) visits? · Full text$/,
        )
        return counts ? parseCount(counts[1]) : Number.NaN
      },
      { timeout: 30_000 },
    )
    .toBe(expected)
}

/** Types the file's path into the Browser Direct card and waits for the preview. */
async function inspectFile(page: Page, file = browserFilePath()) {
  await page.goto('/#/backup')
  // The card keeps the last import's result until the user starts another.
  const card = page
    .getByRole('heading', { name: 'Import a browser’s history' })
    .locator('xpath=ancestor::section[1]')
  const another = card.getByRole('button', { name: 'Import another' })
  if (await another.isVisible()) await another.click()
  const field = page.getByRole('textbox', {
    name: 'Path to a history file or profile folder',
  })
  await field.fill(file)
  await field.press('Enter')
  const preview = page.getByTestId('browser-import-preview')
  await expect(preview).toBeVisible({ timeout: 30_000 })
  return preview
}

const visitsLabel = (count: number, word: string) =>
  `${count.toLocaleString('en-US')} ${count === 1 ? word : `${word}s`}`

test('a browser file previews new vs archived visits, imports, undoes and restores exactly', async ({
  page,
}, testInfo) => {
  const first = firstBrowserFileVisits()
  const later = laterBrowserFileVisits()
  mkdirSync(path.dirname(browserFilePath()), { recursive: true })
  writeFirefoxPlaces(browserFilePath(), first)

  // Not in the archive before the import.
  await expectSearchHits(page, 0)

  // First look: every visit in the file is new.
  let preview = await inspectFile(page)
  await expect(preview).toContainText('places.sqlite · Firefox history')
  await expect(preview).toContainText(
    `${visitsLabel(first.length, 'new visit')}`,
  )
  await expect(preview).toContainText('0 are already in your archive')
  await expect(preview).toContainText(
    `${visitsLabel(first.length, 'visit')} in the file`,
  )
  await page
    .getByRole('button', { name: `Import ${first.length} visits` })
    .click()
  await expect(
    page.getByText(`Imported ${first.length} new visits`).first(),
  ).toBeVisible({ timeout: 60_000 })
  await expectSearchHits(page, first.length)

  // More browsing in the same file: only the added visits are new.
  appendFirefoxVisits(browserFilePath(), later)
  preview = await inspectFile(page)
  const previewText = await preview.innerText()
  const newCount = parseCount(previewText.match(/([\d,]+) new visits?/)![1])
  const archivedCount = parseCount(
    previewText.match(/([\d,]+) (?:is|are) already in your archive/)![1],
  )
  await expectCount(
    testInfo,
    'new visits in the preview',
    newCount,
    later.length,
  )
  await expectCount(
    testInfo,
    'already-archived visits in the preview',
    archivedCount,
    first.length,
  )
  await page
    .getByRole('button', { name: `Import ${later.length} visits` })
    .click()
  await expect(
    page.getByText(`Imported ${later.length} new visits`).first(),
  ).toBeVisible({ timeout: 60_000 })
  await expectSearchHits(page, first.length + later.length)

  // Undo the second import: the preview shows what it added, then exactly
  // those visits disappear.
  await page.goto('/#/backup')
  const imports = page
    .getByRole('heading', { name: 'Recent imports' })
    .locator('xpath=ancestor::section[1]')
  const secondBatch = imports
    .getByRole('listitem')
    .filter({ hasText: `${later.length} new · ${first.length} already there` })
  await secondBatch.getByRole('button', { name: 'Undo' }).click()
  const undo = page.getByRole('dialog', { name: /^Import from / })
  const counts = undo.getByTestId('batch-counts')
  await expect(counts).toContainText(`Added${later.length}`)
  await expect(counts).toContainText(`Already there${first.length}`)
  await expect(counts).toContainText(`Shown in history${later.length}`)
  await expect(undo).toContainText(browserFilePath())
  await undo.getByRole('button', { name: 'Undo import' }).click()
  await expect(undo).toBeHidden({ timeout: 60_000 })
  await expectSearchHits(page, first.length)

  // Restore it: back to every visit, still no duplicates.
  await page.goto('/#/backup')
  const undone = imports
    .getByRole('listitem')
    .filter({ hasText: 'Undone. Hidden from your history.' })
  await undone.getByRole('button', { name: 'Restore' }).click()
  const restore = page.getByRole('dialog', { name: /^Import from / })
  await expect(restore.getByTestId('batch-counts')).toContainText(
    'Shown in history0',
  )
  await expect(
    restore.getByText(
      `Undone: this import’s ${visitsLabel(later.length, 'visit')} ${later.length === 1 ? 'is' : 'are'} hidden.`,
    ),
  ).toBeVisible()
  await restore.getByRole('button', { name: 'Restore import' }).click()
  await expect(restore).toBeHidden({ timeout: 60_000 })
  await expectSearchHits(page, first.length + later.length)
})

/** Searches History for `query` and waits for exactly these pages and visits. */
async function expectPagesAndVisits(
  page: Page,
  query: string,
  expected: { pages: number; visits: number },
) {
  await page.goto('/#/history')
  await page.getByRole('searchbox', { name: 'Search history' }).fill(query)
  const header = page.getByText(/visits? · Full text$|^Nothing found$/).first()
  await expect
    .poll(
      async () => {
        const text = await header.innerText().catch(() => '')
        if (text === 'Nothing found') return { pages: 0, visits: 0 }
        const counts = text.match(
          /^([\d,]+) pages? · ([\d,]+) visits? · Full text$/,
        )
        return counts
          ? { pages: parseCount(counts[1]), visits: parseCount(counts[2]) }
          : null
      },
      { timeout: 30_000 },
    )
    .toEqual(expected)
}

test('a Safari History.db puts each visit on its day, named as Safari last saw it', async ({
  page,
}) => {
  const visits = safariVisits()
  const pages = new Set(visits.map((visit) => visit.url)).size
  mkdirSync(path.dirname(safariFilePath()), { recursive: true })
  writeSafariHistory(safariFilePath(), visits)
  await expectPagesAndVisits(page, SAFARI_TITLE, { pages: 0, visits: 0 })

  const preview = await inspectFile(page, safariFilePath())
  await expect(preview).toContainText('History.db · Safari history')
  await expect(preview).toContainText(visitsLabel(visits.length, 'new visit'))
  await expect(preview).toContainText('0 are already in your archive')
  await page
    .getByRole('button', { name: `Import ${visits.length} visits` })
    .click()
  await expect(
    page.getByText(`Imported ${visits.length} new visits`).first(),
  ).toBeVisible({ timeout: 60_000 })

  // Every page by its newest title; the first one's older title is gone.
  await expectPagesAndVisits(page, SAFARI_TITLE, {
    pages,
    visits: visits.length,
  })
  const rows = page
    .getByRole('listbox', { name: 'History results' })
    .getByRole('option')
  for (const index of [0, 1, 2]) {
    await expect(
      rows.filter({ hasText: `${SAFARI_TITLE} ${index}` }),
    ).toHaveCount(1)
  }
  await expect(rows.filter({ hasText: 'Almanac draft' })).toHaveCount(0)

  // Each day holds the visits Safari recorded on it, in this computer's time.
  for (const day of new Set(visits.map((visit) => localDay(visit.at)))) {
    const onDay = visits.filter((visit) => localDay(visit.at) === day)
    await expectPagesAndVisits(
      page,
      `${SAFARI_TITLE} after:${day} before:${day}`,
      {
        pages: new Set(onDay.map((visit) => visit.url)).size,
        visits: onDay.length,
      },
    )
  }

  // Safari keeps no opener, so Insights can't say how the visits began.
  await page.goto('/#/insights/site/almanac.example')
  await expect(
    page.getByText(
      "Your visits here came from a browser that doesn't record which page opened a page (Safari, or history imported from Google Takeout), so PathKeep can't tell how you got here.",
    ),
  ).toBeVisible({ timeout: 60_000 })

  // The same file again: nothing new.
  const again = await inspectFile(page, safariFilePath())
  await expect(again).toContainText('0 new visits')
  await expect(again).toContainText(
    `${visits.length} are already in your archive`,
  )
})

const sectionOf = (page: Page, title: string) =>
  page
    .getByRole('heading', { name: title, exact: true })
    .locator('xpath=ancestor::section[1]')

/**
 * Opens `url` until its "Page visits" tile reads `visits`: Insights take an
 * import or a rebuild in a moment after it finishes. Returns what it read.
 */
async function expectPageVisits(page: Page, url: string, visits: number) {
  let shown = Number.NaN
  await expect
    .poll(
      async () => {
        await page.goto('about:blank')
        await page.goto(url)
        const tile = page.getByRole('group', {
          name: 'Page visits',
          exact: true,
        })
        await expect(tile).toHaveAttribute('aria-busy', 'false', {
          timeout: 30_000,
        })
        shown = parseCount((await tile.innerText()).split('\n')[1] ?? '')
        return shown
      },
      { timeout: 180_000, intervals: [2_000, 5_000] },
    )
    .toBe(visits)
  return shown
}

/** The row of `site` in Top sites, by its link. */
const topSiteRow = (page: Page, site: string) =>
  sectionOf(page, 'Top sites').locator(`a[href*="/insights/site/${site}?"]`)

test('an undone import leaves Insights, and its starred page stays in Starred', async ({
  page,
}, testInfo) => {
  test.setTimeout(300_000)
  const visits = undoneBrowserFileVisits()
  const day = fixtureDay(visits[0].at)
  // The day as the fixture's browsers had it; the other specs add nothing to it.
  const before = archivedVisits().filter(
    (visit) => fixtureDay(visit.at) === day,
  ).length
  const starredTitle = `${UNDONE_FILE_TITLE} 1`
  mkdirSync(path.dirname(undoneBrowserFilePath()), { recursive: true })
  writeChromeHistory(undoneBrowserFilePath(), visits)

  const preview = await inspectFile(page, undoneBrowserFilePath())
  await expect(preview).toContainText(visitsLabel(visits.length, 'new visit'))
  await page
    .getByRole('button', { name: `Import ${visits.length} visits` })
    .click()
  await expect(
    page.getByText(`Imported ${visits.length} new visits`).first(),
  ).toBeVisible({ timeout: 60_000 })

  // Imported: its own profile, the day and Top sites count its visits.
  const dayRange = `/#/insights?range=custom&start=${day}&end=${day}`
  await page.goto(dayRange)
  await page.getByRole('combobox', { name: 'Browser profile' }).click()
  await page
    .getByRole('option', {
      name: 'Chromium · Imported browser profile',
      exact: true,
    })
    .click()
  await expect(page).toHaveURL(/profile=browser-direct/)
  const profileRange = page.url().replace(/^.*?#/, '/#')
  await expectPageVisits(page, profileRange, visits.length)
  await expectPageVisits(page, `/#/insights/day/${day}`, before + visits.length)
  await page.goto(dayRange)
  await expect(topSiteRow(page, UNDONE_FILE_SITE)).toContainText(
    String(visits.length),
    { timeout: 60_000 },
  )

  // A page only the import brought, starred.
  await page.goto('/#/history')
  await page
    .getByRole('searchbox', { name: 'Search history' })
    .fill(`"${starredTitle}"`)
  await page
    .getByRole('listbox', { name: 'History results' })
    .getByRole('option')
    .filter({ hasText: starredTitle })
    .click()
  const detail = page.getByRole('complementary', { name: 'Page details' })
  await detail.getByRole('button', { name: 'Star this page' }).click()
  await expect(
    detail.getByRole('button', { name: 'Remove star' }),
  ).toBeVisible()

  // Undone.
  await page.goto('/#/backup')
  await page
    .getByRole('heading', { name: 'Recent imports' })
    .locator('xpath=ancestor::section[1]')
    .getByRole('listitem')
    .filter({ hasText: `${visits.length} new · 0 already there` })
    .getByRole('button', { name: 'Undo' })
    .click()
  const undo = page.getByRole('dialog', { name: /^Import from / })
  await expect(undo).toContainText(undoneBrowserFilePath())
  await undo.getByRole('button', { name: 'Undo import' }).click()
  await expect(undo).toBeHidden({ timeout: 60_000 })

  // The undo queues the Insights build itself.
  await expectCount(
    testInfo,
    `visits on ${day}, every profile, after the undo`,
    await expectPageVisits(page, `/#/insights/day/${day}`, before),
    before,
  )
  await expectCount(
    testInfo,
    'visits in the undone import’s profile',
    await expectPageVisits(page, profileRange, 0),
    0,
  )
  await page.goto(dayRange)
  await expect(
    sectionOf(page, 'Top sites').getByRole('listitem').first(),
  ).toBeVisible({
    timeout: 60_000,
  })
  await expect(topSiteRow(page, UNDONE_FILE_SITE)).toHaveCount(0)

  // Still starred, under its site.
  await page.goto('/#/history')
  await page.getByRole('tab', { name: /^Starred/ }).click()
  const entry = page
    .getByRole('listbox', { name: 'Starred' })
    .getByRole('option')
    .filter({ hasText: starredTitle })
  await expect(entry).toHaveCount(1)
  await expect(entry).toContainText(UNDONE_FILE_SITE)
})
