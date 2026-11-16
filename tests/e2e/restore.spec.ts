/**
 * Restoring from a safety copy: history comes back, the archive keeps its own
 * password and encryption, and the archive it replaces becomes a copy too.
 *
 * Proves: the archive the move replaced is listed as "Before an import"; it is
 * restored with the password it had then, yet afterwards only the current
 * password opens the archive, and the key this computer's keychain remembers
 * still opens it on its own; the archive the restore replaced is listed as
 * "Before a restore", so the restore can be undone the same way. A copy nothing
 * has checked yet is listed as such and checked before it is restored: one
 * that can't be checked is not restored and nothing changes; when the copy a
 * restore makes can't be checked, the restore still finishes and says why.
 *
 * Runs after move.spec (app lock is on, passcode 4826): the archive's password
 * is the moved file's, and the archive the move replaced, with security.spec's
 * password, is the "Before an import" copy, which move.spec left unchecked.
 */
import { expect, test, type Page } from '@playwright/test'
import { backend, injectFault, openArchiveVisitTotal } from './support/fixture'

/** move.spec's file password, the archive's password since. */
const CURRENT = 'moving day, 2026'
/** security.spec's, the password of the archive the move replaced. */
const BEFORE_THE_MOVE = 'a longer passphrase, 2026'

interface Copy {
  sourceOp: string
  verifiedOpenable: boolean
  unchecked: boolean
}

/** The toast an operation shows when the copy it kept could not be checked. */
const COPY_UNCHECKED =
  /^Your previous archive is kept as a safety copy, but PathKeep couldn’t check it: .*simulated error/

/** Whether `password` opens the archive, asked without changing anything. */
async function opensArchive(password: string) {
  try {
    await backend('preview_rekey_archive', {
      request: {
        newMode: 'Encrypted',
        newKey: 'unused new password',
        currentKey: password,
      },
    })
    return true
  } catch {
    return false
  }
}

async function unlockAppLock(page: Page) {
  const appLock = page.getByRole('heading', { name: 'PathKeep is locked' })
  if (await appLock.isVisible().catch(() => false)) {
    await page.getByPlaceholder('Passcode').fill('4826')
    await page.getByPlaceholder('Passcode').press('Enter')
    await expect(appLock).toBeHidden()
  }
}

async function unlockIfAsked(page: Page) {
  await unlockAppLock(page)
  const archive = page.getByRole('heading', {
    name: 'Your archive is encrypted',
  })
  if (await archive.isVisible().catch(() => false)) {
    await page.getByPlaceholder('Password').fill(CURRENT)
    await page.getByPlaceholder('Password').press('Enter')
    await expect(archive).toBeHidden()
  }
}

async function openRestoreDialog(page: Page, unlock = unlockIfAsked) {
  await page.goto('/#/settings/storage')
  await unlock(page)
  await page
    .getByText('Restore from a safety copy', { exact: true })
    .locator('xpath=../..')
    .getByRole('button', { name: 'Choose…' })
    .click()
  return page.getByRole('dialog')
}

test('a restore keeps the archive password and can itself be undone', async ({
  page,
}) => {
  // Two restores and an undo copy, check and index the whole archive in a
  // debug build.
  test.setTimeout(900_000)
  const dialog = await openRestoreDialog(page)
  const before = await openArchiveVisitTotal()
  expect(before).toBeGreaterThan(0)
  expect(await opensArchive(BEFORE_THE_MOVE)).toBe(false)

  const beforeImport = dialog.getByRole('radio', { name: /^Before an import/ })
  await expect(beforeImport).toHaveAccessibleName(/Not checked yet/)
  await beforeImport.click()
  await expect(
    dialog.getByText(/^This copy hasn’t been checked yet\./),
  ).toBeVisible()
  await expect(dialog.getByText(/Your current password stays/)).toBeVisible()
  await dialog.getByLabel('Password of this copy').fill(BEFORE_THE_MOVE)
  const restore = dialog.getByRole('button', { name: 'Restore', exact: true })

  // A copy that can't be checked is not restored, and nothing changes.
  let heal = injectFault('native-seal-before-import')
  try {
    await restore.click()
    await expect(
      dialog.getByText(/^The restore did not finish\..*could not be checked/),
    ).toBeVisible({ timeout: 120_000 })
  } finally {
    heal()
  }
  expect(await openArchiveVisitTotal()).toBe(before)
  expect(await opensArchive(CURRENT)).toBe(true)

  // Once it can be, the restore goes ahead. The archive it replaces is kept as
  // a copy in turn, and when that one can't be checked the restore still
  // finishes and says why.
  heal = injectFault('native-seal-before-restore')
  try {
    await restore.click()
    await expect(page.getByText('Archive restored.')).toBeVisible({
      timeout: 300_000,
    })
    await expect(page.getByText(COPY_UNCHECKED)).toBeVisible()
  } finally {
    heal()
  }
  const warnedAt = Date.now()
  await unlockIfAsked(page)
  await expect.poll(openArchiveVisitTotal, { timeout: 60_000 }).toBe(before)

  // The copy's password opened the copy; the archive keeps its own.
  expect(await opensArchive(CURRENT)).toBe(true)
  expect(await opensArchive(BEFORE_THE_MOVE)).toBe(false)
  const copies = await backend<Copy[]>('list_recovery_snapshots')
  const listed = (op: string) =>
    copies
      .filter((copy) => copy.sourceOp === op && copy.verifiedOpenable)
      .map((copy) => copy.unchecked)
  // The restore checked the copy it restored from; the one it made is not yet.
  expect(listed('import')).toEqual([false])
  expect(listed('restore')).toEqual([true])

  // The same key, too: with the session key gone, the window opens the archive
  // from the keychain alone when it loads a page of history.
  expect(await backend<boolean>('keyring_get_database_key')).toBe(true)
  await backend('clear_session_database_key')
  await page.goto('/#/browsing/history')
  await unlockAppLock(page)
  expect(await openArchiveVisitTotal()).toBe(before)
  await expect(
    page.getByRole('heading', { name: 'Your archive is encrypted' }),
  ).toBeHidden()

  // The warning stays until it is closed, however long the user was away:
  // past the 20 seconds it once had, it is still there.
  await page.waitForTimeout(Math.max(0, 21_000 - (Date.now() - warnedAt)))
  const warning = page
    .locator('[data-sonner-toast]')
    .filter({ hasText: 'Archive restored.' })
  await expect(warning.getByText(COPY_UNCHECKED)).toBeVisible()
  await warning.getByRole('button', { name: 'Close' }).click()
  await expect(warning).toHaveCount(0)

  // Undone from the copy the restore made, which is checked first.
  const undo = await openRestoreDialog(page, unlockAppLock)
  const beforeRestore = undo.getByRole('radio', { name: /^Before a restore/ })
  await expect(beforeRestore).toHaveAccessibleName(/Not checked yet/)
  await beforeRestore.click()
  await undo.getByLabel('Password of this copy').fill(CURRENT)
  await undo.getByRole('button', { name: 'Restore', exact: true }).click()
  await expect(page.getByText('Archive restored.')).toBeVisible({
    timeout: 300_000,
  })
  await expect(page.getByText(COPY_UNCHECKED)).toHaveCount(0)
  await unlockIfAsked(page)
  await expect.poll(openArchiveVisitTotal, { timeout: 60_000 }).toBe(before)
  expect(await opensArchive(CURRENT)).toBe(true)
  const after = await backend<Copy[]>('list_recovery_snapshots')
  expect(
    after
      .filter((copy) => copy.sourceOp === 'restore')
      .map((copy) => copy.unchecked),
  ).toEqual([false, false])
})
