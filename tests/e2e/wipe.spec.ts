/**
 * Delete all data: the preview says what goes, typing the word deletes it,
 * and PathKeep starts over at onboarding.
 *
 * Proves: the preview's visit count is the archive's real size (the first
 * backup plus the nine added in backup.spec), the archive and the stored key
 * are gone afterwards, and the browsers' own history files are untouched.
 */
import { expect, test } from '@playwright/test'
import {
  archivedVisits,
  backend,
  browserFileHashes,
  expectCount,
  parseCount,
} from './support/fixture'

test('delete all data previews, deletes, and returns to onboarding', async ({
  page,
}, testInfo) => {
  const before = browserFileHashes()
  const expectedVisits = archivedVisits().length + 9

  await page.goto('/#/settings/storage')
  // App lock is on from lock.spec; unlock if the reload locked it.
  const lock = page.getByRole('heading', { name: 'PathKeep is locked' })
  if (await lock.isVisible().catch(() => false)) {
    await page.getByPlaceholder('Passcode').fill('4826')
    await page.getByPlaceholder('Passcode').press('Enter')
  }
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
  expect(browserFileHashes(), 'browser files must not change').toEqual(before)
})
