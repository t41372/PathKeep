/**
 * Google Takeout: import Chrome history from a Takeout folder.
 *
 * Proves: the preview counts exactly the visits in `BrowserHistory.json`,
 * the import adds exactly those (search finds them), a file that is not a
 * Takeout history payload stays out even when its lines look like history,
 * and importing the same folder again adds nothing.
 *
 * The folder is written for this test (see `support/takeout-file.ts`); none
 * of its pages are in the archive before the import.
 */
import { expect, test, type Page } from '@playwright/test'
import { parseCount } from './support/fixture'
import {
  TAKEOUT_TITLE,
  takeoutPageCount,
  takeoutVisitCount,
  writeTakeoutFolder,
} from './support/takeout-file'

/** Waits for History's search header to count exactly `pages` and `visits`. */
async function expectSearchHits(
  page: Page,
  expected: { pages: number; visits: number },
) {
  await page.goto('/#/history')
  await page
    .getByRole('searchbox', { name: 'Search history' })
    .fill(TAKEOUT_TITLE)
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
  await page.getByRole('button', { name: 'Import', exact: true }).click()
  await expect(page.getByText(`${visits} new visits imported`)).toBeVisible({
    timeout: 60_000,
  })
  await expectSearchHits(page, { pages: takeoutPageCount(), visits })

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
