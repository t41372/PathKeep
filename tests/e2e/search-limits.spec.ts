/**
 * History search at its limits, made small: the E2E backend runs with a
 * keyword window of 40 URLs (`PATHKEEP_DEBUG_SEARCH_WINDOW` in
 * playwright.config.ts) instead of the release build's 25,000.
 *
 * Regex requests scan 2,000 visits each (`PATHKEEP_DEBUG_REGEX_CHUNK_ROWS`)
 * instead of 200 ms' worth, so the fixture is scanned in about eleven chunks.
 *
 * Proves: a word that matches more URLs than one search ranks shows an "N+"
 * count, never an exact one, with one plain line saying not every match was
 * ranked and that another word narrows it; and a regex finds visits older
 * than its first chunk by continuing the scan, with an exact count only once
 * the whole archive was scanned.
 */
import { expect, test } from '@playwright/test'
import {
  archivedProfiles,
  archivedVisits,
  fixture,
  parseCount,
} from './support/fixture'

const WINDOW = Number(process.env.PATHKEEP_DEBUG_SEARCH_WINDOW)
const CHUNK = Number(process.env.PATHKEEP_DEBUG_REGEX_CHUNK_ROWS)

test('a word on more pages than one search ranks says so', async ({ page }) => {
  // Every URL has "https" in it; each browser profile keeps its own copy.
  const copies = new Set(
    archivedProfiles.flatMap((id) =>
      fixture().visitsByProfile[id].map((visit) => `${id} ${visit.url}`),
    ),
  )
  expect(WINDOW, 'the E2E config sets a small window').toBeGreaterThan(0)
  expect(
    copies.size,
    'the word must match more than the window',
  ).toBeGreaterThan(WINDOW)

  await page.goto('/#/history')
  await page.getByRole('searchbox', { name: 'Search history' }).fill('https')

  const header = page.getByText(/^[\d,]+\+ pages · Full text$/)
  await expect(header).toBeVisible()
  const shown = parseCount((await header.innerText()).split('+')[0])
  expect(shown).toBeGreaterThan(0)
  expect(shown).toBeLessThanOrEqual(WINDOW)
  await expect(
    page.getByText(
      'Not every match was ranked, only the most recently saved ones. Add a word to narrow the search.',
    ),
  ).toBeVisible()
  // The count stays a lower bound once the separate count query lands too.
  await expect(
    page.getByText(/^[\d,]+ pages? · [\d,]+ visits? · Full text$/),
  ).toHaveCount(0)
})

test('a regex keeps scanning until it reaches the oldest visits', async ({
  page,
}) => {
  const url = 'https://news.ycombinator.com/item?id=41502281'
  const newestFirst = archivedVisits().sort((a, b) => b.at - a.at)
  const firstChunkEnds = newestFirst[CHUNK - 1].at
  const visits = newestFirst.filter((visit) => visit.url === url)
  expect(CHUNK, 'the E2E config sets a small chunk').toBeGreaterThan(0)
  expect(
    visits.filter((visit) => visit.at < firstChunkEnds).length,
    'some visits of the page are older than the first chunk',
  ).toBeGreaterThan(0)

  await page.goto('/#/history')
  const search = page.getByRole('searchbox', { name: 'Search history' })
  // The mode switch shows once there is text to search.
  await search.fill('tokio')
  await page.getByRole('radio', { name: 'Regex' }).click()
  await search.fill('item\\?id=41502281$')

  // Every visit, the old ones included, and an exact count once the scan
  // has covered the archive.
  await expect(
    page.getByText(
      new RegExp(
        `^1 page · ${visits.length.toLocaleString('en-US')} visits · Regex$`,
      ),
    ),
  ).toBeVisible()
  await expect(page.getByText(/so far · searched back to/)).toHaveCount(0)
  const rows = page
    .getByRole('listbox', { name: 'History results' })
    .getByRole('option')
    .filter({ hasText: 'Hacker News · Why I left tokio' })
  await expect(rows).toHaveCount(1)
  await expect(rows).toContainText(
    `${visits.length.toLocaleString('en-US')} visits`,
  )
})
