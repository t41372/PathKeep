/**
 * When PathKeep won't open the archive in its data folder: the screen says
 * why and offers the action that fixes it. Nothing on it deletes data except
 * finishing the deletion PathKeep 0.5 started, after the word is typed.
 *
 * Proves:
 * - an archive a newer PathKeep stamped as unreadable for this version
 *   gets its own words, the format this version reads against the one the
 *   archive needs, and the updater; no repair or restore is offered; the
 *   desktop answers only recovery commands; neither the screen, checking
 *   again, nor another start writes, renames, adds or removes a file of the
 *   archive. One a newer version may let this one read
 *   but not change says that instead. With the settings file put back the
 *   archive opens as before;
 * - an archive whose settings file is gone says so and offers to rebuild it
 *   and to restore from a safety copy (listing the copies); rebuilding asks
 *   for the archive's current password, refuses a wrong one without writing
 *   anything, and with the right one writes a settings file in this
 *   version's form, keeping App Lock; after a restart the archive opens
 *   with every visit and every conversation;
 * - a PathKeep 0.5 archive (made by that release) whose Ask chats file is
 *   gone says which file, offers to start Ask chats empty, does it with the
 *   archive's password, and after a restart the archive is brought up to
 *   this version with every visit and no Ask chats.
 *
 * - a “Delete all data” PathKeep 0.5 left unfinished (its record, no
 *   settings file) gets its own words and the advice to copy the folder,
 *   and nothing else; the start deletes nothing, and neither does opening
 *   the confirmation and cancelling it, nor a word other than DELETE; with
 *   DELETE typed it finishes, the record and the archive's files are gone,
 *   and PathKeep is at setup.
 *
 * Runs last, after rollback-v05.spec (encrypted with move.spec's file
 * password, App Lock 4826). The third test deletes all data the way a user
 * does and unpacks PathKeep 0.5.0's data folder; the fourth turns that into
 * the folder PathKeep 0.5's own unfinished deletion leaves, and finishes it,
 * so the run ends at setup.
 */
