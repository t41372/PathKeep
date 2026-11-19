/**
 * Two states that aren't damage of the whole conversation store, last in the
 * repair project.
 *
 * Access refused for now: the file that holds the saved conversations can't
 * be read, because its permissions are taken away while PathKeep runs (as a
 * sync tool or another account's cleanup can do). That is not damage (`access-denied`): nothing
 * offers a repair, and "Try again" brings the conversations back once the
 * file can be read (`agent-history.md` §6.1).
 *
 * The app finds it the way it would on its own: the next operation that
 * checks every file of the archive (here a move to another computer) finds
 * the conversation file unreadable, refuses, and tells the window.
 *
 * Fixture state: runs after repair-3 (App Lock on, passcode 4826; the archive
 * encrypted with move.spec's password, remembered by the keychain; saved
 * conversations readable). Ends the same way; the file's permissions are put
 * back whatever happens.
 *
 * Skipped where the account running the tests reads a file whose
 * permissions forbid it (root): access can't be refused there.
 *
 * Proves:
 * - a move is refused with nothing written, saying it is not damage and
 *   nothing changed, with Try again and no repair, and names the file it
 *   can't read on a line of its own, with a way to show it; the backend
 *   reports `access-denied`; Try again while still refused says so;
 * - the Agent backup page then says PathKeep isn't allowed to read the
 *   files right now and that nothing is damaged, with Try again but no
 *   repair; browsing history is all there;
 * - once the permissions are back, Try again in the move dialog says the
 *   conversations can be read again (and no longer names a file), and the
 *   backup page shows every conversation it showed before, with no notice.
 *
 * One conversation that can't be read while the rest open (runs second, in
 * the state the first leaves; it sits here, last in the chain, because it
 * leaves the archive unencrypted and one conversation unreadable if it
 * fails). What PathKeep shows one conversation from is damaged in the file
 * directly (`damageOneConversationLists`), which needs the archive
 * unencrypted: encryption is turned off through the backend first and on
 * again at the end, password and keychain as before (setting up, not what
 * is tested). Proves:
 * - reopened, the store works (no damage notice) and the backend says some
 *   conversations wait for a rebuild; the Agent backup page and Manage data
 *   types say calmly that some conversations can't be read until their
 *   lists are rebuilt and nothing is lost, and offer the repair;
 * - the repair says the same, offers only the rebuild with nothing to
 *   choose, counts exactly the one conversation it reopens, and names the
 *   copy that will hold what it replaces;
 * - afterwards the result says that one opens again, the notice is gone
 *   from both screens and the backend says nothing waits; browsing history
 *   is all there.
 * No command reads one conversation yet, so "the rest still open" shows
 * only as the store staying available.
 */
