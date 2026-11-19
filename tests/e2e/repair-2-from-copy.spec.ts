/**
 * The file that holds the saved conversations themselves (the evidence) is
 * damaged as a whole while PathKeep is stopped. A safety copy the person made
 * earlier, under the archive's previous password, brings them back
 * (`agent-history.md` › 從安全副本修復對話).
 *
 * Fixture state: runs after repair-1 (App Lock on, passcode 4826; the archive
 * encrypted with move.spec's password, remembered by the keychain; saved
 * conversations readable). Ends the same way: password, keychain and
 * conversations as they were.
 *
 * Proves:
 * - Settings → Storage makes a safety copy on request: the check says where it
 *   goes and what it holds, and the copy is listed as made by you;
 * - while conversations can't be read, a safety copy can still be made; the
 *   dialog says beforehand that it can't repair conversations, and the copy
 *   is listed with that mark;
 * - moving the archive to another computer is refused with nothing written,
 *   and the refusal leads to the repair;
 * - checking the damaged files offers to start over, says how many
 *   conversations that would lose, and lists the copies: the one made during
 *   the damage can't be chosen and says why;
 * - the earlier copy asks for its own password, says when it's wrong, and
 *   then holds exactly the conversations starting over would lose;
 * - after the repair the conversations are back, the archive's current
 *   password still opens it and the copy's no longer does, the result says
 *   to choose folders again, and a move is no longer refused;
 * - Restore from a safety copy marks the copy made during the damage and
 *   says what it can and can't do.
 */
import { existsSync, mkdirSync, rmSync } from 'node:fs'
import { join } from 'node:path'
import { expect, test, type Page } from '@playwright/test'
import { agentOverview, conversationsText, count } from './support/agent-backup'
import { unlockIfLocked } from './support/app'
import {
  ARCHIVE_PASSWORD,
  agentKind,
  conversationFile,
  openAgentBackup,
  opensArchive,
  overwriteWhole,
  recoveryCopies,
  rekey,
  repairDialog,
  repairPlan,
  withArchiveStopped,
} from './support/conversation-damage'
import { fixture, openArchiveVisitTotal, parseCount } from './support/fixture'

/** The password the archive has between the first copy and the end of the spec. */
const LATER_PASSWORD = 'harbour lights at dusk'
const FILE_PASSWORD = 'a file for the other computer'

async function openStorage(page: Page) {
  await page.goto('/#/settings/storage')
  await unlockIfLocked(page)
  await expect(page.getByRole('button', { name: 'Make a copy…' })).toBeVisible()
}

/** Makes a safety copy through Settings → Storage and returns the new copy's path. */
async function makeSafetyCopy(page: Page, unreadable: boolean) {
  const before = new Set((await recoveryCopies()).map((copy) => copy.path))
  await page.getByRole('button', { name: 'Make a copy…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Make a safety copy' })
  const plan = dialog.locator('[data-safety-copy-plan]')
  await expect(plan).toContainText(
    'It holds everything in the archive as it is now: browsing history and any saved Agent conversations.',
  )
  const warning = dialog.getByText(
    'Saved Agent conversations can’t be read right now. The copy keeps their files exactly as they are and can restore browsing history, but it can’t be used to repair conversations.',
  )
  await expect(warning).toHaveCount(unreadable ? 1 : 0)
  await dialog.getByRole('button', { name: 'Make the copy' }).click()
  await expect(page.getByText('Safety copy made.')).toBeVisible({
    timeout: 120_000,
  })
  await expect(dialog).toHaveCount(0)
  const made = (await recoveryCopies()).filter((copy) => !before.has(copy.path))
  expect(made).toHaveLength(1)
  expect(made[0]).toMatchObject({
    sourceOp: 'manual',
    conversationsUnreadable: unreadable,
    verifiedOpenable: true,
  })
  return made[0].path
}

/** Starts a move to another computer through Settings → Storage. */
async function startExport(page: Page, file: string) {
  await page
    .getByText('Move to another computer', { exact: true })
    .locator('xpath=../..')
    .getByRole('button', { name: 'Export…' })
    .click()
  const dialog = page.getByRole('dialog')
  await dialog.getByLabel('Save to').fill(file)
  await dialog.getByLabel('Password for this file').fill(FILE_PASSWORD)
  await dialog.getByLabel('Type it again').fill(FILE_PASSWORD)
  await dialog.getByRole('button', { name: 'Export', exact: true }).click()
  return dialog
}

