/**
 * A move import keeps this computer's Agent conversations and adds the
 * file's (user decision 2026-11-15), and the copy made before it can take
 * back what it added.
 *
 * Proves:
 * - Storage has no FAT32/exFAT warning for a data folder that isn't on one
 *   (no E2E machine has one on FAT);
 * - with Pi's conversations backed up here, the import check says, before
 *   anything changes, which PathKeep made the file (this version: its own
 *   number, not "0.5 or earlier"), that the conversations here stay and the
 *   file's are added, and how to take them back;
 * - after the import, browsing history is the file's, every Pi
 *   conversation backed up here is still there, and the import recorded
 *   what it added: the archive holds exactly the ones it had plus those;
 * - Storage lists the import with how many it added;
 * - the restore dialog offers to take them back only for the copy made
 *   before that import, not for the copy an import that added nothing made;
 * - restoring that copy with the box ticked brings browsing history back to
 *   what it was, keeps the archive's password, moves exactly what the import
 *   added to the trash (nothing is deleted) and leaves the Pi conversations
 *   visible; a copy of the taken-back conversations is kept as a folder of
 *   its own, listed (Agent conversations only) and shown in Free up space;
 *   Storage then lists the import as taken back, with no Undo.
 *
 * And first: with `raw-snapshots/` linked to another disk, the check refuses
 * the file in words that say what to do, and nothing changes (only where a
 * folder on another volume exists, such as Linux's /dev/shm).
 *
 * Runs after upgrade-v05.spec, on the archive PathKeep 0.5.0's move file
 * left (no Agent conversations, App Lock passcode 4826), and opens the file
 * move.spec made. The archive's password is the file's afterwards; the
 * specs after this one rely on that.
 */
import {
  existsSync,
  mkdtempSync,
  renameSync,
  rmSync,
  statSync,
  symlinkSync,
  unlinkSync,
} from 'node:fs'
import path from 'node:path'
import { expect, test } from '@playwright/test'
import {
  agentKindState,
  chooseAndStart,
  count,
  heldBy,
} from './support/agent-backup'
import { wireCounts, wireList } from './support/agent-chats'
import { enableDialog } from './support/agent-tools'
import {
  conversationImports,
  dataRoot,
  enterApp,
  MOVE_FILE_PASSWORD,
  moveFile,
  opensArchive,
  safetyCopies,
  storageRow,
  v05Expected,
} from './support/archive-recovery'
import {
  backend,
  expectCount,
  fixture,
  openArchiveVisitTotal,
} from './support/fixture'

test.describe.configure({ mode: 'serial' })

/**
 * A new folder on another volume than the data folder, or null where none
 * can be made.
 */
function folderOnOtherVolume() {
  const here = statSync(dataRoot()).dev
  for (const candidate of ['/dev/shm', '/var/tmp', '/tmp']) {
    try {
      if (statSync(candidate).dev === here) continue
      return mkdtempSync(path.join(candidate, 'pathkeep-e2e-raw-'))
    } catch {
      // Not on this system, or not writable.
    }
  }
  return null
}

test('a raw-snapshots folder on another disk is refused at the check, and nothing changes', async ({
  page,
}) => {
  const elsewhere = folderOnOtherVolume()
  test.skip(elsewhere === null, 'no folder on another volume on this machine')
  test.setTimeout(300_000)
  const expected = v05Expected()
  await page.goto('/#/settings/storage')
  await enterApp(
    page,
    expected.exportPassword,
    page.getByRole('heading', { name: 'Storage' }),
  )
  const visits = await openArchiveVisitTotal()
  const copies = (await safetyCopies()).map((copy) => copy.path).sort()

  const raw = path.join(dataRoot(), 'raw-snapshots')
  const away = `${raw}.e2e-away`
  renameSync(raw, away)
  symlinkSync(elsewhere!, raw)
  try {
    await page.getByRole('button', { name: 'Import…' }).click()
    const importing = page.getByRole('dialog')
    await importing.getByLabel('File').fill(moveFile())
    await importing.getByRole('button', { name: 'Check file' }).click()
    await expect(importing.getByRole('alert')).toHaveText(
      'The raw-snapshots folder inside the data folder is a link to another disk. PathKeep keeps safety copies there by moving files, which can’t cross disks, so it stopped before changing anything. Put raw-snapshots back inside the data folder, or on the same disk, then try again.',
      { timeout: 60_000 },
    )
    await importing.getByRole('button', { name: 'Cancel' }).click()
  } finally {
    unlinkSync(raw)
    renameSync(away, raw)
    rmSync(elsewhere!, { recursive: true, force: true })
  }
  expect(await openArchiveVisitTotal()).toBe(visits)
  expect((await safetyCopies()).map((copy) => copy.path).sort()).toEqual(copies)
})

