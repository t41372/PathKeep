/**
 * Pausing and turning off Agent conversations in Settings › Manage data
 * types (`data-spaces.md` §2, §8), with a backup running and without.
 *
 * Proves:
 * - pausing while a backup runs stops it, and the app says which backup
 *   stopped (`affectedTasks`); the run ends stopped before the end;
 * - paused, the backup page says so; "Back up everything now" backs up
 *   browsing history and skips conversations (no new conversation run),
 *   and says it left them out because they are paused, while the page's
 *   own "Back up now" still backs them up;
 * - off, the switcher no longer lists the space, `#/agent/backup` says it
 *   is off, and the archive still holds every conversation;
 * - "On" again turns it on directly (its folders are still approved here):
 *   no preview, no second first save, the same conversations.
 */
import { expect, test, type Page } from '@playwright/test'
import type { AppSnapshot } from '../../src/lib/types'
import {
  agentKindState,
  agentRuns,
  conversationsText,
  count,
  newestRunRef,
  openBackupPage,
  openTaskList,
  waitForRunAfter,
} from './support/agent-backup'
import { enableDialog } from './support/agent-tools'
import { unlockIfLocked } from './support/app'
import { backend } from './support/fixture'

const states = (page: Page) =>
  page.getByRole('radiogroup', { name: 'State of Agent conversations' })

const switcherHasAgent = async (page: Page) => {
  await page.getByRole('button', { name: /^Switch data type\. Now: / }).click()
  const menu = page.getByRole('menu', { name: /^Switch data type/ })
  await expect(menu).toBeVisible()
  const has = await menu
    .getByRole('menuitemradio', { name: /Agent conversations/ })
    .count()
  await page.keyboard.press('Escape')
  return has > 0
}

async function lastBrowsingRun() {
  const snapshot = await backend<AppSnapshot>('app_snapshot')
  return snapshot.recentRuns[0]
}

test('pause stops a running backup and skips the kind; off hides it; on brings it back', async ({
  page,
}, testInfo) => {
  // Pausing while a backup runs stops it, and says so.
  await openBackupPage(page)
  const previous = await newestRunRef()
  await page.getByRole('button', { name: 'Back up now' }).click()
  await expect(page.getByText(/^Backing up · /)).toBeVisible()
  await page.goto('/#/settings/kinds')
  await unlockIfLocked(page)
  await states(page).getByRole('radio', { name: 'Paused' }).click()
  await expect(
    page.getByText(
      'The backup of Agent conversations that was running stopped. What it saved stays in the archive.',
    ),
  ).toBeVisible()
  const stopped = await waitForRunAfter(previous)
  expect(stopped.result).toBe('interrupted')
  expect((await agentKindState())?.state).toBe('paused')
  await testInfo.attach('paused while a backup ran', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  // Paused: the page says so, and "everything" skips the kind.
  await openBackupPage(page)
  await expect(page.getByText('Paused', { exact: true })).toBeVisible()
  const runsBefore = (await agentRuns()).length
  const browsingBefore = await lastBrowsingRun()
  const list = await openTaskList(page)
  await list.getByRole('button', { name: 'Back up everything now' }).click()
  await expect(
    page.getByText('“Agent conversations” is paused, so it wasn’t backed up.'),
  ).toBeVisible()
  await expect
    .poll(
      async () => {
        const now = await lastBrowsingRun()
        return (
          now !== undefined &&
          now.id !== browsingBefore?.id &&
          now.status !== 'running'
        )
      },
      { timeout: 120_000, intervals: [1_000] },
    )
    .toBe(true)
  expect(await agentRuns()).toHaveLength(runsBefore)

  // Its own "Back up now" still works.
  await page.mouse.move(800, 400)
  const beforeOwn = await newestRunRef()
  await page.getByRole('button', { name: 'Back up now' }).click()
  const own = await waitForRunAfter(beforeOwn)
  expect(own.trigger).toBe('manual')
  expect(own.result).not.toBe('interrupted')
  await testInfo.attach('paused, after its own backup', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })

  // Off: hidden, its page says so, and the archive keeps everything.
  const held = await backend<{ roots: string }>('get_agent_overview', {
    request: { purpose: 'backup' },
  })
  const runsHeld = (await agentRuns()).length
  await page.goto('/#/settings/kinds')
  await unlockIfLocked(page)
  await states(page).getByRole('radio', { name: 'Off' }).click()
  await expect(
    page.getByText(
      'Agent conversations is off. Everything it saved is still there.',
    ),
  ).toBeVisible()
  expect(await switcherHasAgent(page)).toBe(false)
  await page.goto('/#/agent/backup')
  await unlockIfLocked(page)
  await expect(
    page.getByRole('heading', { name: '“Agent conversations” is off' }),
  ).toBeVisible()
  const off = await backend<{
    kinds: { kind: string; archiveCount?: string; hasSavedData?: boolean }[]
  }>('list_data_kinds', { request: {} })
  expect(
    off.kinds.find((row) => row.kind === 'agent-history')?.hasSavedData,
  ).toBe(true)
  await testInfo.attach('off', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  // On again: directly, with no preview and no second first save.
  await page.goto('/#/settings/kinds')
  await unlockIfLocked(page)
  await states(page).getByRole('radio', { name: 'On' }).click()
  await expect.poll(async () => (await agentKindState())?.state).toBe('on')
  await expect(page).not.toHaveURL(/enable=agent/)
  await expect(enableDialog(page)).toHaveCount(0)
  expect(await switcherHasAgent(page)).toBe(true)
  expect(await agentRuns()).toHaveLength(runsHeld)
  await openBackupPage(page)
  await expect(
    page.getByText(
      `Agent conversations · ${conversationsText(count(held.roots))}.`,
    ),
  ).toBeVisible()
  await testInfo.attach('on again', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })
})
