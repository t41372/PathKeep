/**
 * Delete all data: the preview says what goes, typing the word deletes it,
 * and PathKeep starts over at onboarding.
 *
 * Proves: the preview's visit count is the archive's real size (the first
 * backup plus the nine added in backup.spec), an installed automatic backup
 * is named in the preview and removed with the data, the archive and the
 * stored key are gone afterwards, and the browsers' own history files are
 * untouched.
 *
 * The automatic backup is installed through Backup like a user would. The
 * E2E backend keeps the OS scheduler in a sandbox folder (debug builds only),
 * so nothing reaches the real launchd or Task Scheduler. Linux has manual
 * setup only, so there the schedule part is skipped.
 */
import { existsSync, readdirSync } from 'node:fs'
import path from 'node:path'
import { expect, test, type Page } from '@playwright/test'
import {
  archivedVisits,
  backend,
  browserFileHashes,
  expectCount,
  fixture,
  parseCount,
} from './support/fixture'

const schedulerSupported = process.platform !== 'linux'

/** App lock is on from lock.spec; unlock if this page load locked it. */
async function unlockIfLocked(page: Page) {
  const lock = page.getByRole('heading', { name: 'PathKeep is locked' })
  if (await lock.isVisible().catch(() => false)) {
    await page.getByPlaceholder('Passcode').fill('4826')
    await page.getByPlaceholder('Passcode').press('Enter')
    await expect(lock).toBeHidden()
  }
}

const installState = async () =>
  (
    await backend<{ installState: string }>('schedule_status', {
      platform: null,
    })
  ).installState

/** Every file in the sandbox that stands in for the OS scheduler. */
function sandboxedSchedulerFiles(): string[] {
  const root = fixture().dirs.osSandbox
  return ['LaunchAgents', 'launchd-loaded', 'TaskScheduler'].flatMap((dir) =>
    existsSync(path.join(root, dir))
      ? readdirSync(path.join(root, dir)).map((name) => path.join(dir, name))
      : [],
  )
}

test('delete all data previews, deletes, and returns to onboarding', async ({
  page,
}, testInfo) => {
  const before = browserFileHashes()
  const expectedVisits = archivedVisits().length + 9

  if (schedulerSupported) {
    await page.goto('/#/backup')
    await unlockIfLocked(page)
    await page.getByRole('radio', { name: 'Daily' }).click()
    // Turning it on shows the file first; nothing is installed until Install.
    await page
      .getByRole('dialog', { name: 'Turn on automatic backup' })
      .getByRole('button', { name: 'Install' })
      .click()
    await expect.poll(installState).toBe('installed')
    expect(sandboxedSchedulerFiles()).not.toEqual([])
  }

  await page.goto('/#/settings/storage')
  await unlockIfLocked(page)
  await page.getByRole('button', { name: 'Delete…' }).click()

  const dialog = page.getByRole('dialog', { name: 'Delete all data?' })
  const summary = dialog.getByText(/of files will be deleted:$/)
  await expect(summary).toBeVisible()
  const visits =
    (await summary.innerText()).match(/^([\d,]+) visits?/)?.[1] ?? ''
  await expectCount(
    testInfo,
    'visits the preview says it will delete',
    parseCount(visits),
    expectedVisits,
  )
  await expect(dialog).toContainText(
    'The archive password saved in the keychain is removed too.',
  )
  if (schedulerSupported) {
    await expect(dialog).toContainText(
      'Automatic backup is turned off and removed from the system:',
    )
    // The item is the sandboxed LaunchAgent file, or the Task Scheduler task.
    await expect(dialog).toContainText(
      process.platform === 'darwin'
        ? path.join(fixture().dirs.osSandbox, 'LaunchAgents')
        : 'Task Scheduler:',
    )
  }

  const confirm = dialog.getByRole('button', { name: 'Delete forever' })
  await expect(confirm).toBeDisabled()
  await dialog.getByRole('textbox').fill('delete')
  await expect(confirm).toBeDisabled()
  await dialog.getByRole('textbox').fill('DELETE')
  await confirm.click()

  await expect(
    page.getByRole('heading', {
      name: 'Your browser is deleting your history',
    }),
  ).toBeVisible({
    timeout: 60_000,
  })
  const snapshot = await backend<{ config: { initialized: boolean } }>(
    'app_snapshot',
  )
  expect(snapshot.config.initialized).toBe(false)
  expect(await backend<string | null>('keyring_get_database_key')).toBeNull()
  if (schedulerSupported) {
    expect(await installState()).toBe('not-installed')
    expect(sandboxedSchedulerFiles(), 'no task left in the OS').toEqual([])
  }
  expect(browserFileHashes(), 'browser files must not change').toEqual(before)
})
