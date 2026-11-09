/**
 * Moving to another computer: the file is encrypted with a password chosen at export.
 *
 * Proves: export asks for that password twice and won't run until both match;
 * the file opens only with that password (a wrong one is refused in plain words
 * and the archive here is untouched); opening it replaces the archive with the
 * same visits, and from then on that password is the archive's password and
 * the one this computer's keychain remembers.
 *
 * Runs in its own project after `change` (app lock is on, passcode 4826):
 * opening the file replaces the archive and its password.
 */
import { mkdirSync } from 'node:fs'
import { join } from 'node:path'
import { expect, test, type Page } from '@playwright/test'
import { backend, fixture, openArchiveVisitTotal } from './support/fixture'

const FILE_PASSWORD = 'moving day, 2026'

async function visitTotal() {
  const dashboard = await backend<{ totalVisits: number }>(
    'load_dashboard_snapshot',
  )
  return dashboard.totalVisits
}

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

/** The app lock, then the archive password if this session doesn't have it. */
async function unlockIfAsked(page: Page, password: string) {
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
    await page.getByPlaceholder('Password').fill(password)
    await page.getByPlaceholder('Password').press('Enter')
    await expect(archive).toBeHidden()
  }
}

test('a moved archive opens only with the password chosen at export', async ({
  page,
}) => {
  // Export and import each write the whole archive, encrypted, and the debug
  // build does that slowly: on CI the two took 97 s of the default two minutes.
  test.setTimeout(600_000)
  const dir = join(fixture().root, 'move')
  mkdirSync(dir, { recursive: true })
  const file = join(dir, 'pathkeep-move.pathkeep')

  // security.spec ended on a fresh session; the window reopens the archive.
  await page.goto('/#/settings/storage')
  await unlockIfAsked(page, FILE_PASSWORD)
  const before = await openArchiveVisitTotal()
  expect(before).toBeGreaterThan(0)
  // Export history has an "Export…" too.
  await page
    .getByText('Move to another computer', { exact: true })
    .locator('xpath=../..')
    .getByRole('button', { name: 'Export…' })
    .click()
  const exporting = page.getByRole('dialog')
  await exporting.getByLabel('Save to').fill(file)
  await exporting.getByLabel('Password for this file').fill(FILE_PASSWORD)
  await exporting.getByLabel('Type it again').fill('something else')
  await expect(
    exporting.getByText('The two passwords don’t match.'),
  ).toBeVisible()
  const exportButton = exporting.getByRole('button', {
    name: 'Export',
    exact: true,
  })
  await expect(exportButton).toBeDisabled()
  await exporting.getByLabel('Type it again').fill(FILE_PASSWORD)
  await exportButton.click()
  await expect(page.getByText(/^Saved /)).toBeVisible({ timeout: 120_000 })
  await expect(exporting).toHaveCount(0)

  await page.getByRole('button', { name: 'Import…' }).click()
  const importing = page.getByRole('dialog')
  await importing.getByLabel('File').fill(file)
  await importing.getByRole('button', { name: 'Check file' }).click()
  const replace = importing.getByRole('button', { name: 'Replace my archive' })
  await importing.getByLabel('Password of this file').fill('not the password')
  await replace.click()
  await expect(
    importing.getByText('That password doesn’t open this file.'),
  ).toBeVisible({ timeout: 60_000 })
  expect(await visitTotal()).toBe(before)

  await importing.getByLabel('Password of this file').fill(FILE_PASSWORD)
  await replace.click()
  await expect(page.getByText('Import finished.')).toBeVisible({
    timeout: 300_000,
  })
  await unlockIfAsked(page, FILE_PASSWORD)
  await expect.poll(visitTotal, { timeout: 60_000 }).toBe(before)
  expect(await opensArchive(FILE_PASSWORD)).toBe(true)
  expect(await opensArchive('not the password')).toBe(false)

  // Remembering the password is this computer's choice (first-run turned it
  // on), so the keychain now holds the moved archive's key: with the session
  // key gone, the window opens the archive from the keychain alone.
  expect(await backend<boolean>('keyring_get_database_key')).toBe(true)
  await backend('clear_session_database_key')
  await page.goto('/#/history')
  const appLock = page.getByRole('heading', { name: 'PathKeep is locked' })
  if (await appLock.isVisible().catch(() => false)) {
    await page.getByPlaceholder('Passcode').fill('4826')
    await page.getByPlaceholder('Passcode').press('Enter')
  }
  expect(await openArchiveVisitTotal()).toBe(before)
  await expect(
    page.getByRole('heading', { name: 'Your archive is encrypted' }),
  ).toBeHidden()
})
