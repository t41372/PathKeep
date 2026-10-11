/**
 * Google Takeout: import Chrome history from a Takeout folder.
 *
 * Proves: the preview counts exactly the visits in `BrowserHistory.json`,
 * the import adds exactly those (search finds them), files that are not a
 * Chrome history payload stay out even when they look like history (My
 * Activity's Chrome entries in HTML, JSON Lines), the card says to export My
 * Activity as JSON, and importing the same folder again adds nothing. Takeout
 * records no page's opener, so its site's Insights page says it cannot tell
 * how the visits began rather than that none came from elsewhere. My
 * Activity in JSON, in a Chinese account's folder, imports its visits: each
 * the page behind Google's redirect, titled without "Visited ", the "Used
 * Chrome" record left out, and nothing new the second time. A My Activity
 * export of visits the browsers already recorded keeps them and counts them
 * once: the card says they were already there from another source, the page
 * keeps its visit count, and its details say Takeout recorded them too. A
 * download as Google sends it, one .zip with Chrome's `History.json`, Chrome's
 * other files and My Activity's copy of every visit, imports each visit once
 * and says the copies were already there, whichever the zip lists first.
 *
 * The folders and the zip are written for this test (see
 * `support/takeout-file.ts`); none of their pages are in the archive before
 * the import.
 */
import { expect, test, type Page } from '@playwright/test'
import { archivedVisits, parseCount } from './support/fixture'
import {
  ACTIVITY_TITLE,
  TAKEOUT_TITLE,
  activityPageCount,
  activityVisitCount,
  takeoutPageCount,
  takeoutVisitCount,
  writeMyActivityCopiesFolder,
  writeMyActivityJsonFolder,
  writeMyActivityOnlyFolder,
  writeTakeoutFolder,
  writeTakeoutZip,
  ZIP_TITLE,
  zipPageCount,
  zipVisitCount,
} from './support/takeout-file'