import { execFileSync } from 'node:child_process'
import {
  existsSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from 'node:fs'
import path from 'node:path'
import { expect, test } from '@playwright/test'
import { agentKindState, count } from './support/agent-backup'
import { killAndRestartApp } from './support/app'
import {
  archiveAccess,
  controlPath,
  dataRoot,
  enterApp,
  folderFingerprint,
  loadFresh,
  MOVE_FILE_PASSWORD,
  openAccessScreen,
  readControl,
  V05_FIXTURE,
  v05Expected,
} from './support/archive-recovery'
import { backend, fixture, openArchiveVisitTotal } from './support/fixture'

test.describe.configure({ mode: 'serial' })

interface Stamped {
  archiveMode: {
    compatibility: { revision: number; minReader: number; minWriter: number }
  }
}

/** The settings file with its stamp raised as a newer PathKeep would. */
function stampedBy(original: Buffer, minReader: number, minWriter: number) {
  const control = JSON.parse(original.toString('utf8')) as Stamped
  control.archiveMode.compatibility = {
    revision: 99,
    minReader,
    minWriter,
  }
  return JSON.stringify(control, null, 2)
}

test('an archive a newer PathKeep changed says so, offers the update and touches nothing', async ({
  page,
}, testInfo) => {
  test.setTimeout(600_000)
  await page.goto('/#/settings/storage')
  await enterApp(
    page,
    MOVE_FILE_PASSWORD,
    page.getByRole('heading', { name: 'Storage' }),
  )
  const visits = await openArchiveVisitTotal()
  const original = readFileSync(controlPath())
  expect(typeof readControl().archiveMode).toBe('object')

  try {
    // Unreadable by this version.
    const newer = stampedBy(original, 99, 99)
    writeFileSync(controlPath(), newer)
    await killAndRestartApp()
    let screen = await openAccessScreen(page)
    // The start refused before writing anything; what the screen and the
    // next start do is compared with this.
    expect(readFileSync(controlPath(), 'utf8')).toBe(newer)
    const before = folderFingerprint()
    await expect(
      screen.getByRole('heading', {
        name: 'This archive was updated by a newer PathKeep',
      }),
    ).toBeVisible()
    await expect(screen).toContainText(
      /This PathKeep reads and writes archive format \d+\. This archive needs format 99\./,
    )
    await expect(
      screen.getByRole('button', { name: 'Check now' }),
    ).toBeVisible()
    await expect(
      screen.getByRole('heading', { name: 'Rebuild the settings file' }),
    ).toHaveCount(0)
    await expect(
      screen.getByRole('heading', { name: 'Restore from a safety copy' }),
    ).toHaveCount(0)
    await screen.locator('summary', { hasText: 'Details' }).click()
    await expect(screen.locator('[data-access-detail]')).toContainText(
      'archive-reader-too-old',
    )
    await testInfo.attach('a newer PathKeep’s archive', {
      body: await page.screenshot({ fullPage: true }),
      contentType: 'image/png',
    })
    expect(await archiveAccess()).toMatchObject({
      state: 'refusedRead',
      recoveryOnly: true,
      code: 'archive-reader-too-old',
      archive: { minReader: 99 },
    })
    await expect(backend('app_snapshot')).rejects.toThrow(
      /archive-recovery-only/,
    )
    // Looking again changes nothing either, nor does another start.
    await screen.getByRole('button', { name: 'Check again' }).click()
    await expect(
      screen.getByRole('heading', {
        name: 'This archive was updated by a newer PathKeep',
      }),
    ).toBeVisible({ timeout: 60_000 })
    await killAndRestartApp()
    screen = await openAccessScreen(page)
    expect(folderFingerprint(), 'nothing in the folder changed').toEqual(before)
    expect(readFileSync(controlPath(), 'utf8')).toBe(newer)

    // Readable, but only a newer PathKeep may change it.
    writeFileSync(controlPath(), stampedBy(original, 1, 99))
    await killAndRestartApp()
    screen = await openAccessScreen(page)
    await expect(
      screen.getByRole('heading', {
        name: 'This archive needs a newer PathKeep to change it',
      }),
    ).toBeVisible()
    await expect(
      screen.getByRole('button', { name: 'Check now' }),
    ).toBeVisible()
    expect(await archiveAccess()).toMatchObject({
      state: 'refusedWrite',
      code: 'archive-writer-too-old',
    })
  } finally {
    writeFileSync(controlPath(), original)
  }

  await killAndRestartApp()
  await loadFresh(page, '/#/settings/storage')
  await enterApp(
    page,
    MOVE_FILE_PASSWORD,
    page.getByRole('heading', { name: 'Storage' }),
  )
  expect(await openArchiveVisitTotal()).toBe(visits)
})

test('a missing settings file is rebuilt with the archive’s password, and the archive opens', async ({
  page,
}, testInfo) => {
  test.setTimeout(600_000)
  await page.goto('/#/settings/storage')
  await enterApp(
    page,
    MOVE_FILE_PASSWORD,
    page.getByRole('heading', { name: 'Storage' }),
  )
  const visits = await openArchiveVisitTotal()
  // Counted as Manage data types counts them (the trash included): a rebuilt
  // settings file puts which kinds PathKeep keeps back to the defaults, so
  // the conversations list itself may be off afterwards.
  const conversations = count((await agentKindState())?.archiveCount)
  expect(conversations).toBeGreaterThan(0)

  // Gone, the way a sync tool or a slip in the file manager loses it.
  renameSync(controlPath(), path.join(fixture().root, 'config.json.gone'))
  await killAndRestartApp()
  const screen = await openAccessScreen(page)
  await expect(
    screen.getByRole('heading', {
      name: 'The archive’s settings file is missing',
    }),
  ).toBeVisible()
  expect(await archiveAccess()).toMatchObject({
    state: 'needsRecovery',
    recoveryOnly: true,
    code: 'archive-control-missing',
  })
  // Both ways out: rebuild, or a safety copy.
  await expect(
    screen.getByRole('heading', { name: 'Restore from a safety copy' }),
  ).toBeVisible()
  await expect(
    screen
      .getByRole('radiogroup', { name: 'Restore from a safety copy' })
      .getByRole('radio')
      .first(),
  ).toBeVisible({ timeout: 60_000 })
  await testInfo.attach('the settings file is missing', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })

  // The archive is encrypted: its password is asked for, and checked.
  const rebuild = screen.getByRole('button', {
    name: 'Rebuild the settings file',
  })
  await rebuild.click()
  const password = screen.getByLabel('The archive’s current password')
  await expect(password).toBeVisible({ timeout: 60_000 })
  expect(existsSync(controlPath())).toBe(false)
  await password.fill('not the password')
  await rebuild.click()
  await expect(
    screen.getByText('That isn’t the archive’s current password.'),
  ).toBeVisible({ timeout: 60_000 })
  expect(existsSync(controlPath()), 'nothing written').toBe(false)

  await password.fill(MOVE_FILE_PASSWORD)
  await rebuild.click()
  await expect(
    screen.getByText(
      'The settings file is rebuilt. Restart PathKeep to open the archive.',
    ),
  ).toBeVisible({ timeout: 120_000 })
  await expect(
    screen.getByRole('button', { name: 'Restart PathKeep' }),
  ).toBeVisible()
  const rebuilt = readControl()
  expect(rebuilt.initialized).toBe(true)
  expect(rebuilt.archiveMode).toMatchObject({ mode: 'Encrypted' })

  // The restart button restarts the desktop itself, which the E2E launcher
  // can't follow; the launcher restarts it instead.
  await killAndRestartApp()
  await loadFresh(page, '/#/settings/storage')
  await enterApp(
    page,
    MOVE_FILE_PASSWORD,
    page.getByRole('heading', { name: 'Storage' }),
  )
  expect(await openArchiveVisitTotal()).toBe(visits)
  expect(count((await agentKindState())?.archiveCount)).toBe(conversations)
})

