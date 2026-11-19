/**
 * One page of the saved conversations' file is damaged: the page that holds
 * this computer's backup records. The file opens, and the damage shows only
 * when the backup page reads those records. Starting over brings every
 * conversation back from what still checks out (`agent-history.md` ›
 * 從安全副本修復對話).
 *
 * Fixture state: runs after repair-2 (App Lock on, passcode 4826; the archive
 * encrypted with move.spec's password, remembered by the keychain; saved
 * conversations readable). To find that page the spec reads the file's
 * table of contents itself, which needs the archive unencrypted: encryption
 * is turned off through the backend first (setting up, not what is tested)
 * and turned on again through Settings at the end, password and keychain as
 * before.
 *
 * Proves:
 * - the damage found while reading shows on the backup page, saying which
 *   file and when it was found;
 * - turning encryption on is refused while conversations can't be read, and
 *   the refusal leads to the repair;
 * - the repair offers to start over and says every conversation comes back,
 *   all of them complete (none without its latest messages), so it doesn't
 *   point to the safety copies; that the next backup saves the tools' files
 *   again; and asks to choose the folders again;
 * - afterwards the backup page shows the conversations it showed before, and
 *   encryption can be turned on again.
 */
import { expect, test } from '@playwright/test'
import {
  agentOverview,
  agentRuns,
  conversationsText,
  count,
} from './support/agent-backup'
import { unlockIfLocked } from './support/app'
import {
  ARCHIVE_PASSWORD,
  agentKind,
  conversationFile,
  damageNotice,
  openAgentBackup,
  opensArchive,
  overwritePage,
  rekey,
  repairDialog,
  repairPlan,
  rootPageOf,
  withArchiveStopped,
} from './support/conversation-damage'
import { backend, openArchiveVisitTotal, parseCount } from './support/fixture'

const archiveEncrypted = async () =>
  (await backend<{ archiveStatus: { encrypted: boolean } }>('app_snapshot'))
    .archiveStatus.encrypted

