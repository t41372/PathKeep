/**
 * Quitting while the archive is being updated for this version asks first
 * (`WORK-SCHEMA-UPDATE-QUIT`), on a real v1 archive (tests/fixtures/native-v1).
 *
 * Proves, against the real app: the update that runs as an old archive opens
 * counts as work someone is waiting on. Closing the window during it asks,
 * names the update and says what quitting does; "Quit now" ends the app in
 * the middle of it, and the next launch starts it again. There ⌘Q asks too:
 * "Keep PathKeep open" leaves the same app updating, and "Quit when done"
 * keeps it running until the update has finished, then quits. The launch
 * after that opens the archive; backup-integrity.spec then checks it in full.
 *
 * The fixture's update takes well under a second, so the debug-only switch
 * `PATHKEEP_TEST_HOLD_SCHEMA_UPDATE` holds it just before its first change
 * while the file it names exists, and says which app is holding it
 * (`vault-core/src/test_support.rs`); deleting the file lets it go on. As in
 * quit-during-work.spec, `debug_request_quit` raises the window's close and
 * ⌘Q through the app's own handlers.
 */
import { existsSync, readFileSync, rmSync } from 'node:fs'
import { expect, test, type Page } from '@playwright/test'
import {
  restartAppWhenItExits,
  runningPid,
  waitForAppRestart,
} from './support/app'
import { backend } from './support/fixture'

function holdFile() {
  const file = process.env.PATHKEEP_TEST_HOLD_SCHEMA_UPDATE
  if (!file)
    throw new Error(
      'PATHKEEP_TEST_HOLD_SCHEMA_UPDATE is not set; run via playwright.backup-integrity.config.ts.',
    )
  return file
}

/**
 * The process id of the last app that started updating the archive, if one
 * has: each records itself just before its first change, held or not.
 */
function heldBy() {
  const held = `${holdFile()}.held`
  return existsSync(held) ? Number(readFileSync(held, 'utf8')) : null
}

/** Waits until app `pid` holds the update, before it has changed anything. */
async function expectHeldBy(pid: number) {
  await expect.poll(heldBy, { timeout: 120_000 }).toBe(pid)
}

const question = (page: Page, name: string) =>
  page.getByRole('alertdialog', { name })

const UPDATE = 'Updating the archive for this version'
const IF_QUIT =
  'It stops partway without damaging the archive, and carries on the next time PathKeep opens.'

test('quitting during the archive update asks first, and waiting lets it finish', async ({
  page,
}) => {
  test.setTimeout(600_000)
  // The app's own window opened the archive when it started.
  const first = await runningPid()
  expect(first, 'the app answers').not.toBeNull()
  await page.goto('/')
  await expectHeldBy(first!)
  restartAppWhenItExits()

  // Closing the window: it stays, and asks.
  await backend('debug_request_quit', { via: 'window' })
  const asking = question(page, 'PathKeep is still working')
  await expect(asking).toBeVisible()
  const running = asking.getByRole('list', { name: 'Running now' })
  await expect(running).toContainText(UPDATE)
  await expect(running).toContainText(IF_QUIT)
  expect(await runningPid(), 'asking did not quit').toBe(first)

  // Quit now ends the app in the middle of the update.
  await asking.getByRole('button', { name: 'Quit now' }).click()
  await waitForAppRestart(first!)

  // The next launch starts the update again; ⌘Q asks too.
  const second = await runningPid()
  await page.reload()
  await expectHeldBy(second!)
  restartAppWhenItExits()
  await backend('debug_request_quit', { via: 'app' })
  await expect(asking).toBeVisible()
  await expect(running).toContainText(UPDATE)

  // Keep it open: the same app goes on, and nothing is asked any more.
  await asking.getByRole('button', { name: 'Keep PathKeep open' }).click()
  await expect(asking).toBeHidden()
  expect(await runningPid(), 'keeping it open did not quit').toBe(second)
  expect(await backend('get_quit_request')).toBeNull()

  // Quit when done: it waits while the update runs.
  await backend('debug_request_quit', { via: 'app' })
  await asking.getByRole('button', { name: 'Quit when done' }).click()
  const waiting = question(page, 'PathKeep will quit when this is done')
  await expect(waiting).toBeVisible()
  await expect(
    waiting.getByRole('list', { name: 'Running now' }),
  ).toContainText(UPDATE)
  // Nothing to wait for here; the app checks what runs every 200 ms.
  await new Promise((resolve) => setTimeout(resolve, 1_000))
  expect(await runningPid(), 'still running while the update is held').toBe(
    second,
  )

  // Let the update go on: once it is done, the app quits by itself.
  rmSync(holdFile())
  await waitForAppRestart(second!)

  // The archive opens; the new app asks nothing.
  await page.reload()
  await expect(page.getByRole('main')).toBeVisible({ timeout: 120_000 })
  expect(await backend('get_quit_request')).toBeNull()
  // The app that waited finished the update itself. Had it quit partway, this
  // one would have had to carry on, and would be the last to have started one.
  expect(heldBy(), 'the update was finished by the app that waited').toBe(
    second,
  )
})
