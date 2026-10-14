/**
 * Export: a whole-history export is saved where the save dialog says, shows
 * how far it has got, survives leaving Settings, can be stopped without
 * leaving a file behind, and when allowed to finish writes exactly the
 * archived visits, with each visit's duration in milliseconds.
 *
 * Playwright cannot answer the system's save dialog, so the test build saves
 * under the suggested name in PATHKEEP_TEST_SAVE_DIR (`saved` in the run's
 * folder), which other specs save into too.
 *
 * The E2E backend sleeps briefly per exported row (PATHKEEP_TEST_EXPORT_ROW_DELAY_US,
 * debug builds only); without it the fixture exports before the first poll.
 */
import { existsSync, readdirSync, readFileSync } from 'node:fs'
import path from 'node:path'
import { expect, test, type Page } from '@playwright/test'
import {
  archivedProfiles,
  archivedVisits,
  expectCount,
  fixture,
} from './support/fixture'

const savedDir = () => path.join(fixture().root, 'saved')

/** History exports in the save folder, and any temp file one left beside them. */
function exportFiles() {
  try {
    return readdirSync(savedDir()).filter(
      (name) =>
        name.startsWith('PathKeep history ') || name.startsWith('.pk-durable-'),
    )
  } catch {
    return []
  }
}

/** The Export history row; Move to another computer has an "Export…" too. */
const exportRow = (page: Page) =>
  page.getByText('Export history', { exact: true }).locator('xpath=../..')

const progressBar = (page: Page) =>
  page.getByRole('progressbar', { name: 'Export progress' })

async function percentDone(page: Page) {
  return Number((await progressBar(page).getAttribute('aria-valuenow')) ?? 0)
}

async function startJsonlExport(page: Page) {
  await page.getByRole('combobox', { name: 'Export format' }).click()
  await page.getByRole('option', { name: 'JSON Lines' }).click()
  await exportRow(page)
    .getByRole('button', { name: 'Export…', exact: true })
    .click()
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
    exportRow(page).getByRole('button', { name: 'Export…', exact: true }),
  ).toHaveCount(0)
  await expect.poll(() => percentDone(page)).toBeGreaterThan(before)

  // Stopping leaves nothing on disk, not even the half-written temp file.
  await page.getByRole('button', { name: 'Stop' }).click()
  await expect(
    page.getByText('Export stopped. No file was saved.'),
  ).toBeVisible()
  await expect(
    exportRow(page).getByRole('button', { name: 'Export…', exact: true }),
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
  // Named for the day, where the dialog said.
  expect(files[0]).toMatch(/^PathKeep history \d{4}-\d{2}-\d{2}\.jsonl$/)
  await expect(page.getByText(path.join(savedDir(), files[0]))).toBeVisible()
  // Not in the app's own exports folder, where history exports used to go.
  const appExports = path.join(fixture().dirs.projectRoot, 'exports')
  const leftInApp = existsSync(appExports) ? readdirSync(appExports) : []
  expect(leftInApp.filter((name) => name.endsWith('.jsonl'))).toEqual([])

  const rows = readFileSync(path.join(savedDir(), files[0]), 'utf8')
    .split('\n')
    .filter(Boolean)
    .map(
      (line) =>
        JSON.parse(line) as {
          url: string
          visitTime: number
          profileId: string
          durationMs?: number | null
        },
    )
  const key = (url: string, at: number) => `${at} ${url}`
  const written = rows.map((row) => key(row.url, row.visitTime)).sort()
  const expected = visits.map((visit) => key(visit.url, visit.at)).sort()
  await expectCount(testInfo, 'rows in the export', rows.length, visits.length)
  expect(written, 'the export holds exactly the archived visits').toEqual(
    expected,
  )

  // Chrome records how long a visit lasted, in microseconds; the export says
  // milliseconds, as its name does. Firefox's history file has no duration.
  const durations = new Map(
    archivedProfiles.flatMap((profile) =>
      fixture().visitsByProfile[profile].map((visit) => [
        `${profile} ${key(visit.url, visit.at)}`,
        profile.startsWith('chrome:') ? visit.duration : null,
      ]),
    ),
  )
  for (const row of rows)
    expect(row.durationMs ?? null, `${row.profileId} ${row.url}`).toBe(
      durations.get(`${row.profileId} ${key(row.url, row.visitTime)}`),
    )
})
