/**
 * Going back to PathKeep 0.5.0, and coming back: PathKeep's own side of it.
 * (The released 0.5.0 binary isn't available to E2E; the Rust suite runs it
 * against a prepared archive, `group5::released_binary_can_back_up_into_the_handoff`.)
 *
 * Proves:
 * - Storage's "Go back to PathKeep 0.5.0" shows, before anything changes,
 *   where the conversations go (a folder next to the data folder, the same
 *   one the backend names), how much they take, every copy this version
 *   made that goes with them and none that 0.5.0 made, and what 0.5.0 will
 *   and won't see;
 * - preparing leaves the ordinary screens for one that says to quit and
 *   open 0.5.0, with the choice folded away; on disk the settings file is
 *   in 0.5.0's form, both conversation files and those copies are in the
 *   package and gone from the data folder, 0.5.0's own copy stayed, and
 *   the desktop answers only recovery commands;
 * - the next start asks before opening: a wrong password is refused and
 *   changes nothing; with the right one the conversations come back, the
 *   settings file is this version's again and the package's conversation
 *   folder is gone; the screen offers to restart;
 * - after the restart the archive opens with every visit and every
 *   conversation as before, and the copies that went into the package are
 *   still listed for restoring, by their place in it.
 *
 * Runs after wipe-notice.spec: encrypted with move.spec's file password,
 * Pi's conversations visible and an import's in the trash, App Lock 4826.
 */
import { existsSync } from 'node:fs'
import path from 'node:path'
import { expect, test } from '@playwright/test'
import { wireCounts, wireList } from './support/agent-chats'
import { killAndRestartApp } from './support/app'
import {
  accessScreen,
  archiveAccess,
  dataRoot,
  enterApp,
  loadFresh,
  MOVE_FILE_PASSWORD,
  openAccessScreen,
  readControl,
  safetyCopies,
  storageRow,
  v05Expected,
} from './support/archive-recovery'
import { backend, openArchiveVisitTotal } from './support/fixture'

interface RollbackPreview {
  targetVersion: string
  packageFolder: string
  encrypted: boolean
  conversationBytes: number
  copyFoldersAndFiles: string[]
  needsChoice: boolean
}

const CONVERSATION_FILES = [
  'archive/agent-history.db',
  'derived/agent-history-read-model.db',
]