test('an import keeps the conversations here, and the copy before it takes back what it added', async ({
  page,
}, testInfo) => {
  test.setTimeout(900_000)
  const expected = v05Expected()
  expect(existsSync(moveFile()), 'move.spec left its file').toBe(true)

  // Where upgrade-v05.spec left it: PathKeep 0.5.0's data, no conversations.
  await page.goto('/#/settings/storage')
  await enterApp(
    page,
    expected.exportPassword,
    page.getByRole('heading', { name: 'Storage' }),
  )
  expect(await openArchiveVisitTotal()).toBe(expected.totalVisits)
  expect(count((await agentKindState())?.archiveCount)).toBe(0)
  // The data folder isn't on FAT here, so Storage has no FAT warning. (No
  // E2E machine has the data folder on FAT; this is all a spec can show.)
  expect(
    await backend<{ fat: boolean }>('get_storage_volume', { folder: null }),
  ).toMatchObject({ fat: false })
  await expect(page.locator('[data-fat-notice]')).toHaveCount(0)

  // Pi's conversations, backed up here.
  // Opened the way openEnableDialog does, through the archive's lock too.
  await page.goto('/#/browsing/home?enable=agent')
  await enterApp(page, expected.exportPassword, enableDialog(page))
  await chooseAndStart(page, ['pi'], false)
  await expect(
    enableDialog(page).getByRole('heading', { name: 'Saved' }),
  ).toBeVisible({ timeout: 180_000 })
  await page.keyboard.press('Escape')
  const local = heldBy(fixture().agents, ['pi']).conversations
  expect(local).toBeGreaterThan(0)
  await expect.poll(async () => (await wireCounts()).savedRoots).toBe(local)
  const localIds = (await wireList()).map((row) => row.id).sort()
  expect(localIds).toHaveLength(local)

  // The check says what happens to them before anything changes.
  await page.goto('/#/settings/storage')
  await enterApp(
    page,
    expected.exportPassword,
    page.getByRole('heading', { name: 'Storage' }),
  )
  await page.getByRole('button', { name: 'Import…' }).click()
  const importing = page.getByRole('dialog')
  await importing.getByLabel('File').fill(moveFile())
  await importing.getByRole('button', { name: 'Check file' }).click()
  await expect(importing).toContainText(/PathKeep version\s*\d+\.\d+\.\d+/, {
    timeout: 60_000,
  })
  await expect(importing).not.toContainText('PathKeep 0.5 or earlier')
  await expect(importing).toContainText(
    'The Agent conversations on this computer stay. The file’s are added beside them, and a conversation both have is kept once. Only browsing history is replaced.',
  )
  await expect(importing).toContainText(
    'If it turns out to be the wrong file, Conversations added by imports in Storage moves what it added to the trash.',
  )
  await expect(importing).toContainText(
    'This replaces the browsing history on this computer with the file’s.',
  )
  await testInfo.attach('the import check', {
    body: await importing.screenshot(),
    contentType: 'image/png',
  })
  expect((await wireCounts()).savedRoots, 'nothing changed yet').toBe(local)
  await importing.getByLabel('Password of this file').fill(MOVE_FILE_PASSWORD)
  await importing.getByRole('button', { name: 'Replace my archive' }).click()
  await expect(page.getByText('Import finished.')).toBeVisible({
    timeout: 600_000,
  })
  await enterApp(
    page,
    MOVE_FILE_PASSWORD,
    page.getByRole('heading', { name: 'Storage' }),
  )

  // Browsing is the file's; the conversations here stayed and the file's
  // were added, each once.
  const movedVisits = await openArchiveVisitTotal()
  expect(movedVisits).not.toBe(expected.totalVisits)
  const imports = await conversationImports()
  expect(imports).toHaveLength(1)
  const added = imports[0]
  expect(added.added).toBeGreaterThan(0)
  expect(added.eligible).toBe(added.added)
  expect(added.previousCopy).toBeTruthy()
  await expectCount(
    testInfo,
    'conversations after the import',
    (await wireCounts()).savedRoots,
    local + added.added,
  )
  const visible = new Set((await wireList()).map((row) => row.id))
  for (const id of localIds) expect(visible.has(id), id).toBe(true)

  // Storage lists the import.
  await page.reload()
  await enterApp(
    page,
    MOVE_FILE_PASSWORD,
    page.getByRole('heading', { name: 'Storage' }),
  )
  await storageRow(page, 'Conversations added by imports')
    .getByRole('button', { name: 'Review…' })
    .click()
  const list = page.getByRole('dialog')
  await expect(
    list.getByRole('list', { name: 'Conversations added by imports' }),
  ).toContainText(
    `Added ${added.added} conversation${added.added === 1 ? '' : 's'}`,
  )
  await expect(list.getByRole('button', { name: 'Undo…' })).toBeVisible()
  await list.getByRole('button', { name: 'Close' }).first().click()
  await expect(list).toHaveCount(0)

  // The restore offers to take them back with the copy made before this
  // import only: upgrade-v05.spec's import added none.
  const copies = await safetyCopies()
  const before = copies.find(
    (copy) => path.basename(copy.path) === added.previousCopy,
  )
  const earlier = copies.find(
    (copy) =>
      copy.sourceOp === 'import' &&
      copy.verifiedOpenable &&
      path.basename(copy.path) !== added.previousCopy,
  )
  expect(before?.sourceOp).toBe('import')
  expect(earlier, 'upgrade-v05.spec’s import left its copy').toBeTruthy()
  await storageRow(page, 'Restore from a safety copy')
    .getByRole('button', { name: 'Choose…' })
    .click()
  const restoring = page.getByRole('dialog')
  const undoBox = restoring.getByRole('checkbox', {
    name: /^Also move the \d+ conversations? this import added to the trash$/,
  })
  await restoring.locator(`[id="restore-${earlier!.id}"]`).click()
  await expect(undoBox).toHaveCount(0)
  await restoring.locator(`[id="restore-${before!.id}"]`).click()
  await expect(undoBox).toBeVisible()
  await expect(undoBox).toHaveAccessibleName(
    `Also move the ${added.eligible} conversation${added.eligible === 1 ? '' : 's'} this import added to the trash`,
  )
  await undoBox.check()
  // The copy holds the archive as it was before the import, under the
  // password it had then.
  await restoring
    .getByLabel('Password of this copy')
    .fill(expected.exportPassword)
  await testInfo.attach('the restore with the undo ticked', {
    body: await restoring.screenshot(),
    contentType: 'image/png',
  })
  await restoring.getByRole('button', { name: 'Restore', exact: true }).click()
  await expect(page.getByText('Browsing history restored.')).toBeVisible({
    timeout: 600_000,
  })
  await enterApp(
    page,
    MOVE_FILE_PASSWORD,
    page.getByRole('heading', { name: 'Storage' }),
  )

  // Browsing is back, under the archive's own password.
  await expect
    .poll(openArchiveVisitTotal, { timeout: 60_000 })
    .toBe(expected.totalVisits)
  expect(await opensArchive(MOVE_FILE_PASSWORD)).toBe(true)

  // What the import added is in the trash; nothing was deleted; Pi's stay.
  const after = await conversationImports()
  expect(after).toEqual([
    expect.objectContaining({ id: added.id, eligible: 0 }),
  ])
  const counts = await wireCounts()
  expect(counts.savedRoots).toBe(local + added.added)
  expect(counts.spaceRoots).toBe(local)
  const stillVisible = (await wireList()).map((row) => row.id).sort()
  expect(stillVisible).toEqual(localIds)
  const trashed = await wireList({ flags: ['trash'] })
  expect(trashed.length).toBeGreaterThanOrEqual(added.eligible)
  for (const row of trashed) expect(localIds).not.toContain(row.id)

  // The copy of them: its own folder, Agent conversations only.
  const undone = (await safetyCopies()).filter(
    (copy) => copy.sourceOp === 'importUndo',
  )
  expect(undone).toHaveLength(1)
  expect(undone[0].dataKinds).toEqual(['agent-history'])
  expect(undone[0].verifiedOpenable).toBe(false)
  expect(existsSync(path.join(undone[0].path, 'agent-history-format'))).toBe(
    true,
  )

  // Free up space lists it on its own, with the way to it.
  await page.reload()
  await enterApp(
    page,
    MOVE_FILE_PASSWORD,
    page.getByRole('heading', { name: 'Storage' }),
  )
  await storageRow(page, 'Free up space')
    .getByRole('button', { name: 'Review…' })
    .click()
  const freeing = page.getByRole('dialog')
  const row = freeing
    .getByRole('listitem')
    .filter({ hasText: 'Conversations taken back from an import' })
  await expect(row).toHaveCount(1, { timeout: 60_000 })
  await expect(row).toContainText(
    'Agent conversations an undone import added, kept before they went to the trash',
  )
  await expect(row.getByRole('button', { name: /^Show in/ })).toBeVisible()
  await freeing.getByRole('button', { name: 'Cancel' }).click()

  // Storage lists the import as taken back, with nothing left to undo.
  await storageRow(page, 'Conversations added by imports')
    .getByRole('button', { name: 'Review…' })
    .click()
  const taken = page.getByRole('dialog')
  await expect(taken).toContainText('Already taken back')
  await expect(taken.getByRole('button', { name: 'Undo…' })).toHaveCount(0)
  await testInfo.attach('the import, taken back', {
    body: await taken.screenshot(),
    contentType: 'image/png',
  })
  await page.keyboard.press('Escape')
})
