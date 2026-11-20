/**
 * The app is killed in the middle of a first save of Agent conversations
 * (`agent-history.md` §6; the launcher starts it again behind App Lock). It
 * turns the kind off through the backend (setting up, not what is tested)
 * and on again through the dialog, so a first save of the two tools
 * `agent-backup-1` chose runs, slowed per file.
 *
 * Proves:
 * - after the restart, that first save is over: it ended stopped before
 *   the end because PathKeep closed, and nothing started it again;
 * - Settings › Background work lists it with that reason and "Try again",
 *   which runs it once more to the end;
 * - nothing ran twice: the ledger has the stopped first save once and the
 *   retry after it, and the archive holds the same conversations;
 * - format notices dismissed before the restart (agent-backup-8 dismissed
 *   Pi's formats 1000 and 1001, each with its own Dismiss) stay dismissed:
 *   no item needing a look and no run carries one, and Background work
 *   shows none.
 */
import { expect, test } from '@playwright/test'
import {
  agentOverview,
  agentRuns,
  agentTasks,
  chooseAndStart,
  count,
  newestRunRef,
  waitForRunAfter,
} from './support/agent-backup'
import { enableDialog } from './support/agent-tools'
import { killAndRestartApp, unlockIfLocked } from './support/app'
import { backend } from './support/fixture'

const CHOSEN = ['claude-code', 'codex'] as const

test('a first save cut off by a restart ends stopped, and a retry finishes it once', async ({
  page,
}, testInfo) => {
  // Read while still on: the ledger is not listed while the kind is off.
  const held = count((await agentOverview()).roots)
  const previous = await newestRunRef()
  const runsBefore = (await agentRuns()).length
  const list = await backend<{ policyToken: string }>('list_data_kinds', {
    request: {},
  })
  await backend('set_data_kind_state', {
    request: {
      kind: 'agent-history',
      state: 'off',
      expectedPolicyToken: list.policyToken,
    },
  })

  await page.goto('/#/agent/backup?enable=agent')
  await unlockIfLocked(page)
  await expect(enableDialog(page)).toBeVisible()
  await chooseAndStart(page, CHOSEN, false)
  await expect(
    enableDialog(page)
      .getByText(/\d+ files? (found so far|saved, )/)
      .first(),
  ).toBeVisible()
  const [running] = await agentRuns(1)
  expect(running.result).toBe('running')
  await testInfo.attach('the first save before the restart', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  await killAndRestartApp()
  // Load the restarted app: until the page is loaded again it still shows
  // the killed one, unlocked, while the new one starts behind App Lock.
  await page.goto('/#/settings/background?show=conversations')
  await page.reload()
  await unlockIfLocked(page)

  // Over, stopped by the restart; nothing started it again.
  const stopped = await waitForRunAfter(previous)
  expect(stopped.runRef).toBe(running.runRef)
  expect(stopped.result).toBe('interrupted')
  expect(stopped.trigger).toBe('enable')
  expect(await agentRuns()).toHaveLength(runsBefore + 1)

  // Listed with its reason and "Try again".
  const row = page.locator(`li[data-run-ref="${stopped.runRef}"]`).first()
  await expect(row).toContainText('Stopped before the end')
  await expect(row).toContainText('PathKeep closed while it ran.')
  expect(
    (await agentTasks('attention')).filter(
      (task) => (task.formatNotices ?? []).length > 0,
    ),
  ).toEqual([])
  expect(
    (await agentRuns()).filter((run) => (run.formatNotices ?? []).length > 0),
  ).toEqual([])
  await expect(page.locator('[data-format-notice]')).toHaveCount(0)
  await testInfo.attach('Background work after the restart', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })
  await row
    .getByRole('button', {
      name: 'Try “Save Agent conversations for the first time” again',
    })
    .click()
  const retried = await waitForRunAfter(stopped.runRef)
  expect(retried.trigger).toBe('retry')
  expect(retried.result).not.toBe('interrupted')

  // Once each, and the same conversations.
  const after = await agentRuns()
  expect(after).toHaveLength(runsBefore + 2)
  expect(after.slice(0, 2).map((run) => [run.trigger, run.result])).toEqual([
    ['retry', retried.result],
    ['enable', 'interrupted'],
  ])
  expect(count((await agentOverview()).roots)).toBe(held)
  await testInfo.attach('after the retry', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })
})
