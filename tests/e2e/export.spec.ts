/**
 * Export: a whole-history export shows how far it has got, survives leaving
 * Settings, can be stopped without leaving a file behind, and when allowed to
 * finish writes exactly the archived visits.
 *
 * The E2E backend sleeps briefly per exported row (PATHKEEP_TEST_EXPORT_ROW_DELAY_US,
 * debug builds only); without it the fixture exports before the first poll.
 */
import { readdirSync, readFileSync } from 'node:fs'
import path from 'node:path'
import { expect, test, type Page } from '@playwright/test'
import { archivedVisits, expectCount, fixture } from './support/fixture'

const exportsDir = () => path.join(fixture().dirs.projectRoot, 'exports')

function exportFiles() {
  try {
    return readdirSync(exportsDir())
  } catch {
    return []
  }
}

const progressBar = (page: Page) =>
  page.getByRole('progressbar', { name: 'Export progress' })

async function percentDone(page: Page) {
  return Number((await progressBar(page).getAttribute('aria-valuenow')) ?? 0)
}

async function startJsonlExport(page: Page) {
  await page.getByRole('combobox', { name: 'Export format' }).click()
  await page.getByRole('option', { name: 'JSON Lines' }).click()
  await page.getByRole('button', { name: 'Export', exact: true }).click()
}

test('an export can be followed, left, stopped, and finished', async ({
  page,
}, testInfo) => {
  const visits = archivedVisits()
  const total = visits.length.toLocaleString('en-US')
  expect(exportFiles(), 'no export has run yet').toEqual([])

  await page.goto('/#/settings/storage')
  await startJsonlExport(page)

  // The bar moves and the line counts against the archive's size.
  await expect(progressBar(page)).toBeVisible()
  await expect.poll(() => percentDone(page)).toBeGreaterThan(0)
  await expect(page.getByText(`of about ${total} visits`)).toBeVisible()
  const before = await percentDone(page)

  // Leaving Settings does not lose the export or offer a second one.
  await page.goto('/#/history')
  await expect(
    page.getByRole('searchbox', { name: 'Search history' }),
  ).toBeVisible()
  await page.goto('/#/settings/storage')
  await expect(page.getByRole('button', { name: 'Stop' })).toBeVisible()
  await expect(
    page.getByRole('button', { name: 'Export', exact: true }),
  ).toHaveCount(0)
  await expect.poll(() => percentDone(page)).toBeGreaterThan(before)

  // Stopping leaves nothing on disk, not even the half-written temp file.
  await page.getByRole('button', { name: 'Stop' }).click()
  await expect(
    page.getByText('Export stopped. No file was saved.'),
  ).toBeVisible()
  await expect(
    page.getByRole('button', { name: 'Export', exact: true }),
  ).toBeEnabled()
  await expect(progressBar(page)).toHaveCount(0)
  expect(exportFiles(), 'a stopped export leaves no file').toEqual([])

  // Run it to the end: one file holding every archived visit, once each.
  await startJsonlExport(page)
  await expect(page.getByText(`Exported ${total} visits.`)).toBeVisible({
    timeout: 120_000,
  })
  const files = exportFiles()
  expect(files).toHaveLength(1)
  expect(files[0]).toMatch(/\.jsonl$/)

  const rows = readFileSync(path.join(exportsDir(), files[0]), 'utf8')
    .split('\n')
    .filter(Boolean)
    .map((line) => JSON.parse(line) as { url: string; visitTime: number })
  const key = (url: string, at: number) => `${at} ${url}`
  const written = rows.map((row) => key(row.url, row.visitTime)).sort()
  const expected = visits.map((visit) => key(visit.url, visit.at)).sort()
  await expectCount(testInfo, 'rows in the export', rows.length, visits.length)
  expect(written, 'the export holds exactly the archived visits').toEqual(
    expected,
  )
})
