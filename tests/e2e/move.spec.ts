/**
 * Moving to another computer: the file is encrypted with a password chosen at export.
 *
 * Proves: export says the safety copies stay on this computer, asks for that
 * password twice and won't run until both match;
 * a file made by a newer PathKeep is refused at the check, in words that say
 * what to do, and the archive here is untouched; the file opens only with that
 * password (a wrong one is refused in plain words and the archive here is
 * untouched); opening it replaces the archive with the
 * same visits, and from then on that password is the archive's password and
 * the one this computer's keychain remembers. When the archive it replaced,
 * kept as a safety copy, can't be checked, the import still finishes and says
 * why, and the copy is listed as not checked yet (restore.spec restores it).
 *
 * And what the file does not carry, after the move (the second test):
 * - the folders approved for Agent conversations on this computer: one
 *   more folder is approved after the file is made, so the file does not
 *   hold this computer's approvals. After the move the kind is still on,
 *   with nothing to back up (`sourceReviewRequired`): the backup page asks
 *   to choose the folders again, without "Back up now", and "Choose
 *   folders…" opens the look at the folders; Manage data types says it too;
 * - links to browsing backups made before it (`browsing:{origin}/{id}`): the
 *   archive's ledger starts again, so such a link says the backup is not in
 *   the archive instead of opening whichever backup now has that number.
 *
 * Runs in its own project after `change` (app lock is on, passcode 4826):
 * opening the file replaces the archive and its password. The two tests run
 * in order; the second needs what the first did.
 */
import { createHash } from 'node:crypto'
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { expect, test, type Page } from '@playwright/test'
import {
  addAgentFolder,
  agentKindState,
  openBackupPage,
} from './support/agent-backup'
import { enableDialog, waitForPreview } from './support/agent-tools'
import {
  backend,
  fixture,
  injectFault,
  openArchiveVisitTotal,
} from './support/fixture'
import { zipBytes, zipEntries } from './support/zip-file'

const FILE_PASSWORD = 'moving day, 2026'

test.describe.configure({ mode: 'serial' })

/** A browsing backup's link from before the move, for the second test. */
let linkBeforeMove: string | undefined

/** The newest browsing backup's public reference. */
async function newestBrowsingRunRef() {
  const page = await backend<{ items: { runRef: string }[] }>(
    'list_archive_runs',
    { request: { space: 'browsing', includeArchiveWide: false, limit: 1 } },
  )
  return page.items[0]?.runRef
}

async function visitTotal() {
  const dashboard = await backend<{ totalVisits: number }>(
    'load_dashboard_snapshot',
  )
  return dashboard.totalVisits
}

/**
 * `file` as a newer PathKeep would write it: its manifest says a format this
 * build does not have yet, and the digest the file keeps of its manifest
 * matches.
 */
