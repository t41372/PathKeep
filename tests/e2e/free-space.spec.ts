/**
 * Free up space lists every safety copy on its own and deletes exactly what is
 * ticked, the newest copy too.
 *
 * Proves: right after a restore, the copy it made ("Before a restore", the
 * newest, as big as the archive) can be freed on its own; the size on the
 * button is the size of what is ticked; the older copy ("Before an import")
 * stays and can still be restored from; the archive's history is untouched.
 * Before 2026-11-15 the newest copy was always kept and every other one went.
 *
 * Runs after restore.spec (app lock is on, passcode 4826): the "Before an
 * import" copy (the archive the move replaced) and the "Before a restore" copy
 * are both there.
 */
import { expect, test, type Page } from '@playwright/test'
import { backend, openArchiveVisitTotal } from './support/fixture'

/** move.spec's file password, the archive's password since. */
const CURRENT = 'moving day, 2026'

interface Copy {
  sourceOp: string
  verifiedOpenable: boolean
}

async function unlockIfAsked(page: Page) {
  const appLock = page.getByRole('heading', { name: 'PathKeep is locked' })
  if (await appLock.isVisible().catch(() => false)) {
    await page.getByPlaceholder('Passcode').fill('4826')
    await page.getByPlaceholder('Passcode').press('Enter')
    await expect(appLock).toBeHidden()
  }
  const archive = page.getByRole('heading', {
    name: 'Your archive is encrypted',
  })
  if (await archive.isVisible().catch(() => false)) {
    await page.getByPlaceholder('Password').fill(CURRENT)
    await page.getByPlaceholder('Password').press('Enter')
    await expect(archive).toBeHidden()
  }
}

test('the copy a restore made is freed on its own', async ({ page }) => {
  await page.goto('/#/settings/storage')
  await unlockIfAsked(page)
  const before = await openArchiveVisitTotal()
  const listed = (copies: Copy[], op: string) =>
    copies.filter((copy) => copy.sourceOp === op && copy.verifiedOpenable)
  const copiesBefore = await backend<Copy[]>('list_recovery_snapshots')
  expect(listed(copiesBefore, 'restore')).toHaveLength(1)
  expect(listed(copiesBefore, 'import')).toHaveLength(1)

  await page
    .getByText('Free up space', { exact: true })
    .locator('xpath=../..')
    .getByRole('button', { name: 'Review…' })
    .click()
  const dialog = page.getByRole('dialog')
  const restoreCopy = dialog.getByRole('checkbox', {
    name: /^Before a restore/,
  })
  const importCopy = dialog.getByRole('checkbox', { name: /^Before an import/ })
  await expect(restoreCopy).not.toBeChecked()
  await expect(importCopy).not.toBeChecked()

  // Only the copy: leftovers are ticked by default, so untick them.
  for (const leftover of await dialog
    .getByRole('checkbox', { checked: true })
    .all()) {
    await leftover.click()
  }
  const deleteButton = dialog.getByRole('button', { name: /^Delete / })
  await expect(deleteButton).toBeDisabled()
  await restoreCopy.click()
  await expect(
    dialog.getByText('You can’t restore from a safety copy once it’s deleted.'),
  ).toBeVisible()
  // The button says what the ticked row says.
  const rowSize = await dialog
    .getByRole('listitem')
    .filter({
      has: page.getByRole('checkbox', { name: /^Before a restore/ }),
    })
    .locator('.font-mono')
    .innerText()
  await expect(deleteButton).toHaveText(`Delete ${rowSize}`)

  await deleteButton.click()
  await expect(page.getByText(`Freed ${rowSize}.`)).toBeVisible({
    timeout: 60_000,
  })

  const copiesAfter = await backend<Copy[]>('list_recovery_snapshots')
  expect(
    copiesAfter.filter((copy) => copy.sourceOp === 'restore'),
  ).toHaveLength(0)
  expect(listed(copiesAfter, 'import')).toHaveLength(1)
  expect(await openArchiveVisitTotal()).toBe(before)
})