import {
  chmodSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs'
import { join } from 'node:path'
import { expect, test } from '@playwright/test'
import { agentOverview, conversationsText, count } from './support/agent-backup'
import {
  ARCHIVE_PASSWORD,
  agentKind,
  conversationFile,
  damageNotice,
  damageOneConversationLists,
  evidencePath,
  expectAffectedFile,
  openAgentBackup,
  openStorage,
  opensArchive,
  parkedNotice,
  rekey,
  repairDialog,
  repairPlan,
  startExport,
  withArchiveStopped,
} from './support/conversation-damage'
import { backend, fixture, openArchiveVisitTotal } from './support/fixture'

/** Whether this account is kept out of a file whose permissions forbid reading it. */
function permissionsHold(dir: string) {
  const probe = join(dir, 'permission-probe')
  writeFileSync(probe, 'probe')
  chmodSync(probe, 0o000)
  try {
    readFileSync(probe)
    return false
  } catch {
    return true
  } finally {
    chmodSync(probe, 0o600)
    rmSync(probe, { force: true })
  }
}

test('conversations that can’t be read for now come back with Try again, not a repair', async ({
  page,
}, testInfo) => {
  test.setTimeout(600_000)
  const dir = join(fixture().root, 'repair-access')
  mkdirSync(dir, { recursive: true })
  test.skip(
    !permissionsHold(dir),
    'This account reads files whose permissions forbid it (root), so access can’t be refused here.',
  )

  await openAgentBackup(page)
  expect((await agentKind()).availability).toBe('available')
  const before = count((await agentOverview()).roots)
  expect(before, 'conversations saved by the agent specs').toBeGreaterThan(0)
  const visits = await openArchiveVisitTotal()
  const evidence = conversationFile.evidence()
  const mode = statSync(evidence).mode & 0o777
  const file = join(dir, 'refused.pathkeep')

  try {
    chmodSync(evidence, 0o000)

    // A move checks every file: refused, not damage, nothing written.
    await openStorage(page)
    const exporting = await startExport(page, file)
    const offer = exporting.locator('[data-access-offer="export"]')
    await expect(offer).toContainText(
      'PathKeep isn’t allowed to read the saved Agent conversations right now, so they can’t be packed into the file. Nothing is damaged, and nothing was changed.',
      { timeout: 60_000 },
    )
    await expect(offer).toContainText(
      'When their files can be read again, for example once their permissions are fixed, choose Try again.',
    )
    await expect(exporting.locator('[data-repair-offer]')).toHaveCount(0)
    await expectAffectedFile(offer, 'evidence', evidencePath)
    expect(existsSync(file)).toBe(false)
    await expect
      .poll(async () => (await agentKind()).failure?.reason)
      .toBe('access-denied')
    await testInfo.attach('the move refused', {
      body: await page.screenshot(),
      contentType: 'image/png',
    })

    // Still refused: Try again says so.
    await offer.getByRole('button', { name: 'Try again' }).click()
    await expect(offer).toContainText(
      'They still can’t be read. Check the files’ permissions, or whether another program holds them, then choose Try again.',
      { timeout: 60_000 },
    )
    await exporting.getByRole('button', { name: 'Cancel' }).click()
    await expect(exporting).toHaveCount(0)

    // The backup page says the same, with Try again and no repair.
    await page.goto('/#/agent/backup')
    const notice = damageNotice(page)
    await expect(notice).toContainText(
      'PathKeep isn’t allowed to read their files right now. Nothing is damaged: once the files can be read again, for example after their permissions are fixed, choose Try again.',
    )
    await expect(notice).toContainText('Browsing history is not affected.')
    await expect(notice).not.toContainText('damaged or can’t be read')
    await expect(
      notice.getByRole('button', { name: 'Try again' }),
    ).toBeEnabled()
    await expect(
      notice.getByRole('button', { name: 'Repair conversations…' }),
    ).toHaveCount(0)
    expect(await openArchiveVisitTotal()).toBe(visits)
    await testInfo.attach('the notice', {
      body: await page.screenshot({ fullPage: true }),
      contentType: 'image/png',
    })

    // Access back: Try again in the move dialog opens them, right there.
    chmodSync(evidence, mode)
    await openStorage(page)
    const again = await startExport(page, file)
    const offerAgain = again.locator('[data-access-offer="export"]')
    await expect(offerAgain).toBeVisible({ timeout: 60_000 })
    await offerAgain.getByRole('button', { name: 'Try again' }).click()
    await expect(offerAgain).toContainText(
      'The saved conversations can be read again. You can start this again now.',
      { timeout: 60_000 },
    )
    await expect(offerAgain.locator('[data-affected-file]')).toHaveCount(0)
    expect((await agentKind()).availability).toBe('available')
    await again.getByRole('button', { name: 'Cancel' }).click()
    await expect(again).toHaveCount(0)
    expect(existsSync(file)).toBe(false)
  } finally {
    chmodSync(evidence, mode)
  }

  // Every conversation is where it was.
  await page.goto('/#/agent/backup')
  await expect(damageNotice(page)).toHaveCount(0)
  await expect(
    page.getByText(new RegExp(`· ${conversationsText(before)}\\.`)),
  ).toBeVisible()
  expect(count((await agentOverview()).roots)).toBe(before)
  expect(await openArchiveVisitTotal()).toBe(visits)
  rmSync(dir, { recursive: true, force: true })
})

test('one conversation whose lists can’t be read is rebuilt while the rest stay open', async ({
  page,
}, testInfo) => {
  // Encryption off and on again, a restart and a rebuild, in a debug build.
  test.setTimeout(900_000)
  await openAgentBackup(page)
  expect(await agentKind()).toMatchObject({
    availability: 'available',
    readerRebuildRequired: false,
  })
  const visits = await openArchiveVisitTotal()

  await rekey({
    newMode: 'Plaintext',
    newKey: null,
    currentKey: ARCHIVE_PASSWORD,
  })
  await withArchiveStopped(page, () => {
    damageOneConversationLists()
  })

  // Reopened: the store works; only that conversation waits for a rebuild.
  await openAgentBackup(page)
  await expect
    .poll(async () => (await agentKind()).readerRebuildRequired, {
      timeout: 60_000,
    })
    .toBe(true)
  expect((await agentKind()).availability).toBe('available')
  expect(await openArchiveVisitTotal()).toBe(visits)
  const calm =
    'Their lists need to be rebuilt before they open again. Nothing is lost: the saved conversations themselves are intact, and the others open as usual.'
  const notice = parkedNotice(page)
  await expect(notice).toContainText(
    'Some saved conversations can’t be read right now',
  )
  await expect(notice).toContainText(calm)
  await expect(damageNotice(page)).toHaveCount(0)
  // Manage data types says the same, with the same way out.
  await page.goto('/#/settings/kinds')
  await expect(
    parkedNotice(page).getByRole('button', { name: 'Repair conversations…' }),
  ).toBeVisible()
  await expect(damageNotice(page)).toHaveCount(0)
  await page.goto('/#/agent/backup')
  await testInfo.attach('the calm notice', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })

  await notice.getByRole('button', { name: 'Repair conversations…' }).click()
  const dialog = repairDialog(page)
  await expect(dialog).toBeVisible()
  await expect(dialog).toContainText(
    'Some saved conversations can’t be read until their lists are rebuilt. Nothing is lost: the saved conversations themselves are intact, and the others open as usual.',
  )
  const plan = repairPlan(page)
  await expect(plan).toHaveAttribute('data-repair-action', 'rebuild', {
    timeout: 180_000,
  })
  await expect(
    plan.getByRole('heading', {
      name: 'Rebuild the lists from the saved conversations',
    }),
  ).toBeVisible()
  await expect(plan).toContainText(
    'The saved conversations themselves are intact; the lists of some of them can’t be read.',
  )
  await expect(
    plan.getByText('1 conversation that can’t be read now opens again.', {
      exact: true,
    }),
  ).toBeVisible()
  await expect(dialog.getByRole('radiogroup')).toHaveCount(0)
  const keptAs = await plan.locator('[data-kept-as]').innerText()
  expect(keptAs).toContain('before-agent-repair-')
  await testInfo.attach('the rebuild plan', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  await dialog.getByRole('button', { name: 'Rebuild the lists' }).click()
  await expect(
    dialog.getByRole('heading', { name: 'Saved conversations open again' }),
  ).toBeVisible({ timeout: 300_000 })
  await expect(
    dialog.getByText('1 conversation that couldn’t be read opens again.', {
      exact: true,
    }),
  ).toBeVisible()
  await expect(dialog.locator('[data-kept-copy]')).toHaveText(keptAs)
  await testInfo.attach('the result', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
  await dialog.getByRole('button', { name: 'Done' }).click()
  await expect(dialog).toHaveCount(0)

  // Nothing waits any more, on either screen.
  await expect(parkedNotice(page)).toHaveCount(0, { timeout: 60_000 })
  expect(await agentKind()).toMatchObject({
    availability: 'available',
    readerRebuildRequired: false,
  })
  await page.goto('/#/settings/kinds')
  await expect(page.locator('[data-kind="agent-history"]')).toBeVisible()
  await expect(parkedNotice(page)).toHaveCount(0)
  expect(await openArchiveVisitTotal()).toBe(visits)

  // As the specs after this one expect: move.spec's password, in the keychain.
  await rekey({ newMode: 'Encrypted', newKey: ARCHIVE_PASSWORD })
  expect(await opensArchive(ARCHIVE_PASSWORD)).toBe(true)
  expect(
    (await backend<{ storedSecret: boolean }>('keyring_status')).storedSecret,
  ).toBe(true)
})
