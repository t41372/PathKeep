/**
 * A Takeout import the app stopped in the middle carries on when the same
 * export is imported again (`WORK-TAKEOUT-RESUME`).
 *
 * Proves: the app is killed for real (`support/app.ts`) once the import has
 * saved its first part. Importing the same folder again finishes it as one
 * import: the card counts every visit of the export as new, none as already
 * there, and says it carried on the import that stopped. What the first run
 * saved is neither imported twice nor counted as a duplicate.
 */
import { expect, test, type Page } from '@playwright/test'
import { killAndRestartApp, unlockIfLocked } from './support/app'
import { backend } from './support/fixture'
import {
  continuedVisitCount,
  writeContinuedFolder,
} from './support/takeout-file'

const RECORDS = continuedVisitCount()

interface Batch {
  sourcePath: string
  status: string
  visibleItems: number
}

/** The import batches of `folder`, newest first. */
async function batchesOf(folder: string) {
  const page = await backend<{ items: Batch[] }>('load_import_batches', {
    limit: 64,
  })
  return page.items.filter((batch) => batch.sourcePath === folder)
}

async function startImport(page: Page, folder: string) {
  await page.goto('/#/backup')
  const field = page.getByRole('textbox', {
    name: 'Path to a Takeout .zip or folder',
  })
  await field.fill(folder)
  await field.press('Enter')
  await expect(
    page.getByText(`${RECORDS.toLocaleString('en-US')} visits found`),
  ).toBeVisible({ timeout: 60_000 })
  await page.getByRole('button', { name: 'Import', exact: true }).click()
}

test('an import stopped half way carries on, as one import', async ({
  page,
}) => {
  test.setTimeout(900_000)
  await page.goto('/#/')
  await unlockIfLocked(page)
  const folder = writeContinuedFolder()

  await startImport(page, folder)
  // Once the import has saved its first part (10,000 records), and before it
  // is done.
  await expect
    .poll(
      async () => {
        const [batch] = await batchesOf(folder)
        return batch?.status === 'running' ? batch.visibleItems : 0
      },
      { timeout: 120_000, intervals: [250] },
    )
    .toBeGreaterThanOrEqual(10_000)
  await killAndRestartApp()

  // Load the restarted app; the page still shows the one that was killed.
  await page.goto('/#/')
  await page.reload()
  await unlockIfLocked(page)
  await startImport(page, folder)
  await expect(
    page.getByText(`${RECORDS.toLocaleString('en-US')} new visits imported`),
  ).toBeVisible({ timeout: 600_000 })
  await expect(
    page.getByText('0 visits were already in your archive.'),
  ).toBeVisible()
  await expect(
    page.getByText(
      /^This finished the import of this export that stopped on .+; the numbers include what it saved then\.$/,
    ),
  ).toBeVisible()
  // One import, finished, holding every visit.
  const batches = await batchesOf(folder)
  expect(
    batches.map(({ status, visibleItems }) => ({ status, visibleItems })),
  ).toEqual([{ status: 'imported', visibleItems: RECORDS }])
})
