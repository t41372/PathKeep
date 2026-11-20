/**
 * The run ledger's rows on the background tasks page (`data-spaces.md`
 * §5.2), with both kinds on, on the archive the specs before left.
 *
 * Proves:
 * - the page keeps itself current from the backend's event, without
 *   reading its lists on a timer: while nothing changes it reads nothing;
 *   an Insights job queued from outside the window (as the menu bar or
 *   another window would) shows up and moves to Done today with no reload;
 * - a browsing backup that ended short is listed under Needs a look with
 *   Dismiss; dismissing it takes it out of Needs a look (the backend's
 *   list and the rail's count follow), it stays in Done today and its
 *   details still open;
 * - work on the whole archive (the password change `security.spec` made
 *   earlier today) is listed under Done today as "Whole archive", with no
 *   space's tile, and opens its details.
 *
 * Puts back the history file it hid. Assumes the E2E run started today:
 * the whole-archive row is from earlier in it.
 */
import { renameSync, existsSync } from 'node:fs'
import { expect, test, type Page } from '@playwright/test'
import {
  expectIdleIndicator,
  lastBrowsingRun,
  railAttention,
  taskRow,
  taskSummary,
  waitForBrowsingRunAfter,
} from './support/agent-backup'
import { unlockIfLocked } from './support/app'
import { backend, fixture } from './support/fixture'

interface WireLedgerTask {
  taskId: string
  jobId: number
  kind: 'browsing' | 'archive'
  operation: string
  state: string
  attention: boolean
  commands: string[]
}

const ledger = (group: string, kind: 'browsing' | 'archive' = 'browsing') =>
  backend<{ items: WireLedgerTask[] }>('list_background_tasks', {
    request: { group, kind, limit: 50 },
  }).then((page) => page.items)

/** Counts the page's reads of task lists from now on. */
function countListReads(page: Page) {
  let reads = 0
  page.on('request', (request) => {
    if (request.url().includes('/commands/list_background_tasks')) reads++
  })
  return () => reads
}

test('the page follows the backend’s event, and reads nothing while nothing changes', async ({
  page,
}) => {
  test.setTimeout(180_000)
  await page.goto('/#/tasks')
  await unlockIfLocked(page)
  await expect(
    page.getByRole('heading', { name: 'Background tasks', level: 1 }),
  ).toBeVisible()
  await expect(
    page.getByText('Nothing is running now.', { exact: true }),
  ).toBeVisible({ timeout: 60_000 })

  // Quiet: no reads on a timer.
  const reads = countListReads(page)
  await page.waitForTimeout(6_000)
  expect(reads(), 'list reads while nothing changed').toBe(0)

  // Work started outside this window appears, runs and ends here: an
  // Insights build, as the app queues one. It builds what earlier specs left
  // stale, so it can take a while. `limit` is the build's batch size; 1
  // would write each visit on its own and take ten minutes on the Linux
  // debug build.
  const { jobId } = await backend<{ jobId: number }>(
    'queue_core_intelligence_rebuild',
    { request: { profileId: null, fullRebuild: false, limit: null } },
  )
  const row = page.locator(
    `section[data-task-group] li[data-task-id="browsing:intelligence/${jobId}"]`,
  )
  await expect(row.first()).toBeVisible({ timeout: 15_000 })
  // The page has to show it done once the backend says so (nothing else
  // was queued or running: the page said so above).
  await expect
    .poll(
      async () => {
        const summary = await taskSummary()
        return (
          summary.queuedCount === 0 &&
          !summary.running.some(
            (task) => task.taskId === `browsing:intelligence/${jobId}`,
          )
        )
      },
      { timeout: 150_000 },
    )
    .toBe(true)
  const done = await taskRow(
    page,
    'today',
    `li[data-task-id="browsing:intelligence/${jobId}"]`,
  )
  await expect(done).toContainText('Done', { timeout: 60_000 })
  expect(reads(), 'the lists were read again on the event').toBeGreaterThan(0)
  await expect(
    page.getByText('Nothing is running now.', { exact: true }),
  ).toBeVisible({ timeout: 60_000 })
})