test('conversations whose backup records are damaged all come back when starting over', async ({
  page,
}, testInfo) => {
  // Encryption off and on again, a restart and a repair, in a debug build.
  test.setTimeout(900_000)
  await openAgentBackup(page)
  expect((await agentKind()).availability).toBe('available')
  const before = count((await agentOverview()).roots)
  expect(before, 'conversations saved by the agent specs').toBeGreaterThan(0)
  const visits = await openArchiveVisitTotal()

  await rekey({
    newMode: 'Plaintext',
    newKey: null,
    currentKey: ARCHIVE_PASSWORD,
  })
  await withArchiveStopped(page, () => {
    const evidence = conversationFile.evidence()
    overwritePage(evidence, rootPageOf(evidence, 'ah_desktop_backups'))
  })

  // The file opens; reading the backup records finds the damage. The page
  // lists them; the spec reads them too, so the read doesn't depend on
  // which query the page sends first.
  await openAgentBackup(page)
  await expect(agentRuns(1)).rejects.toThrow()
  await expect
    .poll(async () => (await agentKind()).failure?.stage, { timeout: 30_000 })
    .toBe('read')
  expect(await agentKind()).toMatchObject({
    availability: 'unavailable',
    failure: { role: 'evidence', stage: 'read' },
  })
  // The backend tells the window, which swaps the page for the notice.
  const notice = damageNotice(page)
  await expect(notice).toBeVisible({ timeout: 60_000 })
  await expect(notice).toContainText(
    'The file that holds the saved conversations is damaged or can’t be read. Found while reading saved conversations.',
  )
  expect(await openArchiveVisitTotal()).toBe(visits)

  // Turning encryption on rewrites every file: refused, and leads to the repair.
  await page.goto('/#/settings/security')
  await unlockIfLocked(page)
  await page.getByRole('switch', { name: 'Encrypt archive' }).click()
  const encrypting = page.getByRole('dialog', { name: 'Encrypt the archive' })
  await encrypting.getByLabel('New password').fill(ARCHIVE_PASSWORD)
  await encrypting.getByLabel('Type it again').fill(ARCHIVE_PASSWORD)
  await encrypting.getByRole('button', { name: 'Continue' }).click()
  const offer = encrypting.locator('[data-repair-offer="encryption"]')
  const encrypt = encrypting.getByRole('button', {
    name: 'Encrypt',
    exact: true,
  })
  // The check may already refuse; otherwise running it does.
  await expect(offer.or(encrypt)).toBeVisible({ timeout: 60_000 })
  if (await encrypt.isVisible()) await encrypt.click()
  await expect(offer).toContainText(
    'Turning encryption on or off rewrites every file of the archive, and the saved Agent conversations can’t be read right now. Repair them first, then try again. Nothing was changed.',
    { timeout: 120_000 },
  )
  expect(await archiveEncrypted(), 'nothing was changed').toBe(false)
  await testInfo.attach('encryption refused', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
  await offer.getByRole('button', { name: 'Repair conversations…' }).click()
  await expect(encrypting).toHaveCount(0)
  const dialog = repairDialog(page)
  await expect(dialog).toBeVisible()
  expect(page.url()).toContain('#/settings/security')
  await expect(dialog).toContainText('Found while reading saved conversations.')

  const plan = repairPlan(page)
  await expect(plan).toHaveAttribute('data-repair-action', 'startFresh', {
    timeout: 180_000,
  })
  const back = parseCount(
    await plan.getByText(/^\d[\d,]* conversations? comes? back\.$/).innerText(),
  )
  expect(back).toBeGreaterThan(0)
  // Every revision still verifies: all of them complete, none left behind,
  // so nothing points to the copies.
  await expect(
    plan.getByText(
      'All of them come back complete, with their latest messages.',
    ),
  ).toBeVisible()
  await expect(plan.getByText('Every conversation comes back.')).toBeVisible()
  await expect(plan.locator('[data-partial]')).toHaveCount(0)
  await expect(dialog.locator('[data-suggest-copies]')).toHaveCount(0)
  await expect(
    plan.getByText(
      'The next backup saves again every conversation still in the tools’ folders.',
    ),
  ).toBeVisible()
  await expect(
    plan.getByText(
      'Conversations the tools have already deleted come back only if they checked out above.',
    ),
  ).toHaveCount(0)
  await expect(
    plan.getByText(
      'Conversations aren’t backed up again until you choose the folders to back up on the Agent backup page.',
    ),
  ).toBeVisible()
  const keptAs = await plan.locator('[data-kept-as]').innerText()
  await testInfo.attach('the start-over plan', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  await dialog.getByRole('button', { name: 'Start over' }).click()
  await expect(
    dialog.getByRole('heading', { name: 'Saved conversations open again' }),
  ).toBeVisible({ timeout: 300_000 })
  await expect(
    dialog.getByText(
      `${back.toLocaleString('en-US')} conversation${back === 1 ? '' : 's'} can be read.`,
    ),
  ).toBeVisible()
  await expect(
    dialog.getByText(
      'The next backup saves again every conversation still in the tools’ folders.',
    ),
  ).toBeVisible()
  await expect(dialog.locator('[data-kept-copy]')).toHaveText(keptAs)
  await dialog.getByRole('button', { name: 'Done' }).click()
  await expect(dialog).toHaveCount(0)

  // Every conversation is back where it was.
  expect((await agentKind()).availability).toBe('available')
  expect(count((await agentOverview()).roots)).toBe(before)
  await page.goto('/#/agent/backup')
  await expect(damageNotice(page)).toHaveCount(0)
  await expect(
    page.getByText(new RegExp(`· ${conversationsText(before)}\\.`)),
  ).toBeVisible()
  await testInfo.attach('the backup page afterwards', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })

  // Encryption can be turned on again: move.spec's password, in the keychain.
  await page.goto('/#/settings/security')
  await page.getByRole('switch', { name: 'Encrypt archive' }).click()
  const again = page.getByRole('dialog', { name: 'Encrypt the archive' })
  await again.getByLabel('New password').fill(ARCHIVE_PASSWORD)
  await again.getByLabel('Type it again').fill(ARCHIVE_PASSWORD)
  await expect(
    again.getByRole('checkbox', { name: 'Save it in the system keychain' }),
  ).toBeChecked()
  await again.getByRole('button', { name: 'Continue' }).click()
  await again.getByRole('button', { name: 'Encrypt', exact: true }).click()
  await expect(page.getByText('The archive is encrypted.')).toBeVisible({
    timeout: 300_000,
  })
  expect(await archiveEncrypted()).toBe(true)
  expect(await opensArchive(ARCHIVE_PASSWORD)).toBe(true)
  expect(
    (await backend<{ storedSecret: boolean }>('keyring_status')).storedSecret,
  ).toBe(true)
  expect(await openArchiveVisitTotal()).toBe(visits)
})
