/**
 * A running import can be stopped from its card (`WORK-IMPORT-CANCEL`).
 *
 * Proves: a Takeout import keeps running, with its progress and its Stop,
 * when the user leaves Backup and comes back. Stop ends it after the part it
 * is writing: the card says how many visits it saved, which is what its
 * batch holds, and the batch is interrupted. Importing the same export again
 * carries on in that batch and ends with every visit. A browser's history
 * file stops the same way; imported again it is a new import, where what was
 * saved counts as already in the archive.
 */
import { mkdirSync } from 'node:fs'
import path from 'node:path'
import { expect, test, type Page } from '@playwright/test'
import { writeFirefoxPlaces } from '../../scripts/fixtures/synthetic-browsers.mjs'
import { unlockIfLocked } from './support/app'
import {
  largeBrowserFilePath,
  largeBrowserFileVisits,
} from './support/browser-file'
import { backend, parseCount } from './support/fixture'
import { stoppedVisitCount, writeStoppedFolder } from './support/takeout-file'

interface Batch {
  sourcePath: string
  status: string
  visibleItems: number
}

/** The import batches of `source`: interrupted ones first, then newest first. */
async function batchesOf(source: string) {
  const page = await backend<{ items: Batch[] }>('load_import_batches', {
    limit: 64,
  })
  return page.items.filter((batch) => batch.sourcePath === source)
}

/** Once the newest import of `source` has saved its first part, and is still running. */
async function firstPartSaved(source: string) {
  await expect
    .poll(
      async () => {
        const [batch] = await batchesOf(source)
        return batch?.status === 'running' ? batch.visibleItems : 0
      },
      { timeout: 120_000, intervals: [250] },
    )
    .toBeGreaterThanOrEqual(10_000)
}

const count = (value: number) => value.toLocaleString('en-US')

/** The Backup card under the heading `title`. */
const card = (page: Page, title: string) =>
  page
    .getByRole('heading', { name: title, exact: true })
    .locator('xpath=ancestor::section[1]')

/** Stops the running import from `importCard` and returns the visits it says it saved. */
async function stop(importCard: ReturnType<typeof card>) {
  await importCard.getByRole('button', { name: 'Stop', exact: true }).click()
  const stopped = importCard.getByText(
    /^Stopped\. [\d,]+ new visits? (was|were) saved\.$/,
  )
  await expect(stopped).toBeVisible({ timeout: 120_000 })
  return parseCount((await stopped.innerText()).match(/[\d,]+/)![0])
}

test('a Takeout import stops from its card and carries on when imported again', async ({
  page,
}) => {
  test.setTimeout(600_000)
  const folder = writeStoppedFolder()
  const total = stoppedVisitCount()
  await page.goto('/#/backup')
  await unlockIfLocked(page)
  const takeout = card(page, 'Import')

  const field = takeout.getByRole('textbox', {
    name: 'Path to a Takeout .zip or folder',
  })
  await field.fill(folder)
  await field.press('Enter')
  await expect(takeout.getByText(`${count(total)} visits found`)).toBeVisible({
    timeout: 60_000,
  })
  await takeout.getByRole('button', { name: 'Import', exact: true }).click()
  await firstPartSaved(folder)

  // Away from Backup and back: the import is still on its card.
  await page.goto('/#/')
  await page.goto('/#/backup')
  await expect(takeout.getByText('Importing…')).toBeVisible()

  const saved = await stop(takeout)
  const [batch] = await batchesOf(folder)
  expect(batch).toMatchObject({ status: 'interrupted', visibleItems: saved })
  expect(saved).toBeLessThan(total)
  await expect(
    takeout.getByText(
      'Import the same export again to finish it. It reads the export from the start, but passes quickly over what was saved.',
    ),
  ).toBeVisible()

  // The same export again carries on: one import, every visit.
  await takeout.getByRole('button', { name: 'Import another' }).click()
  await field.fill(folder)
  await field.press('Enter')
  await takeout.getByRole('button', { name: 'Import', exact: true }).click()
  await expect(
    takeout.getByText(`${count(total)} new visits imported`),
  ).toBeVisible({ timeout: 300_000 })
  await expect(
    takeout.getByText('0 visits were already in your archive.'),
  ).toBeVisible()
  const batches = await batchesOf(folder)
  expect(
    batches.map(({ status, visibleItems }) => ({ status, visibleItems })),
  ).toEqual([{ status: 'imported', visibleItems: total }])
})

test('a browser history file stops from its card, and imported again adds the rest', async ({
  page,
}) => {
  test.setTimeout(600_000)
  const file = largeBrowserFilePath()
  const visits = largeBrowserFileVisits()
  mkdirSync(path.dirname(file), { recursive: true })
  writeFirefoxPlaces(file, visits)
  await page.goto('/#/backup')
  await unlockIfLocked(page)
  const browser = card(page, 'Import a browser’s history')

  const inspect = async () => {
    const field = browser.getByRole('textbox', {
      name: 'Path to a history file or profile folder',
    })
    await field.fill(file)
    await field.press('Enter')
    await expect(browser.getByTestId('browser-import-preview')).toBeVisible({
      timeout: 60_000,
    })
  }
  await inspect()
  await browser
    .getByRole('button', { name: `Import ${count(visits.length)} visits` })
    .click()
  await firstPartSaved(file)

  const saved = await stop(browser)
  const [batch] = await batchesOf(file)
  expect(batch).toMatchObject({ status: 'interrupted', visibleItems: saved })
  expect(saved).toBeLessThan(visits.length)
  await expect(
    browser.getByText(
      'Importing this file again adds the rest; what was saved counts as already in your archive.',
    ),
  ).toBeVisible()

  // Again: a new import of the rest; what was saved is already there.
  await browser.getByRole('button', { name: 'Import another' }).click()
  await inspect()
  const rest = visits.length - saved
  await browser
    .getByRole('button', { name: `Import ${count(rest)} visits` })
    .click()
  await expect(
    browser.getByText(`Imported ${count(rest)} new visits`),
  ).toBeVisible({ timeout: 300_000 })
  await expect(
    browser.getByText(`${count(saved)} visits were already in your archive.`),
  ).toBeVisible()
  // Interrupted imports are listed first.
  const batches = await batchesOf(file)
  expect(
    batches.map(({ status, visibleItems }) => ({ status, visibleItems })),
  ).toEqual([
    { status: 'interrupted', visibleItems: saved },
    { status: 'imported', visibleItems: rest },
  ])
})