test('prepared for PathKeep 0.5.0, then taken back on the next start', async ({
  page,
}, testInfo) => {
  test.setTimeout(900_000)
  await page.goto('/#/settings/storage')
  await enterApp(
    page,
    MOVE_FILE_PASSWORD,
    page.getByRole('heading', { name: 'Storage' }),
  )
  const visits = await openArchiveVisitTotal()
  const counts = await wireCounts()
  const visibleIds = (await wireList()).map((row) => row.id).sort()
  expect(counts.savedRoots).toBeGreaterThan(counts.spaceRoots)
  for (const file of CONVERSATION_FILES)
    expect(existsSync(path.join(dataRoot(), file)), file).toBe(true)
  // PathKeep 0.5.0's own copy (its move file brought it in upgrade-v05.spec)
  // stays where it is: 0.5.0 can restore from it.
  const oldCopy = (await safetyCopies()).find(
    (copy) => path.basename(copy.path) === v05Expected().safetyCopy.name,
  )
  expect(oldCopy).toBeTruthy()

  // What preparing would do, before anything changes.
  const preview = await backend<RollbackPreview>('preview_archive_rollback', {})
  expect(preview.targetVersion).toBe('0.5.0')
  expect(preview.needsChoice).toBe(false)
  expect(preview.encrypted).toBe(true)
  expect(preview.copyFoldersAndFiles.length).toBeGreaterThan(0)
  expect(
    path.resolve(preview.packageFolder).startsWith(dataRoot() + path.sep),
    'the package is next to the data folder, not in it',
  ).toBe(false)
  await storageRow(page, 'Go back to PathKeep 0.5.0')
    .getByRole('button', { name: 'Prepare…' })
    .click()
  const dialog = page.getByRole('dialog', {
    name: 'Prepare for PathKeep 0.5.0',
  })
  await expect(dialog).toContainText(
    'PathKeep 0.5.0 sees all your browsing history, including what you back up with this version, and can keep backing up.',
    { timeout: 60_000 },
  )
  await expect(dialog.locator('[data-rollback-package]')).toHaveText(
    preview.packageFolder,
  )
  const listed = dialog.getByRole('list', {
    name: /^These safety copies and move files were made by this version/,
  })
  for (const item of preview.copyFoldersAndFiles)
    await expect(listed).toContainText(item)
  expect(
    preview.copyFoldersAndFiles.some(
      (item) =>
        path.join(dataRoot(), item) === oldCopy!.path ||
        oldCopy!.path.startsWith(path.join(dataRoot(), item) + path.sep),
    ),
    '0.5.0’s own copy is not moved',
  ).toBe(false)
  await expect(dialog).toContainText(
    'PathKeep 0.5.0’s “Delete all data” doesn’t reach that folder.',
  )
  await expect(dialog).toContainText(
    'The conversations keep the archive’s password as it is now, even if you change the password in PathKeep 0.5.0.',
  )
  await testInfo.attach('the preview', {
    body: await dialog.screenshot(),
    contentType: 'image/png',
  })
  expect(typeof readControl().archiveMode, 'nothing changed yet').toBe('object')

  await dialog
    .getByRole('button', { name: 'Prepare for PathKeep 0.5.0' })
    .click()
  await expect(
    page.getByRole('heading', { name: 'Ready for PathKeep 0.5.0' }),
  ).toBeVisible({ timeout: 300_000 })
  const changedMind = page.getByRole('button', {
    name: 'Changed your mind? You can take the conversations back now instead.',
  })
  await expect(changedMind).toHaveAttribute('aria-expanded', 'false')
  await testInfo.attach('ready for 0.5.0', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })

  // On disk: 0.5.0's settings file, the package holds what it can't use.
  expect(readControl().archiveMode).toBe('Encrypted')
  const prepared = await backend<RollbackPreview>(
    'preview_archive_rollback',
    {},
  )
  expect(prepared.needsChoice).toBe(true)
  const pkg = prepared.packageFolder
  expect(path.dirname(pkg)).toBe(path.resolve(preview.packageFolder))
  for (const file of CONVERSATION_FILES) {
    expect(existsSync(path.join(dataRoot(), file)), file).toBe(false)
    expect(existsSync(path.join(pkg, 'agent-history', file)), file).toBe(true)
  }
  for (const item of preview.copyFoldersAndFiles) {
    expect(existsSync(path.join(dataRoot(), item)), item).toBe(false)
    expect(existsSync(path.join(pkg, 'copies', item)), item).toBe(true)
  }
  expect(existsSync(oldCopy!.path)).toBe(true)
  await expect(backend('app_snapshot')).rejects.toThrow(/archive-recovery-only/)

  // The next start asks first.
  await killAndRestartApp()
  const screen = await openAccessScreen(page)
  await expect(
    screen.getByRole('heading', {
      name: 'Your Agent conversations were set aside for PathKeep 0.5.0',
    }),
  ).toBeVisible()
  expect(await archiveAccess()).toMatchObject({
    state: 'needsRecovery',
    recoveryOnly: true,
    code: 'archive-handoff-choice-required',
  })
  await expect(
    screen.locator('[data-path]').filter({ hasText: pkg }),
  ).toHaveCount(1)
  const takeBack = screen.getByRole('radio', { name: /^Take them back/ })
  await expect(takeBack).toBeChecked()
  await expect(
    screen.getByRole('radio', { name: /^Start without them/ }),
  ).toBeVisible()
  await testInfo.attach('the choice on the next start', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })

  const password = screen.getByLabel('Archive password')
  const confirm = screen.getByRole('button', {
    name: 'Take them back',
    exact: true,
  })
  await password.fill('not the password')
  await confirm.click()
  await expect(
    screen.getByText(
      'That password doesn’t open the archive or the set-aside conversations.',
    ),
  ).toBeVisible({ timeout: 120_000 })
  expect(readControl().archiveMode).toBe('Encrypted')
  for (const file of CONVERSATION_FILES)
    expect(existsSync(path.join(dataRoot(), file)), file).toBe(false)

  await password.fill(MOVE_FILE_PASSWORD)
  await confirm.click()
  await expect(
    screen.getByText(
      'Your Agent conversations are back. Restart PathKeep to open the archive.',
    ),
  ).toBeVisible({ timeout: 300_000 })
  await expect(
    screen.getByRole('button', { name: 'Restart PathKeep' }),
  ).toBeVisible()
  expect(readControl().archiveMode).toMatchObject({
    mode: 'Encrypted',
    compatibility: { revision: 1 },
  })
  for (const file of CONVERSATION_FILES)
    expect(existsSync(path.join(dataRoot(), file)), file).toBe(true)
  expect(existsSync(path.join(pkg, 'agent-history'))).toBe(false)

  // The restart button restarts the desktop itself, which the E2E launcher
  // can't follow; the launcher restarts it instead.
  await killAndRestartApp()
  await loadFresh(page, '/#/settings/storage')
  await enterApp(
    page,
    MOVE_FILE_PASSWORD,
    page.getByRole('heading', { name: 'Storage' }),
  )
  await expect(accessScreen(page)).toHaveCount(0)
  expect(await openArchiveVisitTotal()).toBe(visits)
  const back = await wireCounts()
  expect(back.savedRoots).toBe(counts.savedRoots)
  expect(back.spaceRoots).toBe(counts.spaceRoots)
  expect((await wireList()).map((row) => row.id).sort()).toEqual(visibleIds)

  // The copies that went with them are still listed, where they are now.
  const inPackage = (await safetyCopies()).filter((copy) =>
    copy.path.startsWith(path.join(pkg, 'copies') + path.sep),
  )
  expect(inPackage.length).toBeGreaterThan(0)
  const restorable = inPackage.find((copy) => copy.verifiedOpenable)
  if (restorable) {
    await storageRow(page, 'Restore from a safety copy')
      .getByRole('button', { name: 'Choose…' })
      .click()
    const restoring = page.getByRole('dialog')
    await expect(restoring).toContainText(`Kept in ${restorable.path}`)
    await page.keyboard.press('Escape')
  }
})