test('a PathKeep 0.5 archive without its Ask chats file starts them empty and opens', async ({
  page,
}, testInfo) => {
  test.setTimeout(900_000)
  const expected = v05Expected()
  await page.goto('/#/settings/storage')
  await enterApp(
    page,
    MOVE_FILE_PASSWORD,
    page.getByRole('heading', { name: 'Storage' }),
  )
  // Delete all data the way a user does, then PathKeep 0.5.0's folder.
  await page.getByRole('button', { name: 'Delete…' }).click()
  const wipe = page.getByRole('dialog', { name: 'Delete all data?' })
  await wipe.getByRole('textbox').fill('DELETE')
  await wipe.getByRole('button', { name: 'Delete forever' }).click()
  await expect(
    page.getByRole('heading', { name: 'Keep what gets deleted automatically' }),
  ).toBeVisible({ timeout: 60_000 })
  execFileSync('tar', [
    '-xzf',
    path.join(V05_FIXTURE, 'data.tar.gz'),
    '-C',
    dataRoot(),
  ])
  const askFile = path.join(dataRoot(), 'derived', 'agent.db')
  expect(existsSync(askFile)).toBe(true)
  for (const companion of ['', '-wal', '-shm', '-journal'])
    rmSync(askFile + companion, { force: true })

  await killAndRestartApp()
  const screen = await openAccessScreen(page)
  await expect(
    screen.getByRole('heading', { name: 'A file of the archive is missing' }),
  ).toBeVisible()
  await expect(screen).toContainText(
    'The file that holds your Ask chats is missing, so this version won’t open the archive.',
  )
  const access = await archiveAccess()
  expect(access).toMatchObject({
    state: 'needsRecovery',
    code: 'archive-store-missing',
  })
  expect(access.detail?.replaceAll('\\', '/')).toMatch(/derived\/agent\.db$/)
  await testInfo.attach('the Ask chats file is missing', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })

  const make = screen.getByRole('button', { name: 'Make it empty' })
  await make.click()
  const password = screen.getByLabel('The archive’s current password')
  await expect(password).toBeVisible({ timeout: 60_000 })
  expect(existsSync(askFile)).toBe(false)
  await password.fill(expected.password)
  await make.click()
  await expect(
    screen.getByText('Done. Restart PathKeep to open the archive.'),
  ).toBeVisible({ timeout: 120_000 })
  expect(existsSync(askFile)).toBe(true)
  // Still PathKeep 0.5.0's form: bringing it up is the ordinary start's job.
  expect(readControl().archiveMode).toBe('Encrypted')

  await killAndRestartApp()
  await loadFresh(page, '/')
  await enterApp(page, expected.password)
  expect(await openArchiveVisitTotal()).toBe(expected.totalVisits)
  // The Ask chat PathKeep 0.5.0 had was in the file that went missing.
  const asked = await backend<{ conversations: unknown[] }>(
    'list_ai_conversations',
    { request: { limit: 20 } },
  )
  expect(asked.conversations).toEqual([])
  expect(readControl().archiveMode).toMatchObject({
    compatibility: { revision: 1 },
  })
})

