/**
 * The background-task indicator at the top of the rail's shared part
 * (`WORK-SPACES-SHELL`, `data-spaces.md` §5.1), with a real backup.
 *
 * Proves: when nothing runs the indicator says so (with how many things
 * need a look, as the backend counts them: an earlier spec's backup that
 * ended short is one), and its list, opened by hovering, says nothing is
 * running and stays open while the pointer is in it. "Back up everything now" there runs a backup; while it runs the
 * indicator's name reports one task, and its list shows the backup with its
 * progress. When the backup ends the indicator is idle again, and "Open
 * background tasks" leads to the background tasks page (`#/tasks`), which
 * lists that backup (done today, or needing a look if it ended short) with
 * a link to its details.
 * From the keyboard, ↓ on the indicator moves into its list and Escape comes
 * back to it.
 *
 * The backup's answer is held at the transport (`page.route` on the dev IPC
 * bridge's command): the backup runs to the end and reports its progress,
 * while the app still waits for it, so "running" lasts as long as the test
 * needs. Background work is paused meanwhile, so the Insights work the
 * backup queues waits instead of running beside it (the indicator lists
 * that work too). Runs last in its project and waits for that work once
 * background work runs again, so the specs after it start from an idle
 * queue.
 */
import { expect, test, type Page } from '@playwright/test'
import type {
  AppSnapshot,
  IntelligenceRuntimeSnapshot,
} from '../../src/lib/types'
import {
  browsingRunGroup,
  expectIdleIndicator,
  idleIndicatorName,
  railAttention,
  taskIndicator,
  taskRow,
} from './support/agent-backup'
import { unlockIfLocked } from './support/app'
import { backend } from './support/fixture'

const rail = (page: Page) =>
  page.getByRole('navigation', { name: 'Main navigation' })

const taskList = (page: Page) =>
  page.getByRole('dialog').filter({ hasText: 'Background tasks' })

test('the keyboard reaches the indicator’s list and comes back', async ({
  page,
}) => {
  await page.goto('/#/browsing/insights')
  await unlockIfLocked(page)
  const indicator = taskIndicator(page)
  await expect(indicator).toHaveAccessibleName(
    idleIndicatorName(await railAttention()),
  )
  await indicator.focus()
  const list = taskList(page)
  await expect(list).toBeVisible()

  // The list is drawn outside the rail, so ↓ is the way in.
  await page.keyboard.press('ArrowDown')
  await expect(
    list.getByRole('button', { name: 'Back up everything now' }),
  ).toBeFocused()
  await page.keyboard.press('Tab')
  await expect(
    list.getByRole('button', { name: 'Open background tasks' }),
  ).toBeFocused()
  // Focus inside the list keeps it open past the close delay.
  await page.waitForTimeout(600)
  await expect(list).toBeVisible()

  // Escape closes it, hands the focus back, and it does not spring open again.
  await page.keyboard.press('Escape')
  await expect(list).toBeHidden()
  await expect(indicator).toBeFocused()
  await page.waitForTimeout(600)
  await expect(list).toBeHidden()
})

test('the indicator shows a running backup with its progress, and says when nothing runs', async ({
  page,
}) => {
  await page.goto('/#/browsing/insights')
  await unlockIfLocked(page)
  const idle = rail(page).getByRole('button', {
    name: idleIndicatorName(await railAttention()),
  })
  await expect(idle).toBeVisible()

  // Idle: the list says so, and stays while the pointer is in it.
  await idle.hover()
  const list = taskList(page)
  await expect(list).toBeVisible()
  await expect(list.getByText(/^Nothing is running\./)).toBeVisible()
  await list.getByText('Background tasks', { exact: true }).hover()
  await page.waitForTimeout(600)
  await expect(list).toBeVisible()

  let release = () => {}
  const held = new Promise<void>((resolve) => (release = resolve))
  let finished = () => {}
  const backupDone = new Promise<void>((resolve) => (finished = resolve))
  await page.route('**/commands/run_backup_now', async (route) => {
    const response = await route.fetch({ timeout: 300_000 })
    finished()
    await held
    await route.fulfill({ response })
  })

  // Background work waits while the backup is held (see the header).
  const { config } = await backend<AppSnapshot>('app_snapshot')
  await backend('save_config', {
    config: { ...config, ai: { ...config.ai, jobQueuePaused: true } },
  })
  try {
    await list.getByRole('button', { name: 'Back up everything now' }).click()
    const running = rail(page).getByRole('button', {
      name: '1 background task running',
    })
    await expect(running).toBeVisible()
    await backupDone

    await running.hover()
    await expect(list).toBeVisible()
    const rows = list.getByRole('listitem')
    await expect(rows).toHaveCount(1)
    await expect(rows.first()).toContainText('Backup')
    await expect(
      rows.first().getByRole('progressbar', { name: 'Backup' }),
    ).toBeVisible()
    // The backup reported how far it got, and which source it was on.
    await expect(rows.first()).toContainText(/\d{1,3}%/)
    await expect(rows.first()).toContainText(/Backing up \d+ of \d+ sources/)
    await expect(list.getByText(/^Nothing is running\./)).toHaveCount(0)
  } finally {
    release()
  }
  // The backup's answer arrived; background work runs again (the queues
  // reload, which starts their workers on what the backup queued).
  const after = (await backend<AppSnapshot>('app_snapshot')).config
  await backend('save_config', {
    config: {
      ...after,
      ai: { ...after.ai, jobQueuePaused: config.ai.jobQueuePaused },
    },
  })
  await backend('load_intelligence_runtime')

  // Idle again, once the Insights work the backup queued is done; with what
  // needs a look as the backend counts it now.
  await expectIdleIndicator(page)
  await taskIndicator(page).hover()
  await expect(list.getByText(/^Nothing is running\./)).toBeVisible()
  await list.getByRole('button', { name: 'Open background tasks' }).click()
  await expect(page).toHaveURL(/#\/tasks$/)
  await expect(
    page.getByRole('heading', { name: 'Background tasks', level: 1 }),
  ).toBeVisible()
  // The backup just made: done today, or needing a look if it ended short.
  const { recentRuns } = await backend<AppSnapshot>('app_snapshot')
  const run = recentRuns[0]
  const row = await taskRow(
    page,
    browsingRunGroup(run.status),
    `li[data-task-id="browsing:run/${run.id}"]`,
  )
  await expect(row).toContainText('Back up browsing history')
  await expect(row.getByRole('link')).toHaveAttribute(
    'href',
    `#/browsing/backup?run=${run.id}`,
  )

  // Leave the Insights work this backup queued done for the specs after it.
  await expect
    .poll(
      async () => {
        const { queue } = await backend<IntelligenceRuntimeSnapshot>(
          'load_intelligence_runtime',
        )
        return queue.queued + queue.running
      },
      { timeout: 180_000, intervals: [1_000] },
    )
    .toBe(0)
})
