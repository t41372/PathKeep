/**
 * Quitting while an export runs asks first, and can wait for it
 * (`WORK-QUIT-DURING-WORK`).
 *
 * Proves, against the real app: closing the window during an export does not
 * quit; the question names the export and says what quitting now would do;
 * "Keep PathKeep open" leaves the same app running and the export finishes.
 * Quitting the way ⌘Q and the menu bar icon's Quit do, during a second
 * export, asks again; after "Quit when done" the question says PathKeep is
 * waiting, the export writes its whole file, and only then does the app exit.
 * The launcher starts it again for the specs after this one.
 *
 * A browser cannot press the window's close button or ⌘Q, so the debug-only
 * bridge command `debug_request_quit` raises the same close and quit
 * requests, which go through the app's own handlers. The menu bar icon is off
 * in this run, so closing the window means quitting.
 */
import { readdirSync, readFileSync, statSync } from 'node:fs'
import path from 'node:path'
import { expect, test, type Page } from '@playwright/test'
import {
  restartAppWhenItExits,
  runningPid,
  unlockIfLocked,
  waitForAppRestart,
} from './support/app'
import { backend, expectCount, fixture, parseCount } from './support/fixture'

const savedDir = () => path.join(fixture().root, 'saved')

/** History exports in the save folder, and any temp file one left beside them. */
function savedFiles() {
  return readdirSync(savedDir()).filter(
    (name) =>
      name.startsWith('PathKeep history ') || name.startsWith('.pk-durable-'),
  )
}

/** The Export history row; Move to another computer has an "Export…" too. */
const exportRow = (page: Page) =>
  page.getByText('Export history', { exact: true }).locator('xpath=../..')

const progressBar = (page: Page) =>
  page.getByRole('progressbar', { name: 'Export progress' })

async function startJsonlExport(page: Page) {
  await page.getByRole('combobox', { name: 'Export format' }).click()
  await page.getByRole('option', { name: 'JSON Lines' }).click()
  await exportRow(page)
    .getByRole('button', { name: 'Export…', exact: true })
    .click()
  // Running: the bar is up and has moved.
  await expect(progressBar(page)).toBeVisible()
}

const question = (page: Page, name: string) =>
  page.getByRole('alertdialog', { name })

test('quitting during an export asks first, and waiting lets it finish', async ({
  page,
}, testInfo) => {
  test.setTimeout(600_000)
  await page.goto('/#/settings/storage')
  await unlockIfLocked(page)
  const pid = await runningPid()
  expect(pid, 'the app answers').not.toBeNull()
  // From here on, an exit (expected or not) brings the app back, so a failure
  // stays this spec's and does not end the run.
  restartAppWhenItExits()

  // Closing the window while an export runs: the window stays and asks.
  await startJsonlExport(page)
  await backend('debug_request_quit', { via: 'window' })
  const asking = question(page, 'PathKeep is still working')
  await expect(asking).toBeVisible()
  const running = asking.getByRole('list', { name: 'Running now' })
  await expect(running).toContainText('Exporting history')
  await expect(running).toContainText(
    'No file is saved. You can export again later.',
  )

  // Keep it open: the same app goes on and the export finishes.
  await asking.getByRole('button', { name: 'Keep PathKeep open' }).click()
  await expect(asking).toBeHidden()
  const finished = page.getByText(/^Exported [\d,]+ visits\.$/)
  await expect(finished).toBeVisible({ timeout: 240_000 })
  const exported = parseCount((await finished.textContent()) ?? '')
  expect(exported).toBeGreaterThan(0)
  expect(await runningPid(), 'keeping it open did not quit').toBe(pid)
  expect(await backend('get_quit_request')).toBeNull()

  // Quitting (⌘Q, the menu bar icon's Quit) during a second export asks
  // again; this time, quit when it is done.
  const secondStarted = Date.now()
  await startJsonlExport(page)
  await backend('debug_request_quit', { via: 'app' })
  await expect(asking).toBeVisible()
  await expect(running).toContainText('Exporting history')
  await asking.getByRole('button', { name: 'Quit when done' }).click()
  const waiting = question(page, 'PathKeep will quit when this is done')
  await expect(waiting).toBeVisible()
  await expect(waiting.getByRole('button', { name: 'Quit now' })).toBeVisible()
  expect(await runningPid(), 'still running while the export runs').toBe(pid)

  // The app exits on its own once the export is done.
  await waitForAppRestart(pid!)

  // It waited: the file was written in full by the second export, every
  // line a visit, and no half-written temp file is left beside it.
  const files = savedFiles()
  expect(files.filter((name) => name.startsWith('.pk-durable-'))).toEqual([])
  const written = files
    .filter((name) => name.endsWith('.jsonl'))
    .map((name) => path.join(savedDir(), name))
    .filter((file) => statSync(file).mtimeMs > secondStarted)
  expect(written, 'the second export saved its file').toHaveLength(1)
  const file = written[0]
  const rows = readFileSync(file, 'utf8')
    .split('\n')
    .filter(Boolean)
    .map((line) => JSON.parse(line) as { url?: string; visitTime?: number })
  for (const row of rows) {
    expect(typeof row.url).toBe('string')
    expect(typeof row.visitTime).toBe('number')
  }
  await expectCount(testInfo, 'rows in the export', rows.length, exported)

  // The new app asks nothing.
  await page.reload()
  await unlockIfLocked(page)
  expect(await backend('get_quit_request')).toBeNull()
})