test('conversations whose file is damaged come back from a safety copy under an older password', async ({
  page,
}, testInfo) => {
  // Two safety copies, two password changes, a restart, a repair and an
  // export of the whole archive, in a debug build.
  test.setTimeout(900_000)
  const dir = join(fixture().root, 'repair')
  mkdirSync(dir, { recursive: true })

  await openAgentBackup(page)
  expect((await agentKind()).availability).toBe('available')
  const before = count((await agentOverview()).roots)
  expect(before, 'conversations saved by the agent specs').toBeGreaterThan(0)
  const visits = await openArchiveVisitTotal()

  // A copy while everything reads, then a new password: the copy keeps the old one.
  await openStorage(page)
  const good = await makeSafetyCopy(page, false)
  await rekey({
    newMode: 'Encrypted',
    newKey: LATER_PASSWORD,
    currentKey: ARCHIVE_PASSWORD,
  })
  expect(await opensArchive(LATER_PASSWORD)).toBe(true)

  await withArchiveStopped(page, () =>
    overwriteWhole(conversationFile.evidence(), 'damaged conversations'),
  )

  // Browsing reopens; the conversations don't.
  await openStorage(page)
  expect(await openArchiveVisitTotal()).toBe(visits)
  const damaged = await agentKind()
  expect(damaged.availability).toBe('unavailable')
  expect(damaged.failure?.role).toBe('evidence')

  // A safety copy still works, and says beforehand what it can't do.
  const marked = await makeSafetyCopy(page, true)

  // A move is refused before anything is written, and leads to the repair.
  const file = join(dir, 'refused.pathkeep')
  const exporting = await startExport(page, file)
  const offer = exporting.locator('[data-repair-offer="export"]')
  await expect(offer).toContainText(
    'Saved Agent conversations can’t be read right now, so they can’t be packed into the file. Repair them first, then export again. Nothing was changed.',
    { timeout: 60_000 },
  )
  expect(existsSync(file)).toBe(false)
  await testInfo.attach('the move refused', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
  await offer.getByRole('button', { name: 'Repair conversations…' }).click()
  await expect(exporting.getByLabel('Save to')).toHaveCount(0)
  const dialog = repairDialog(page)
  await expect(dialog).toBeVisible()
  expect(page.url()).toContain('#/settings/storage')

  // The damaged files alone: start over, and lose what only they held.
  const plan = repairPlan(page)
  await expect(plan).toHaveAttribute('data-repair-action', 'startFresh', {
    timeout: 180_000,
  })
  await expect(
    plan.getByRole('heading', {
      name: 'Start over with what still checks out',
    }),
  ).toBeVisible()
  const lost = parseCount(
    await plan.getByText(/can’t be brought back\./).innerText(),
  )
  expect(lost).toBeGreaterThan(0)
  await expect(
    plan.getByText(
      'Conversations the tools have already deleted come back only if they checked out above.',
    ),
  ).toBeVisible()
  await expect(
    plan.getByText(
      'Conversations aren’t backed up again until you choose the folders to back up on the Agent backup page.',
    ),
  ).toBeVisible()
  await expect(dialog.getByRole('button', { name: 'Start over' })).toBeEnabled()

  // The copies: the one made during the damage is listed but can't be chosen.
  const sources = dialog.getByRole('radiogroup', {
    name: 'Bring conversations back from',
  })
  await expect(
    sources.getByRole('radio', { name: /^The damaged files/ }),
  ).toBeChecked()
  const markedRow = sources.locator(`[data-copy-path="${marked}"]`)
  await expect(markedRow.getByRole('radio')).toBeDisabled()
  await expect(markedRow).toContainText(
    'Conversations couldn’t be read when this copy was made, so it can’t repair them.',
  )
  const goodRow = sources.locator(`[data-copy-path="${good}"]`)
  await expect(goodRow).toContainText('Made by you')
  await testInfo.attach('start over, and the copies', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  // The earlier copy asks for the password it was made under.
  await goodRow.click()
  await expect(plan).toHaveCount(0)
  const check = dialog.getByRole('button', { name: 'Check this copy' })
  await check.click()
  await expect(
    dialog.getByText(
      'This copy was made under another password. Enter it to check the copy.',
    ),
  ).toBeVisible({ timeout: 180_000 })
  const password = dialog.getByLabel('Password of this copy')
  await expect(check).toBeDisabled()
  await password.fill(LATER_PASSWORD)
  await check.click()
  await expect(
    dialog.getByText('That isn’t this copy’s password.'),
  ).toBeVisible({ timeout: 180_000 })
  await password.fill(ARCHIVE_PASSWORD)
  await check.click()
  await expect(plan).toHaveAttribute('data-repair-action', 'fromCopy', {
    timeout: 180_000,
  })
  await expect(
    plan.getByRole('heading', {
      name: 'Bring conversations back from a safety copy',
    }),
  ).toBeVisible()
  await expect(plan).toContainText('Made by you · ')
  // What starting over would lose is what the copy holds.
  const held = parseCount(await plan.getByText(/^The copy holds /).innerText())
  expect(held, 'the copy holds what starting over would lose').toBe(lost)
  const keptAs = await plan.locator('[data-kept-as]').innerText()
  await testInfo.attach('the copy plan', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  await dialog.getByRole('button', { name: 'Repair from this copy' }).click()
  await expect(
    dialog.getByRole('heading', { name: 'Saved conversations open again' }),
  ).toBeVisible({ timeout: 300_000 })
  await expect(
    dialog.getByText(
      `${held.toLocaleString('en-US')} conversation${held === 1 ? '' : 's'} can be read.`,
    ),
  ).toBeVisible()
  await expect(
    dialog.getByText(
      'Before the next backup, choose the folders to back up again.',
    ),
  ).toBeVisible()
  await expect(dialog.locator('[data-kept-copy]')).toHaveText(keptAs)
  await testInfo.attach('the result', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
  await dialog.getByRole('button', { name: 'Open Agent backup' }).click()
  await expect(dialog).toHaveCount(0)
  await expect(page).toHaveURL(/#\/agent\/backup$/)
  await expect(
    page.getByText(new RegExp(`· ${conversationsText(before)}\\.`)),
  ).toBeVisible()

  // Back, under the current password; the copy's password stays the copy's.
  expect((await agentKind()).availability).toBe('available')
  expect(count((await agentOverview()).roots)).toBe(before)
  expect(await openArchiveVisitTotal()).toBe(visits)
  expect(await opensArchive(LATER_PASSWORD)).toBe(true)
  expect(await opensArchive(ARCHIVE_PASSWORD)).toBe(false)
  const kept = (await recoveryCopies()).find((copy) => copy.path === keptAs)
  expect(kept).toMatchObject({
    sourceOp: 'agentRepair',
    conversationsUnreadable: true,
  })

  // Restore lists the copy made during the damage with its mark and what it means.
  await openStorage(page)
  await page
    .getByText('Restore from a safety copy', { exact: true })
    .locator('xpath=../..')
    .getByRole('button', { name: 'Choose…' })
    .click()
  const restore = page.getByRole('dialog', {
    name: 'Restore from a safety copy',
  })
  const markedChoice = restore.getByRole('radio', {
    name: /^Made by you.*Conversations couldn’t be read/,
  })
  await expect(markedChoice).toHaveCount(1)
  await markedChoice.click()
  await expect(
    restore.getByText(
      'Saved Agent conversations couldn’t be read when this copy was made. It restores browsing history like any other copy, but it can’t be used to repair conversations.',
    ),
  ).toBeVisible()
  await restore.getByRole('button', { name: 'Cancel' }).click()
  await expect(restore).toHaveCount(0)

  // Repaired, the archive moves again.
  const moved = join(dir, 'after-repair.pathkeep')
  const again = await startExport(page, moved)
  await expect(page.getByText(/^Saved \d/)).toBeVisible({ timeout: 300_000 })
  await expect(again).toHaveCount(0)
  expect(existsSync(moved)).toBe(true)
  rmSync(dir, { recursive: true, force: true })

  // As the specs after this one expect: move.spec's password, in the keychain.
  await rekey({
    newMode: 'Encrypted',
    newKey: ARCHIVE_PASSWORD,
    currentKey: LATER_PASSWORD,
  })
  expect(await opensArchive(ARCHIVE_PASSWORD)).toBe(true)
})
