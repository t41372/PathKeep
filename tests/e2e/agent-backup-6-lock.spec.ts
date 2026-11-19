/**
 * Locking the app while a conversation backup runs (`data-spaces.md` §5;
 * App Lock is on since lock.spec). The lock screen hides everything; the
 * backup is the backend's and goes on.
 *
 * Proves:
 * - locked, nothing of the backup page or its numbers is on screen, and
 *   the backend refuses to describe the archive;
 * - the backup was not stopped by the lock: it ends as backups of these
 *   tools end when nothing interrupts them;
 * - after unlocking, the page shows that backup as the last one, with
 *   nothing running.
 */
import { expect, test } from '@playwright/test'
import {
  agentRun,
  agentRuns,
  conversationsText,
  count,
  newestRunRef,
  openBackupPage,
  waitForRunAfter,
} from './support/agent-backup'
import { unlockIfLocked } from './support/app'
import { backend } from './support/fixture'

test('a backup goes on behind the lock screen', async ({ page }, testInfo) => {
  const [previous] = await agentRuns(1)
  await openBackupPage(page)
  const before = await newestRunRef()
  await page.getByRole('button', { name: 'Back up now' }).click()
  await expect(page.getByText(/^Backing up · /)).toBeVisible()

  await page.keyboard.press('ControlOrMeta+l')
  const lock = page.getByRole('heading', { name: 'PathKeep is locked' })
  await expect(lock).toBeVisible()
  await expect(page.getByRole('heading', { name: 'Backup' })).toHaveCount(0)
  await expect(page.getByText(/^Agent conversations · /)).toHaveCount(0)
  await expect(page.getByRole('region', { name: 'Sources' })).toHaveCount(0)
  await expect(page.getByText(/^Backing up · /)).toHaveCount(0)
  let refusal = ''
  try {
    await backend('get_agent_overview', { request: { purpose: 'backup' } })
  } catch (error) {
    refusal = String(error)
  }
  expect(refusal).toContain('lock-required')
  await testInfo.attach('locked while a backup runs', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  await unlockIfLocked(page)
  const run = await waitForRunAfter(before)
  expect(run.result).toBe(previous.result === 'partial' ? 'partial' : 'success')
  const detail = await agentRun(run.runRef)
  expect(count(detail.summary.filesVisited)).toBeGreaterThan(0)

  await openBackupPage(page)
  await expect(page.getByText(/^Backing up · /)).toHaveCount(0)
  await expect(
    page.getByText(/^Last backup( didn’t read everything)?: /),
  ).toBeVisible()
  const overview = await backend<{ roots: string }>('get_agent_overview', {
    request: { purpose: 'backup' },
  })
  await expect(
    page.getByText(
      `Agent conversations · ${conversationsText(count(overview.roots))}.`,
    ),
  ).toBeVisible()
  await testInfo.attach('after unlocking', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })
})
