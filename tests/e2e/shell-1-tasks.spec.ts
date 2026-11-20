/**
 * The background tasks page (`#/tasks`, `data-spaces.md` §5.2) with both
 * kinds of data on, on the archive the Agent projects left (Claude Code,
 * Codex and Pi backed up; App Lock on).
 *
 * Proves:
 * - "Back up now" in ⌘K starts a backup of both kinds; the page lists the
 *   conversation backup under Running with the Agent space's tile, its
 *   progress in real units and Cancel, the header counts what runs, and
 *   the rail's indicator agrees;
 * - Cancel stops the conversation backup: it leaves Running and is listed
 *   under Done today as stopped before the end, because you cancelled it,
 *   with a link to its details; the browsing backup the same press started
 *   is listed with its own space's tile and a link to its details; the
 *   indicator is idle again;
 * - Try again on it backs up the same scope once more, to the end, and that
 *   backup is listed as ended today;
 * - Settings › Schedule and background work keeps the schedule and the
 *   limits only, and leads to this page, where the queues and their data
 *   now are.
 *
 * Conversation backups are paced (`PATHKEEP_TEST_AGENT_CAPTURE_FILE_DELAY_US`),
 * so one runs long enough to be seen and cancelled. Pausing one is not
 * tested: only its last step (making it searchable) pauses, and nothing
 * paces that step (see the report on this work).
 */
import { expect, test, type Page } from '@playwright/test'
import {
  agentRuns,
  browsingRunGroup,
  idleIndicatorName,
  lastBrowsingRun,
  railAttention,
  taskIndicator,
  taskRow,
  waitForBrowsingRunAfter,
  waitForRunAfter,
} from './support/agent-backup'
import { unlockIfLocked } from './support/app'

const group = (page: Page, name: string) =>
  page.locator(`section[data-task-group="${name}"]`)