function madeByNewerVersion(file: string, copy: string) {
  const entries = zipEntries(readFileSync(file))
  const at = (name: string) => entries.findIndex(([entry]) => entry === name)
  const manifestAt = at('pathkeep-export-manifest.json')
  const manifest = JSON.parse(entries[manifestAt][1].toString('utf8')) as {
    formatVersion: number
    appVersion: string
  }
  manifest.formatVersion += 1
  manifest.appVersion = '9.0.0'
  const bytes = Buffer.from(JSON.stringify(manifest))
  entries[manifestAt] = ['pathkeep-export-manifest.json', bytes]
  entries[at('pathkeep-export-manifest.sha256')] = [
    'pathkeep-export-manifest.sha256',
    Buffer.from(createHash('sha256').update(bytes).digest('hex')),
  ]
  writeFileSync(copy, zipBytes(entries))
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
  // Scoped to its row, so no other "Export…" can be taken for it.
  await page
    .getByText('Move to another computer', { exact: true })
    .locator('xpath=../..')
    .getByRole('button', { name: 'Export…' })
    .click()
  const exporting = page.getByRole('dialog')
  // This computer's restore points don't go into the file.
  await expect(exporting).toContainText(
    'Your app passcode, the backup schedule and the safety copies stay on this computer.',
  )
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

  // After the file is made: a link to a browsing backup, and one more Agent
  // folder approved here, which the file does not hold.
  linkBeforeMove = await newestBrowsingRunRef()
  expect(linkBeforeMove).toMatch(/^browsing:[0-9a-f]+\/\d+$/)
  expect((await agentKindState())?.state).toBe('on')
  await addAgentFolder(page, 'pi', join(fixture().agents.home, 'pi-elsewhere'))
  await page.goto('/#/settings/storage')
  await unlockIfAsked(page, FILE_PASSWORD)

  // The same file from a newer PathKeep: refused at the check, saying what to do.
  const newer = join(dir, 'pathkeep-move-newer.pathkeep')
  madeByNewerVersion(file, newer)
  await page.getByRole('button', { name: 'Import…' }).click()
  const importing = page.getByRole('dialog')
  await importing.getByLabel('File').fill(newer)
  await importing.getByRole('button', { name: 'Check file' }).click()
  await expect(
    importing.getByText(
      'This file was made by a newer version of PathKeep. Update PathKeep in Settings → About, then open the file again. Your archive was not changed.',
    ),
  ).toBeVisible({ timeout: 60_000 })
  expect(await visitTotal()).toBe(before)

  await importing.getByLabel('File').fill(file)
  await importing.getByRole('button', { name: 'Check file' }).click()
  // Agent conversations are on here: the check says they stay on and that
  // the folders are chosen again.
  await expect(importing).toContainText(/Agent conversations\s*On/)
  await expect(importing).toContainText(
    'Any Agent conversations in the file come along, and Agent conversations stay on here. If the file doesn’t carry which folders this computer backs up, you choose them again after opening it.',
  )
  const replace = importing.getByRole('button', { name: 'Replace my archive' })
  await importing.getByLabel('Password of this file').fill('not the password')
  await replace.click()
  await expect(
    importing.getByText('That password doesn’t open this file.'),
  ).toBeVisible({ timeout: 60_000 })
  expect(await visitTotal()).toBe(before)

  const healSeal = injectFault('native-seal-before-import')
  try {
    await importing.getByLabel('Password of this file').fill(FILE_PASSWORD)
    await replace.click()
    await expect(page.getByText('Import finished.')).toBeVisible({
      timeout: 300_000,
    })
    const warning = page
      .locator('[data-sonner-toast]')
      .filter({ hasText: 'Import finished.' })
    await expect(
      warning.getByText(
        /^Your previous archive is kept as a safety copy, but PathKeep couldn’t check it: .*simulated error/,
      ),
    ).toBeVisible()
    await warning.getByRole('button', { name: 'Close' }).click()
    await expect(warning).toHaveCount(0)
  } finally {
    healSeal()
  }
  const copies = await backend<
    { sourceOp: string; verifiedOpenable: boolean; unchecked: boolean }[]
  >('list_recovery_snapshots')
  expect(copies.filter((copy) => copy.sourceOp === 'import')).toEqual([
    expect.objectContaining({ verifiedOpenable: true, unchecked: true }),
  ])
  await unlockIfAsked(page, FILE_PASSWORD)
  await expect.poll(visitTotal, { timeout: 60_000 }).toBe(before)
  expect(await opensArchive(FILE_PASSWORD)).toBe(true)
  expect(await opensArchive('not the password')).toBe(false)

  // Remembering the password is this computer's choice (first-run turned it
  // on), so the keychain now holds the moved archive's key: with the session
  // key gone, the window opens the archive from the keychain alone.
  expect(await backend<boolean>('keyring_get_database_key')).toBe(true)
  await backend('clear_session_database_key')
  await page.goto('/#/browsing/history')
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

test('after the move, this computer chooses its folders again, and older links say so', async ({
  page,
}, testInfo) => {
  expect(linkBeforeMove, 'the first test ran').toBeDefined()

  // On, with no folder approved here: choose them again.
  expect(await agentKindState()).toMatchObject({
    state: 'on',
    hasBackupSource: false,
    sourceReviewRequired: true,
  })
  await openBackupPage(page)
  await expect(
    page.getByText('Choose the folders to back up again', { exact: true }),
  ).toBeVisible()
  await expect(page.getByRole('button', { name: 'Back up now' })).toHaveCount(0)
  await expect(
    page.getByText(
      'Nothing on this computer is backed up. Set up backup to choose which tools to keep.',
    ),
  ).toBeVisible()
  await testInfo.attach('the backup page after the move', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })
  await page.getByRole('button', { name: 'Choose folders…' }).click()
  await expect(enableDialog(page)).toBeVisible()
  await waitForPreview(page)
  await page.keyboard.press('Escape')
  await expect(enableDialog(page)).toHaveCount(0)

  await page.goto('/#/settings/kinds')
  await expect(
    page.getByRole('region', { name: 'Agent conversations', exact: true }),
  ).toContainText(
    'On, but nothing on this computer is chosen to back up, so nothing is backed up. Everything saved is still here.',
  )

  // A browsing backup's link from before: not in this archive. The link
  // opens a sheet over the page, which hides the page from a screen reader,
  // so the page is opened (and unlocked) first and the link followed in it.
  await openBackupPage(page)
  const link = `?run=${encodeURIComponent(linkBeforeMove!)}`
  await page.evaluate((query) => {
    window.location.hash = `/agent/backup${query}`
  }, link)
  const sheet = page.getByRole('dialog')
  await expect(sheet).toContainText('This backup isn’t in the archive.')
  await testInfo.attach('a link from before the move', {
    body: await sheet.screenshot(),
    contentType: 'image/png',
  })
  await expect(
    backend('get_archive_run', { request: { runRef: linkBeforeMove } }),
  ).rejects.toThrow(/record-unavailable/)
})