/** Waits for History's search for `query` to count exactly `pages` and `visits`. */
async function expectSearchHits(
  page: Page,
  expected: { pages: number; visits: number },
  query = TAKEOUT_TITLE,
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

const MY_ACTIVITY_HINT =
  'This export has My Activity’s Chrome entries as HTML, which PathKeep cannot read. In Takeout, choose JSON as the format for My Activity, or tick Chrome.'

async function inspect(page: Page, folder: string) {
  await page.goto('/#/backup')
  const field = page.getByRole('textbox', {
    name: 'Path to a Takeout .zip or folder',
  })
  await field.fill(folder)
  await field.press('Enter')
}

test('a Takeout folder imports its Chrome history and nothing else, once', async ({
  page,
}) => {
  const folder = writeTakeoutFolder()
  const visits = takeoutVisitCount()
  await expectSearchHits(page, { pages: 0, visits: 0 })

  await inspect(page, folder)
  await expect(page.getByText(`${visits} visits found`)).toBeVisible({
    timeout: 30_000,
  })
  await expect(page.getByText(MY_ACTIVITY_HINT)).toBeVisible()
  await page.getByRole('button', { name: 'Import', exact: true }).click()
  await expect(page.getByText(`${visits} new visits imported`)).toBeVisible({
    timeout: 60_000,
  })
  await expectSearchHits(page, { pages: takeoutPageCount(), visits })

  // Only Takeout visited the folder's site, and Takeout keeps no opener.
  await page.goto('/#/insights/site/takeout.example')
  await expect(
    page.getByText(
      "Your visits here came from a browser that doesn't record which page opened a page (Safari, or history imported from Google Takeout), so PathKeep can't tell how you got here.",
    ),
  ).toBeVisible({ timeout: 60_000 })

  // The same folder again: every visit is already there.
  await inspect(page, folder)
  await expect(page.getByText(`${visits} visits found`)).toBeVisible({
    timeout: 30_000,
  })
  await page.getByRole('button', { name: 'Import', exact: true }).click()
  await expect(page.getByText('0 new visits imported')).toBeVisible({
    timeout: 60_000,
  })
  await expect(
    page.getByText(`${visits} visits were already in your archive.`),
  ).toBeVisible()
  await expectSearchHits(page, { pages: takeoutPageCount(), visits })
})

test('My Activity in HTML finds nothing and says to export JSON', async ({
  page,
}) => {
  await page.goto('/#/backup')
  // The steps are folded away until asked for.
  const steps = page.getByRole('button', {
    name: 'How to get it from Google Takeout',
  })
  await expect(page.getByText('Tick Chrome.', { exact: false })).toBeHidden()
  await steps.click()
  await expect(
    page.getByText(
      'Tick Chrome. In its data options you can keep just History.',
    ),
  ).toBeVisible()

  await inspect(page, writeMyActivityOnlyFolder())
  await expect(page.getByText('No browsing history found')).toBeVisible({
    timeout: 30_000,
  })
  await expect(page.getByText(MY_ACTIVITY_HINT)).toBeVisible()
  await expect(
    page.getByRole('button', { name: 'Import', exact: true }),
  ).toHaveCount(0)
})

test('My Activity in JSON imports the pages it visited, once', async ({
  page,
}) => {
  const folder = writeMyActivityJsonFolder()
  const visits = activityVisitCount()
  const pages = activityPageCount()
  await expectSearchHits(page, { pages: 0, visits: 0 }, ACTIVITY_TITLE)

  await inspect(page, folder)
  await expect(page.getByText(`${visits} visits found`)).toBeVisible({
    timeout: 30_000,
  })
  await expect(page.getByText(MY_ACTIVITY_HINT)).toHaveCount(0)
  await page.getByRole('button', { name: 'Import', exact: true }).click()
  await expect(page.getByText(`${visits} new visits imported`)).toBeVisible({
    timeout: 60_000,
  })

  // The pages themselves, not Google's redirect, titled without its prefix.
  await expectSearchHits(page, { pages, visits }, ACTIVITY_TITLE)
  const results = page.getByRole('main')
  await expect(results.getByText(`${ACTIVITY_TITLE} 0`).first()).toBeVisible()
  await expect(
    results.getByText('activity.e2e.example', { exact: false }).first(),
  ).toBeVisible()
  await expect(results.getByText(/google\.com/)).toHaveCount(0)
  await expect(results.getByText(`Visited ${ACTIVITY_TITLE}`)).toHaveCount(0)

  // A later import of the same export adds nothing.
  await inspect(page, folder)
  await expect(page.getByText(`${visits} visits found`)).toBeVisible({
    timeout: 30_000,
  })
  await page.getByRole('button', { name: 'Import', exact: true }).click()
  await expect(page.getByText('0 new visits imported')).toBeVisible({
    timeout: 60_000,
  })
  await expectSearchHits(page, { pages, visits }, ACTIVITY_TITLE)
})

const SAME_VISITS = (count: number) =>
  `${count} visits were already in your archive from another source (the browser, or another export). They are kept and counted once.`

test('a Takeout .zip with Chrome history and My Activity counts each visit once', async ({
  page,
}) => {
  const zip = writeTakeoutZip()
  const visits = zipVisitCount()
  const pages = zipPageCount()
  await expectSearchHits(page, { pages: 0, visits: 0 }, ZIP_TITLE)

  // Chrome's history and My Activity's copy of it; bookmarks and search
  // engines are not visits.
  await inspect(page, zip)
  await expect(page.getByText(`${2 * visits} visits found`)).toBeVisible({
    timeout: 30_000,
  })
  await page.getByRole('button', { name: 'Import', exact: true }).click()
  // My Activity is listed first and its folder name sorts first; Chrome's
  // history is still written first, so the copies are the ones kept aside.
  await expect(page.getByText(`${visits} new visits imported`)).toBeVisible({
    timeout: 60_000,
  })
  await expect(page.getByText(SAME_VISITS(visits))).toBeVisible()
  await expectSearchHits(page, { pages, visits }, ZIP_TITLE)

  // The same download again: every record is already there.
  await inspect(page, zip)
  await expect(page.getByText(`${2 * visits} visits found`)).toBeVisible({
    timeout: 30_000,
  })
  await page.getByRole('button', { name: 'Import', exact: true }).click()
  await expect(page.getByText('0 new visits imported')).toBeVisible({
    timeout: 60_000,
  })
  await expect(
    page.getByText(`${2 * visits} visits were already in your archive.`),
  ).toBeVisible()
  await expectSearchHits(page, { pages, visits }, ZIP_TITLE)
})

/** A page every browser visited; its visits are copied into My Activity below. */
const SHARED_URL = 'https://news.ycombinator.com/item?id=41502281'
const SHARED_TITLE = 'Hacker News · Why I left tokio'

test('visits the browsers already recorded are kept and counted once', async ({
  page,
}) => {
  const visits = archivedVisits().filter((visit) => visit.url === SHARED_URL)
  expect(visits.length, 'the browsers visited the page').toBeGreaterThan(1)
  const { root, copies } = writeMyActivityCopiesFolder(SHARED_URL)

  await inspect(page, root)
  await expect(page.getByText(`${copies} visits found`)).toBeVisible({
    timeout: 30_000,
  })
  await page.getByRole('button', { name: 'Import', exact: true }).click()
  await expect(page.getByText('0 new visits imported')).toBeVisible({
    timeout: 60_000,
  })
  await expect(page.getByText(SAME_VISITS(copies))).toBeVisible()

  await page.goto('/#/history')
  await page
    .getByRole('searchbox', { name: 'Search history' })
    .fill('why I left tokio')
  const row = page
    .getByRole('listbox', { name: 'History results' })
    .getByRole('option')
    .filter({ hasText: SHARED_TITLE })
  await expect(row).toHaveCount(1)
  await row.click()
  const detail = page.getByRole('complementary', { name: 'Page details' })
  const total = detail.getByText('Total visits').locator('..')
  await expect(total).toContainText(/\d/)
  expect(
    parseCount((await total.innerText()).replace('Total visits', '')),
    'the page counts each visit once',
  ).toBe(visits.length)
  await expect(
    detail.getByText(
      'Google Takeout recorded these visits too. They count once.',
    ),
  ).toBeVisible()
})
