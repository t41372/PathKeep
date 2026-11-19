/**
 * A first save that goes on behind the dialog, is cancelled from the rail
 * and finished by "Try again" (`data-spaces.md` §3.4, §5.1). It turns Agent
 * conversations off in Manage data types and on again through the off
 * page's preview, which starts a new first save of the same two tools
 * (`agent-backup-1` saved them; capture is slowed per file, so it runs long
 * enough to be watched).
 *
 * Proves:
 * - "Keep going in the background" closes the dialog and the save goes on:
 *   the rail's indicator counts one task, and its list names the first
 *   save with its progress in files and a Cancel;
 * - opening `?enable=agent` again shows the same run (one running task,
 *   not a second);
 * - Cancel in the rail's list ends it as stopped before the end, because
 *   it was cancelled; the conversations saved before are all still there,
 *   and the backup page lists the stopped run;
 * - the dialog, opened again, shows how it ended and "Try again", which
 *   runs once more and finishes; the ledger then has the stopped run once
 *   and the retry after it.
 */
import { expect, test, type Page } from '@playwright/test'
import { enableDialog } from './support/agent-tools'
import {
  agentKindState,
  agentOverview,
  agentRun,
  agentRuns,
  agentTasks,
  chooseAndStart,
  count,
  heldBy,
  openBackupPage,
  openTaskList,
  taskIndicator,
  waitForRunAfter,
} from './support/agent-backup'
import { unlockIfLocked } from './support/app'
import { fixture } from './support/fixture'

const CHOSEN = ['claude-code', 'codex'] as const
const FIRST_SAVE = 'Save Agent conversations for the first time'

/** Opens the dialog over the page shown, without loading the page again. */
async function reopenDialog(page: Page) {
  await page.evaluate(() => {
    const [path] = window.location.hash.split('?')
    window.location.hash = `${path}?enable=agent`
  })
  await expect(enableDialog(page)).toBeVisible()
}

test('a first save goes on behind the dialog, stops from the rail, and finishes on retry', async ({
  page,
}, testInfo) => {
  const held = heldBy(fixture().agents, CHOSEN)
  const before = await agentRuns()

  // Off in Manage data types.
  await page.goto('/#/settings/kinds')
  await unlockIfLocked(page)
  await page
    .getByRole('radiogroup', { name: 'State of Agent conversations' })
    .getByRole('radio', { name: 'Off' })
    .click()
  await expect.poll(async () => (await agentKindState())?.state).toBe('off')

  // On again through the off page's preview: a new first save.
  await page.goto('/#/agent/backup')
  await page
    .getByRole('button', { name: 'Preview and turn on Agent conversations' })
    .click()
  await expect(enableDialog(page)).toBeVisible()
  await chooseAndStart(page, CHOSEN, false)
  await enableDialog(page)
    .getByRole('button', { name: 'Keep going in the background' })
    .click()
  await expect(enableDialog(page)).toBeHidden()

  // The rail shows it, with progress in files and a Cancel.
  await expect(taskIndicator(page)).toHaveAccessibleName(
    /^\d+ background tasks? running$/,
  )
  const list = await openTaskList(page)
  const row = list.locator('li[data-task="agent-history"]')
  await expect(row).toContainText(FIRST_SAVE)
  await expect(row).toContainText(/\d+ files?/)
  await testInfo.attach('the first save in the rail', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
  await page.mouse.move(800, 400)

  // Opening the dialog again shows the same run.
  const running = await agentTasks('running')
  expect(running).toHaveLength(1)
  await reopenDialog(page)
  await expect(
    enableDialog(page).getByRole('heading', { name: 'Saving' }),
  ).toBeVisible()
  await expect(
    enableDialog(page)
      .getByRole('list', { name: 'Saving, by tool' })
      .getByRole('listitem'),
  ).toHaveCount(CHOSEN.length)
  expect((await agentTasks('running')).map((task) => task.taskId)).toEqual(
    running.map((task) => task.taskId),
  )
  await page.keyboard.press('Escape')
  await expect(enableDialog(page)).toBeHidden()

  // Cancel from the rail.
  const again = await openTaskList(page)
  await again.getByRole('button', { name: `Cancel “${FIRST_SAVE}”` }).click()
  const stopped = await waitForRunAfter(before[0]?.runRef)
  expect(stopped.runRef).toBe(running[0].runRef)
  expect(stopped.result).toBe('interrupted')
  expect(stopped.trigger).toBe('enable')
  // What was saved before is all still there.
  expect(count((await agentOverview()).roots)).toBe(held.conversations)

  // The backup page lists the stopped run.
  await page.mouse.move(800, 400)
  await page.getByRole('button', { name: /^Recent backups/ }).click()
  await expect(
    page.locator(`li[data-run-ref="${stopped.runRef}"]`),
  ).toContainText('Stopped before the end')
  await testInfo.attach('the stopped run on the backup page', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })

  // The dialog shows how it ended; "Try again" finishes it.
  await reopenDialog(page)
  const dialog = enableDialog(page)
  await expect(
    dialog.getByRole('heading', { name: 'Saving stopped before the end' }),
  ).toBeVisible()
  await expect(dialog).toContainText('You cancelled it.')
  await testInfo.attach('the dialog after the cancel', {
    body: await dialog.screenshot(),
    contentType: 'image/png',
  })
  await dialog.getByRole('button', { name: 'Try again' }).click()
  await expect(dialog.getByRole('heading', { name: 'Saved' })).toBeVisible({
    timeout: 120_000,
  })
  const retried = await waitForRunAfter(stopped.runRef)
  expect(retried.trigger).toBe('retry')
  // The fixture's empty file and cut-off line are notes, never a reason for
  // a partial run; a red run lists what it could not save.
  expect((await agentRun(retried.runRef)).warnings).toEqual([])
  expect(retried.result).toBe('success')
  await testInfo.attach('the retry finished', {
    body: await dialog.screenshot(),
    contentType: 'image/png',
  })
  // The footer's Close; the × in the corner has the same name.
  await dialog.getByRole('button', { name: 'Close' }).first().click()

  // Nothing ran twice: the stopped run once, then the retry.
  const after = await agentRuns()
  expect(after.length).toBe(before.length + 2)
  expect(after.slice(0, 2).map((run) => [run.trigger, run.result])).toEqual([
    ['retry', retried.result],
    ['enable', 'interrupted'],
  ])
  expect(count((await agentOverview()).roots)).toBe(held.conversations)
  await openBackupPage(page)
  await testInfo.attach('the backup page after the retry', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })
})