test('a backup of both kinds from ⌘K runs on the tasks page; cancelled, tried again, and the rail follows', async ({
  page,
}, testInfo) => {
  test.setTimeout(300_000)
  await page.goto('/#/tasks')
  await unlockIfLocked(page)
  await expect(
    page.getByRole('heading', { name: 'Background tasks', level: 1 }),
  ).toBeVisible()
  // Nothing runs yet: the header and the Running group say so.
  await expect(
    page.getByText('Nothing is running now.', { exact: true }),
  ).toBeVisible({ timeout: 60_000 })
  await expect(group(page, 'running')).toContainText(/^Running\s*None\./)

  const [lastAgent] = await agentRuns(1)
  expect(lastAgent.result).not.toBe('running')
  const browsingBefore = await lastBrowsingRun()

  // ⌘K: Back up now, for every kind that is on.
  await page.keyboard.press('ControlOrMeta+k')
  const palette = page.getByRole('dialog', { name: 'Search and commands' })
  await palette.getByRole('combobox').fill('back up now')
  await palette
    .getByRole('group', { name: 'Actions' })
    .getByRole('option', { name: 'Back up now', exact: true })
    .click()
  await expect(palette).toBeHidden()

  // The conversation backup runs: its row, its progress, its Cancel.
  const agentRow = group(page, 'running').locator(
    'li[data-kind="agent-history"]',
  )
  await expect(agentRow).toBeVisible({ timeout: 30_000 })
  await expect(agentRow).toContainText('Back up Agent conversations')
  await expect(
    agentRow.getByText('Agent conversations', { exact: true }),
  ).toHaveClass(/sr-only/)
  await expect(agentRow.getByRole('progressbar')).toBeVisible()
  await expect(agentRow).toContainText(
    /Waiting to start|Looking for files|files? found so far|saved, /,
  )
  await expect(page.getByText(/^\d+ running\./).first()).toBeVisible()
  await expect(taskIndicator(page)).toHaveAccessibleName(
    /^\d+ background tasks? running$/,
  )
  const taskId = await agentRow.getAttribute('data-task-id')
  expect(taskId).toBeTruthy()
  await testInfo.attach('a conversation backup on the tasks page', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })

  // Cancel, while it still runs (paced to about 13 s): it stops, keeps what
  // it saved, and moves to Done today.
  await agentRow
    .getByRole('button', { name: 'Cancel “Back up Agent conversations”' })
    .click()
  const stopped = await waitForRunAfter(lastAgent.runRef)
  expect(stopped.result).toBe('interrupted')
  await expect(
    group(page, 'running').locator('li[data-kind="agent-history"]'),
  ).toHaveCount(0)
  const stoppedRow = await taskRow(
    page,
    'today',
    `li[data-task-id="${taskId}"]`,
  )
  await expect(stoppedRow).toContainText('Stopped before the end')
  await expect(stoppedRow).toContainText('You cancelled it.')
  await expect(stoppedRow.getByRole('link')).toHaveAttribute(
    'href',
    `#/agent/backup?run=${encodeURIComponent(stopped.runRef)}`,
  )

  // The browsing backup the same press started: listed with its space's
  // tile, done today (or needing a look if it ended short), with a link to
  // its details.
  await waitForBrowsingRunAfter(browsingBefore?.id)
  const browsing = await lastBrowsingRun()
  const browsingRow = await taskRow(
    page,
    browsingRunGroup(browsing.status),
    `li[data-task-id="browsing:run/${browsing.id}"]`,
  )
  await expect(browsingRow).toContainText('Back up browsing history')
  await expect(
    browsingRow.getByText('Browsing history', { exact: true }),
  ).toHaveClass(/sr-only/)
  await expect(browsingRow.getByRole('link')).toHaveAttribute(
    'href',
    `#/browsing/backup?run=${browsing.id}`,
  )
  await expect(taskIndicator(page)).toHaveAccessibleName(
    idleIndicatorName(await railAttention()),
    { timeout: 30_000 },
  )
  await testInfo.attach('the cancelled backup under Done today', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })

  // Try again: the same scope, to the end. It is listed as ended today
  // (under Needs a look instead when it could not read everything, as the
  // backups before it).
  await stoppedRow
    .getByRole('button', { name: 'Try “Back up Agent conversations” again' })
    .click()
  await expect(
    group(page, 'running').locator('li[data-kind="agent-history"]'),
  ).toBeVisible({ timeout: 30_000 })
  const retried = await waitForRunAfter(stopped.runRef, 240_000)
  expect(retried.trigger).toBe('retry')
  expect(['success', 'partial']).toContain(retried.result)
  const ended = await taskRow(
    page,
    retried.result === 'success' ? 'today' : 'attention',
    `li[data-run-ref="${retried.runRef}"]`,
  )
  if (retried.result === 'success') await expect(ended).toContainText('Done')
  await expect(
    group(page, 'running').locator('li[data-kind="agent-history"]'),
  ).toHaveCount(0)
  await expect(taskIndicator(page)).toHaveAccessibleName(
    idleIndicatorName(await railAttention()),
    { timeout: 30_000 },
  )
})

test('Settings keeps the schedule and the limits; the queues are on the tasks page', async ({
  page,
}) => {
  await page.goto('/#/settings/background')
  await unlockIfLocked(page)
  await expect(
    page.getByRole('heading', { name: 'Schedule and background work' }),
  ).toBeVisible()
  await expect(
    page.getByText('One schedule backs up every kind of data that is on.'),
  ).toBeVisible()
  await expect(
    page.getByRole('heading', { name: 'Automatic backup' }),
  ).toBeVisible()
  await expect(
    page.getByRole('switch', { name: 'Run background work' }),
  ).toBeVisible()
  await expect(
    page.getByRole('combobox', { name: 'Tasks at a time' }),
  ).toBeVisible()
  // State is not a setting: no queue and no list of work here.
  await expect(
    page.getByText('Insights and page details', { exact: true }),
  ).toHaveCount(0)
  await expect(page.locator('section[data-task-group]')).toHaveCount(0)

  await page.getByRole('link', { name: 'Open background tasks' }).click()
  await expect(page).toHaveURL(/#\/tasks$/)
  const queues = page.getByRole('region', { name: /^Queues and data/ })
  await expect(
    queues.getByText('Insights and page details', { exact: true }),
  ).toBeVisible()
  await expect(
    queues.getByText('Search index and assistant', { exact: true }),
  ).toBeVisible()
  await expect(
    queues.getByRole('list', { name: 'Insights data' }),
  ).toBeVisible()

  // The header's way back to the settings.
  await page.getByRole('link', { name: 'Schedule and background work' }).click()
  await expect(page).toHaveURL(/#\/settings\/background$/)
})
