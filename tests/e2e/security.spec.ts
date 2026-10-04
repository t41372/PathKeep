/**
 * Archive password: changing it needs the current one.
 *
 * Proves: a wrong current password is refused in plain words and changes
 * nothing (the old password still opens the archive and is still the one in
 * the keychain); the right one rewrites the archive so that only the new
 * password opens it, the keychain follows, and a fresh session unlocks with
 * it. Also that the check lives in the backend: the same request sent
 * straight to the dev bridge, which holds an unlocked session, is refused.
 *
 * "Opens the archive" is asked of the backend with a read-only rekey preview,
 * which tries the password against the archive file and changes nothing.
 *
 * Runs after lock.spec, so app lock is on (passcode 4826). first-run created
 * the archive with OLD and saved it in the keychain.
 */
import { expect, test, type Page } from '@playwright/test'
import { backend } from './support/fixture'

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
    expect(String(error)).toContain(
      'The current archive password did not open the archive.',
    )
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
  expect(await backend<string | null>('keyring_get_database_key')).toBe(OLD)

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
  expect(await backend<string | null>('keyring_get_database_key')).toBe(NEW)

  // A fresh session: no key in memory, so PathKeep has to unlock the
  // archive with what the keychain holds now.
  await backend('clear_session_database_key')
  await page.goto('/#/')
  await unlockAppIfLocked(page)
  await expect(page.getByRole('link', { name: /Saved visits/ })).toBeVisible()
  await expect(
    page.getByRole('heading', { name: 'Your archive is encrypted' }),
  ).toBeHidden()
})