test('a browsing backup that ended short is dismissed from Needs a look, and its history stays', async ({
  page,
}, testInfo) => {
  test.setTimeout(300_000)
  const work = fixture().files['chrome:Profile 1']
  const away = `${work}.away`
  await page.goto('/#/tasks')
  await unlockIfLocked(page)
  const before = await lastBrowsingRun()

  // Chrome's Work profile lost its history file: the backup ends short.
  renameSync(work, away)
  try {
    await page.keyboard.press('ControlOrMeta+k')
    const palette = page.getByRole('dialog', { name: 'Search and commands' })
    await palette.getByRole('combobox').fill('back up')
    await palette
      .getByRole('option', { name: 'Back up Browsing history only' })
      .click()
    await waitForBrowsingRunAfter(before?.id)
  } finally {
    if (existsSync(away)) renameSync(away, work)
  }
  const run = await lastBrowsingRun()
  expect(run.status).toBe('partial')
  const taskId = `browsing:run/${run.id}`
  const selector = `li[data-task-id="${taskId}"]`

  // Needs a look, with Dismiss; the rail counts it.
  const row = await taskRow(page, 'attention', selector)
  await expect(row).toContainText('Back up browsing history')
  await expect(row).toContainText('Didn’t finish everything')
  const listed = (await ledger('attention')).find(
    (task) => task.taskId === taskId,
  )
  expect(listed?.commands).toContain('dismiss')
  const counted = await railAttention()
  expect(counted).toBeGreaterThan(0)
  // The backup queued Insights work for what it added; the indicator is
  // idle once that has run.
  await expectIdleIndicator(page, 180_000)
  await testInfo.attach('a backup that ended short, under Needs a look', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })

  await row
    .getByRole('button', {
      name: 'Dismiss “Back up browsing history” from Needs a look',
    })
    .click()

  // Out of Needs a look, here and in the backend; one fewer on the rail.
  await expect(
    page.locator(`section[data-task-group="attention"] ${selector}`),
  ).toHaveCount(0)
  expect((await ledger('attention')).map((task) => task.taskId)).not.toContain(
    taskId,
  )
  await expect.poll(railAttention).toBe(counted - 1)
  await expectIdleIndicator(page)

  // Its history stays: Done today, still short, nothing to dismiss, and its
  // details open.
  const today = await taskRow(page, 'today', selector)
  await expect(today).toContainText('Didn’t finish everything')
  await expect(today.getByRole('button', { name: /^Dismiss/ })).toHaveCount(0)
  const kept = (await ledger('today')).find((task) => task.taskId === taskId)
  expect(kept?.attention).toBe(false)
  await today.getByRole('link', { name: 'Back up browsing history' }).click()
  await expect(page).toHaveURL(new RegExp(`#/browsing/backup\\?run=${run.id}$`))
  await expect(
    page.getByRole('dialog').getByText('Run details', { exact: true }),
  ).toBeVisible()
})

/** What the page calls each kind of work on the whole archive. */
const WHOLE_ARCHIVE_NAMES: Record<string, string> = {
  rekey: 'Encryption change',
  doctor: 'Health check',
  retention_prune: 'Cleanup',
  archive_restore: 'Archive restore',
  restore: 'Import restored',
}

test('work on the whole archive is listed as such and opens its details', async ({
  page,
}) => {
  await page.goto('/#/tasks')
  await unlockIfLocked(page)
  const [work] = await ledger('today', 'archive')
  expect(work, 'whole-archive work earlier today').toBeTruthy()
  expect(work.kind).toBe('archive')

  const row = await taskRow(
    page,
    'today',
    `li[data-task-id="archive:run/${work.jobId}"]`,
  )
  await expect(row).toHaveAttribute('data-kind', 'archive')
  await expect(row).toContainText('Whole archive')
  // No space's tile: it belongs to none.
  await expect(
    row.getByText(/^(Browsing history|Agent conversations)$/),
  ).toHaveCount(0)
  const name = WHOLE_ARCHIVE_NAMES[work.operation]
  expect(name, `a name for ${work.operation}`).toBeTruthy()
  const link = row.getByRole('link', { name })
  await expect(link).toHaveAttribute(
    'href',
    `#/browsing/backup?run=${work.jobId}`,
  )
  await link.click()
  await expect(
    page.getByRole('dialog').getByText('Run details', { exact: true }),
  ).toBeVisible()
})
