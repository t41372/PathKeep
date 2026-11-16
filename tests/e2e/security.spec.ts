/**
 * Archive password: changing it needs the current one.
 *
 * Proves: a wrong current password is refused in plain words and changes
 * nothing (the old password still opens the archive and is still the one in
 * the keychain); the right one rewrites the archive so that only the new
 * password opens it, and the keychain follows: when the archive closes under
 * the open window, the window unlocks it again from the keychain and shows
 * the archive, not an empty one. Also that the check lives in the backend: the same request sent
 * straight to the dev bridge, which holds an unlocked session, is refused. And that an archive
 * the right password cannot open (one of its databases is gone) is not called a wrong password:
 * the lock screen says why, keeps what was typed, and opens it once the database is back.
 *
 * "Opens the archive" is asked of the backend with a read-only rekey preview,
 * which tries the password against the archive file and changes nothing.
 *
 * Runs after lock.spec, so app lock is on (passcode 4826). first-run created
 * the archive with OLD and saved it in the keychain.
 */
import { existsSync, renameSync } from 'node:fs'
import { join } from 'node:path'
import { expect, test, type Page } from '@playwright/test'
import { backend, fixture, openArchiveVisitTotal } from './support/fixture'

const OLD = 'correct horse battery'
const NEW = 'a longer passphrase, 2026'

/** Whether `password` opens the archive, asked without changing anything. */
async function opensArchive(password: string) {
  try {
    await backend('preview_rekey_archive', {
      request: { newMode: 'Encrypted', newKey: NEW, currentKey: password },
    })
    return true
  } catch (error) {
    // The backend's typed code; the dialog shows its own words for it.
    expect(String(error)).toContain('archive-password-wrong')
    return false
  }
}

async function unlockAppIfLocked(page: Page) {
  const lock = page.getByRole('heading', { name: 'PathKeep is locked' })
  if (await lock.isVisible().catch(() => false)) {
    await page.getByPlaceholder('Passcode').fill('4826')
    await page.getByPlaceholder('Passcode').press('Enter')
    await expect(lock).toBeHidden()
  }
}

test('changing the archive password needs the current one', async ({
  page,
}) => {
  expect(await opensArchive(OLD)).toBe(true)
  expect(await opensArchive('not the password')).toBe(false)

  // The session is unlocked, yet the bridge refuses without the password.
  await expect(
    backend('rekey_archive', {
      request: { newMode: 'Plaintext', newKey: null },
    }),
  ).rejects.toThrow(
    'Enter the current archive password to change or remove encryption.',
  )

  await page.goto('/#/settings/security')
  await unlockAppIfLocked(page)
  const passwordRow = page
    .locator('div.rounded-xl')
    .filter({ has: page.getByText('Change password', { exact: true }) })
  await passwordRow.getByRole('button', { name: 'Change…' }).click()
  const dialog = page.getByRole('dialog', {
    name: 'Change the archive password',
  })
  const continueButton = dialog.getByRole('button', { name: 'Continue' })

  await dialog.getByLabel('Current password').fill('not the password')
  await dialog.getByLabel('New password').fill(NEW)
  await dialog.getByLabel('Type it again').fill(NEW)
  await expect(dialog.getByRole('checkbox')).toBeChecked()
  await continueButton.click()
  await expect(dialog.getByRole('alert')).toHaveText(
    'That is not the current password. Nothing was changed.',
  )
  await expect(dialog.getByLabel('Current password')).toBeVisible()
  expect(await opensArchive(OLD)).toBe(true)
  expect(await opensArchive(NEW)).toBe(false)
  expect(await backend<boolean>('keyring_get_database_key')).toBe(true)

  await dialog.getByLabel('Current password').fill(OLD)
  await expect(dialog.getByRole('alert')).toBeHidden()
  await continueButton.click()
  await expect(dialog.getByText('What happens')).toBeVisible()
  await dialog.getByRole('button', { name: 'Change password' }).click()
  await expect(page.getByText('Password changed.')).toBeVisible({
    timeout: 60_000,
  })
  await expect(dialog).toBeHidden()

  expect(await opensArchive(NEW)).toBe(true)
  expect(await opensArchive(OLD)).toBe(false)
  expect(await backend<boolean>('keyring_get_database_key')).toBe(true)

  // The archive closes under the open window (no key in memory): Home's first
  // read is refused, and the window has to unlock the archive again with what
  // the keychain holds now, not show an empty archive.
  await backend('clear_session_database_key')
  await page.goto('/#/')
  await unlockAppIfLocked(page)
  const total = await openArchiveVisitTotal()
  expect(total).toBeGreaterThan(0)
  await expect(page.getByRole('link', { name: /Saved visits/ })).toContainText(
    total.toLocaleString('en-US'),
  )
  await expect(
    page.getByRole('heading', { name: 'Your archive is encrypted' }),
  ).toBeHidden()
})

test('an archive the right password cannot open is not called a wrong password', async ({
  page,
}) => {
  // One of the archive's databases moved away while the archive is closed, as a partial copy
  // of the folder would leave it, with its log files if it has them.
  const store = join(fixture().dirs.projectRoot, 'archive', 'agent-history.db')
  const moved = ['', '-wal', '-shm'].filter((suffix) =>
    existsSync(store + suffix),
  )
  expect(moved).toContain('')
  // Nothing in memory and nothing in the keychain: the window asks for the password.
  await backend('keyring_clear_database_key')
  await backend('clear_session_database_key')
  for (const suffix of moved)
    renameSync(store + suffix, `${store}${suffix}.aside`)
  const asked = page.getByRole('heading', { name: 'Your archive is encrypted' })
  try {
    await page.goto('/#/')
    await unlockAppIfLocked(page)
    await expect(asked).toBeVisible()
    await page.getByPlaceholder('Password').fill(NEW)
    await page.getByPlaceholder('Password').press('Enter')
    await expect(
      page.getByText(
        /^PathKeep couldn’t open the archive\. .*archive-store-missing/,
      ),
    ).toBeVisible({ timeout: 60_000 })
    await expect(
      page.getByText('That did not work. Check it and try again.'),
    ).toHaveCount(0)
    // The password was not the problem, so it is still there to try again.
    await expect(page.getByPlaceholder('Password')).toHaveValue(NEW)
  } finally {
    for (const suffix of moved)
      renameSync(`${store}${suffix}.aside`, store + suffix)
  }
  await page.getByPlaceholder('Password').press('Enter')
  await expect(asked).toBeHidden({ timeout: 60_000 })
  expect(await openArchiveVisitTotal()).toBeGreaterThan(0)
  // As the specs after this one expect: the keychain remembers the password again.
  expect(await backend<boolean>('keyring_get_database_key')).toBe(true)
})
