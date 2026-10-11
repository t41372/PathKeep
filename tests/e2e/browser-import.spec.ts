/**
 * Browser Direct: import another browser's history file into the archive.
 *
 * Proves: the preview counts new vs already-archived visits exactly (all
 * new the first time; after more browsing in the same file, only the added
 * visits are new), the import adds exactly the new ones (search finds them,
 * no duplicates), undoing an import hides exactly what it added, and
 * restoring brings exactly those back. Each undo / restore shows the batch
 * before acting. A Safari History.db puts each visit on its own day (Safari
 * counts seconds from 2001), names each page by the newest title Safari
 * saved for it (an untitled latest visit keeps the one before), and Insights
 * says Safari keeps no opener.
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
  writeFirefoxPlaces,
  writeSafariHistory,
} from '../../scripts/fixtures/synthetic-browsers.mjs'
import {
  BROWSER_FILE_TITLE,
  browserFilePath,
  firstBrowserFileVisits,
  laterBrowserFileVisits,
} from './support/browser-file'
import { expectCount, parseCount } from './support/fixture'
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
