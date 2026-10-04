/**
 * Automatic backup with a custom interval: preview, install, verify, remove.
 *
 * Proves: choosing an interval shows the exact file the scheduler will get
 * and changes nothing until Install (not the saved interval, not the
 * scheduler); Install writes that same file byte for byte and saves the
 * interval; the details sheet reports what the backend verified and the
 * last change; turning it off lists the file it removes, removes it, and
 * keeps the interval for next time.
 *
 * The E2E backend keeps the OS scheduler in a sandbox folder (debug builds
 * only), so nothing reaches the real launchd or Task Scheduler. Linux has
 * manual setup only, so the test is skipped there.
 */
import { existsSync, readFileSync, readdirSync } from 'node:fs'
import path from 'node:path'
import { expect, test, type Page } from '@playwright/test'
import { backend, fixture } from './support/fixture'

test.skip(
  process.platform === 'linux',
  'PathKeep cannot install a scheduler on Linux',
)

const CUSTOM_MINUTES = 90

const installState = async () =>
  (
    await backend<{ installState: string }>('schedule_status', {
      platform: null,
    })
  ).installState

const savedInterval = async () =>
  (await backend<{ config: { dueAfterHours: number } }>('app_snapshot')).config
    .dueAfterHours

/**
 * Specs in a project run in file-name order, so lock.spec may already have
 * turned App Lock on; unlock if this page load locked it.
 */
async function unlockIfLocked(page: Page) {
  const lock = page.getByRole('heading', { name: 'PathKeep is locked' })
  if (await lock.isVisible().catch(() => false)) {
    await page.getByPlaceholder('Passcode').fill('4826')
    await page.getByPlaceholder('Passcode').press('Enter')
    await expect(lock).toBeHidden()
  }
}

/** Files in the sandbox that stand in for the OS scheduler. */
function sandboxedSchedulerFiles(): string[] {
  const root = fixture().dirs.osSandbox
  return ['LaunchAgents', 'launchd-loaded', 'TaskScheduler'].flatMap((dir) =>
    existsSync(path.join(root, dir))
      ? readdirSync(path.join(root, dir)).map((name) =>
          path.join(root, dir, name),
        )
      : [],
  )
}

test('a custom interval is previewed, installed exactly as shown, verified and removed', async ({
  page,
}) => {
  const intervalBefore = await savedInterval()
  expect(await installState()).toBe('not-installed')
  expect(sandboxedSchedulerFiles()).toEqual([])

  await page.goto('/#/backup')
  await unlockIfLocked(page)
  const card = page
    .getByRole('heading', { name: 'Automatic backup' })
    .locator('xpath=ancestor::section[1]')
  await card.getByRole('radio', { name: 'Custom' }).click()

  const dialog = page.getByRole('dialog', { name: 'Turn on automatic backup' })
  await dialog.getByRole('combobox', { name: 'Unit' }).click()
  await page.getByRole('option', { name: 'minutes' }).click()
  await dialog.getByRole('spinbutton', { name: 'How many' }).fill('0')
  await expect(dialog.getByRole('alert')).toContainText(
    'between 1 minute and 30 days',
  )
  await expect(dialog.getByRole('button', { name: 'Install' })).toBeDisabled()
  await dialog
    .getByRole('spinbutton', { name: 'How many' })
    .fill(String(CUSTOM_MINUTES))
  await expect(dialog).toContainText(
    `more than ${CUSTOM_MINUTES} minutes old. The system checks every ${CUSTOM_MINUTES} minutes`,
  )

  const file = dialog.getByLabel('File contents')
  await expect(file).toBeVisible()
  const shown = await file.innerText()
  if (process.platform === 'darwin') {
    // launchd wakes PathKeep every 90 minutes.
    expect(shown).toContain('<key>StartInterval</key>')
    expect(shown).toContain(`<integer>${CUSTOM_MINUTES * 60}</integer>`)
  }

  // Looking changes nothing: no file, no install, the saved interval as before.
  expect(await installState()).toBe('not-installed')
  expect(await savedInterval()).toBe(intervalBefore)
  expect(sandboxedSchedulerFiles()).toEqual([])

  await dialog.getByRole('button', { name: 'Install' }).click()
  await expect(dialog).toBeHidden({ timeout: 30_000 })
  await expect.poll(installState).toBe('installed')
  expect(await savedInterval()).toBe(CUSTOM_MINUTES / 60)
  await expect(card).toContainText(`Backs up every ${CUSTOM_MINUTES} minutes.`)
  await expect(card).toContainText('installed')
  if (process.platform === 'darwin') {
    // The plan for this interval, as the backend builds it; the dialog
    // showed it and the sandboxed LaunchAgents folder now holds it.
    const plan = await backend<{ generatedFiles: { contents: string }[] }>(
      'preview_schedule',
      { platform: null, dueAfterHours: CUSTOM_MINUTES / 60 },
    )
    const expected = plan.generatedFiles[0].contents
    expect(shown.trim(), 'preview = plan').toBe(expected.trim())
    const [plist] = sandboxedSchedulerFiles().filter((file) =>
      file.includes(`${path.sep}LaunchAgents${path.sep}`),
    )
    expect(readFileSync(plist, 'utf8'), 'installed file = plan').toBe(expected)
  } else {
    expect(sandboxedSchedulerFiles()).not.toEqual([])
  }

  // Details: what the backend checked and the change just made.
  await card.getByRole('button', { name: 'Details' }).click()
  const details = page.getByRole('dialog', { name: 'Automatic backup details' })
  await expect(details).toContainText('Last change')
  await expect(details).toContainText('Installed · Done')
  if (process.platform === 'darwin') {
    await expect(details).toContainText('Job loaded')
    await expect(details).toContainText('macOS has the job loaded.')
    await expect(details).toContainText(
      path.join(fixture().dirs.osSandbox, 'LaunchAgents'),
    )
  }
  await page.keyboard.press('Escape')
  await expect(details).toBeHidden()

  // Turn it off: the dialog names what goes; the interval stays.
  await card.getByRole('radio', { name: 'Off' }).click()
  const off = page.getByRole('dialog', { name: 'Turn off automatic backup?' })
  if (process.platform === 'darwin')
    await expect(off).toContainText(
      path.join(fixture().dirs.osSandbox, 'LaunchAgents'),
    )
  expect(await installState()).toBe('installed')
  await off.getByRole('button', { name: 'Turn off' }).click()
  await expect(off).toBeHidden({ timeout: 30_000 })
  await expect.poll(installState).toBe('not-installed')
  expect(sandboxedSchedulerFiles(), 'no task left in the OS').toEqual([])
  expect(await savedInterval()).toBe(CUSTOM_MINUTES / 60)
  await expect(card.getByRole('radio', { name: 'Off' })).toBeChecked()
})