test('a “Delete all data” PathKeep 0.5 left unfinished is finished only after the word is typed', async ({
  page,
}, testInfo) => {
  test.setTimeout(600_000)
  const expected = v05Expected()
  await page.goto('/')
  await enterApp(page, expected.password)
  const store = path.join(dataRoot(), 'archive', 'history-vault.db')
  expect(existsSync(store)).toBe(true)

  // What PathKeep 0.5's "Delete all data" leaves when it stops partway: its
  // record, no settings file (it deletes that first), and the rest.
  const marker = path.join(dataRoot(), '.pk-wipe-in-progress.json')
  rmSync(controlPath())
  writeFileSync(marker, JSON.stringify({ providerIds: [], schedule: null }))
  await killAndRestartApp()
  const screen = await openAccessScreen(page)
  await expect(
    screen.getByRole('heading', {
      name: 'A “Delete all data” from PathKeep 0.5 didn’t finish',
    }),
  ).toBeVisible()
  await expect(screen).toContainText(
    'Copy the data folder somewhere safe before you do anything else.',
  )
  expect(await archiveAccess()).toMatchObject({
    state: 'needsRecovery',
    code: 'archive-wipe-confirmation-required',
  })
  // Nothing else is offered for it, and the start deleted nothing.
  await expect(
    screen.getByRole('heading', { name: 'Restore from a safety copy' }),
  ).toHaveCount(0)
  const before = folderFingerprint()
  expect(existsSync(marker)).toBe(true)
  expect(existsSync(store)).toBe(true)
  await testInfo.attach('PathKeep 0.5’s unfinished deletion', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })

  // Asked again, with the folder and the word; anything else isn't enough.
  await screen.getByRole('button', { name: 'Finish deleting…' }).click()
  const confirm = page.getByRole('dialog', {
    name: 'Finish deleting everything?',
  })
  await expect(
    confirm.locator('[data-path]').filter({ hasText: dataRoot() }),
  ).toHaveCount(1)
  const deleteForever = confirm.getByRole('button', { name: 'Delete forever' })
  const word = confirm.getByLabel('Type DELETE to confirm')
  await expect(deleteForever).toBeDisabled()
  await word.fill('delete')
  await expect(deleteForever).toBeDisabled()
  await testInfo.attach('finishing it, before the word', {
    body: await confirm.screenshot(),
    contentType: 'image/png',
  })
  await confirm.getByRole('button', { name: 'Cancel' }).click()
  await expect(confirm).toHaveCount(0)
  expect(folderFingerprint(), 'cancelling deleted nothing').toEqual(before)

  await screen.getByRole('button', { name: 'Finish deleting…' }).click()
  await confirm.getByLabel('Type DELETE to confirm').fill('DELETE')
  await confirm.getByRole('button', { name: 'Delete forever' }).click()

  // Setup again, as after "Delete all data". Where only the recovery
  // commands answered, the app restarts first (the launcher does it here).
  const welcome = page.getByRole('heading', {
    name: 'Keep what gets deleted automatically',
  })
  const restart = page.getByRole('button', { name: 'Restart PathKeep' })
  await expect(welcome.or(restart).first()).toBeVisible({ timeout: 120_000 })
  if (await restart.isVisible()) {
    await killAndRestartApp()
    await loadFresh(page, '/')
  }
  await expect(welcome).toBeVisible({ timeout: 60_000 })
  expect(existsSync(marker)).toBe(false)
  expect(existsSync(store)).toBe(false)
  expect(await archiveAccess()).toMatchObject({
    state: 'notInitialized',
    recoveryOnly: false,
  })
})
