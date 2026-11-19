/**
 * Opening data made by PathKeep 0.5.0's own app (tests/fixtures/v0.5-archive):
 * its data folder, the safety copy it made, and its "move to another computer"
 * file.
 *
 * Proves, for the data folder: after v0.5.0's App Lock passcode the archive
 * asks for its password (this computer's keychain doesn't have it); a wrong
 * one is refused and the archive is not touched, still without the stores
 * this version adds; with the right one every kind of data arrives with the
 * numbers v0.5.0 reported (every visit and browser profile, both spellings of
 * one address with their own visit counts, a page with a Chinese address, the
 * hand-imported Chrome file and the Google Takeout import, the page and domain
 * stars, the note and tags, the saved page text, the chat); the archive is
 * stamped and has the two agent-history stores, with nothing left of the
 * bring-up; a second open writes nothing; and the health check finds every
 * database in the format this version writes.
 *
 * For v0.5.0's safety copy (from its switch to encryption, before the stars,
 * note and tags): it is listed, restores with the archive's password, and
 * takes the archive back to it. For v0.5.0's file: it opens with its own
 * password and replaces the archive with every visit, star, note and the chat.
 *
 * Runs after upgrade.spec, on the archive its v0.3.0 file left (App Lock
 * passcode 4826), and deletes all data first, as a user does in Settings, so
 * the running app finds the v0.5.0 folder on its next launch check.
 */
import { execFileSync } from 'node:child_process'
import { existsSync, readFileSync, readdirSync } from 'node:fs'
import path from 'node:path'
import { expect, test, type Page } from '@playwright/test'
import { backend, fixture, openArchiveVisitTotal } from './support/fixture'

const FIXTURE = path.resolve('tests/fixtures/v0.5-archive')

interface Expected {
  password: string
  exportPassword: string
  appLockPasscode: string
  totalVisits: number
  totalProfiles: number
  pages: Record<string, number>
  stars: { entityKind: string; entityKey: string; title: string }[]
  annotations: { url: string; notes: string; tags: string[] }[]
  savedPageText: {
    url: string
    blob: string
    textStartsWith: string
    record: Record<string, string>
  }
  importBatches: { sourceKind: string; importedItems: number; status: string }[]
  chat: { id: string; title: string; answer: string }
  safetyCopy: { name: string; sourceOp: string; encrypted: boolean }
}

interface Copy {
  path: string
  sourceOp: string
  verifiedOpenable: boolean
}

const expected = JSON.parse(
  readFileSync(path.join(FIXTURE, 'expected.json'), 'utf8'),
) as Expected
/** The password of the archive upgrade.spec's v0.3.0 file left behind. */
const V03_PASSWORD = (
  JSON.parse(
    readFileSync(
      path.resolve('tests/fixtures/v0.3-archive/expected.json'),
      'utf8',
    ),
  ) as { password: string }
).password

const root = () => fixture().dirs.projectRoot
const control = () =>
  JSON.parse(readFileSync(path.join(root(), 'config.json'), 'utf8')) as {
    archiveMode: unknown
  }
const ADDED_STORES = [
  'archive/agent-history.db',
  'derived/agent-history-read-model.db',
]

const results = (page: Page) =>
  page.getByRole('listbox', { name: 'History results' }).getByRole('option')

async function unlockAppLock(page: Page) {
  const lock = page.getByRole('heading', { name: 'PathKeep is locked' })
  if (await lock.isVisible().catch(() => false)) {
    await page.getByPlaceholder('Passcode').fill(expected.appLockPasscode)
    await page.getByPlaceholder('Passcode').press('Enter')
    await expect(lock).toBeHidden()
  }
}

async function unlockIfAsked(page: Page, password: string) {
  await unlockAppLock(page)
  const archive = page.getByRole('heading', {
    name: 'Your archive is encrypted',
  })
  if (await archive.isVisible().catch(() => false)) {
    await page.getByPlaceholder('Password').fill(password)
    await page.getByPlaceholder('Password').press('Enter')
    await expect(archive).toBeHidden()
  }
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

test('a data folder made by PathKeep 0.5.0 opens with all its data', async ({
  page,
}) => {
  test.setTimeout(900_000)
  // Delete all data, the way a user does, so the folder is empty and nothing
  // is open.
  await page.goto('/#/settings/storage')
  await unlockIfAsked(page, V03_PASSWORD)
  await page.getByRole('button', { name: 'Delete…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Delete all data?' })
  await dialog.getByRole('textbox').fill('DELETE')
  await dialog.getByRole('button', { name: 'Delete forever' }).click()
  await expect(
    page.getByRole('heading', {
      name: 'Your browser is deleting your history',
    }),
  ).toBeVisible({ timeout: 60_000 })
  expect(existsSync(path.join(root(), 'config.json'))).toBe(false)
  execFileSync('tar', ['-xzf', path.join(FIXTURE, 'data.tar.gz'), '-C', root()])

  await page.goto('/')
  const archiveLock = page.getByRole('heading', {
    name: 'Your archive is encrypted',
  })
  // v0.5.0's App Lock comes first, then its archive password: this
  // computer's keychain doesn't have it.
  await expect(
    archiveLock.or(page.getByRole('heading', { name: 'PathKeep is locked' })),
  ).toBeVisible()
  await unlockAppLock(page)
  await expect(archiveLock).toBeVisible()
  const password = page.getByPlaceholder('Password')
  await password.fill('not the password')
  await password.press('Enter')
  await expect(
    page.getByText('That did not work. Check it and try again.'),
  ).toBeVisible()
  // Refused before the archive was opened: still v0.5.0's control, and none
  // of the stores this version adds.
  expect(control().archiveMode).toBe('Encrypted')
  for (const store of ADDED_STORES)
    expect(existsSync(path.join(root(), store)), store).toBe(false)

  await password.fill(expected.password)
  await password.press('Enter')
  await expect(
    page.getByRole('navigation', { name: 'Main navigation' }),
  ).toBeVisible({ timeout: 300_000 })

  // Brought up once: stamped, the two stores in place, nothing of the
  // bring-up left behind.
  expect(control().archiveMode).toMatchObject({
    mode: 'Encrypted',
    compatibility: { revision: 1, minReader: 1, minWriter: 1 },
  })
  for (const store of ADDED_STORES)
    expect(existsSync(path.join(root(), store)), store).toBe(true)
  expect(existsSync(path.join(root(), 'archive/.pk-install-v2.json'))).toBe(
    false,
  )
  const staging = path.join(root(), 'staging')
  expect(
    existsSync(staging)
      ? readdirSync(staging).filter((name) => name.startsWith('creating-'))
      : [],
  ).toEqual([])

  const dashboard = await backend<{
    totalVisits: number
    totalProfiles: number
  }>('load_dashboard_snapshot')
  expect(dashboard.totalVisits).toBe(expected.totalVisits)
  expect(dashboard.totalProfiles).toBe(expected.totalProfiles)

  // Each address keeps its own visits, the two spellings of one included.
  const visits = (url: string) =>
    page.getByText(new RegExp(`^${expected.pages[url]} visits?$`))
  await page.goto('/#/browsing/history')
  const search = page.getByRole('searchbox', { name: 'Search history' })
  await search.fill('quokka')
  const quokka = results(page).filter({ hasText: 'Quokka field notes' })
  await expect(quokka).toHaveCount(2)
  for (const url of [
    'https://example.org/%7Equokka?utm_source=old',
    'https://example.org/~quokka',
  ])
    await expect(quokka.filter({ has: visits(url) })).toHaveCount(1)
  await search.fill('潮汐')
  await expect(
    results(page).filter({ has: visits('https://zh.example.net/文章/潮汐') }),
  ).toHaveCount(1)
  // The Takeout import's pages (titled by their number), each with its own
  // visits.
  await search.fill('Harbour almanac')
  const almanac = Object.keys(expected.pages).filter((url) =>
    url.startsWith('https://almanac.example.net/harbour/'),
  )
  expect(almanac.length).toBeGreaterThan(0)
  await expect(results(page)).toHaveCount(almanac.length)
  for (const url of almanac)
    await expect(
      results(page)
        .filter({ hasText: `Harbour almanac ${url.split('/').pop()}` })
        .filter({ has: visits(url) }),
    ).toHaveCount(1)

  // Saved page text: v0.5.0's fetch record, and the words of the page.
  const saved = (
    await backend<{ items: { id: number; url: string }[] }>('query_history', {
      query: { q: 'Example Domain', limit: 50 },
    })
  ).items.find((item) => item.url === expected.savedPageText.url)
  expect(saved).toBeTruthy()
  const records = await backend<Record<string, string>[]>(
    'list_visit_enrichment',
    { historyId: saved!.id },
  )
  expect(records).toHaveLength(1)
  for (const [field, value] of Object.entries(expected.savedPageText.record))
    expect(records[0][field], field).toBe(value)
  expect(
    readFileSync(path.join(root(), expected.savedPageText.blob), 'utf8'),
  ).toMatch(new RegExp(`^${expected.savedPageText.textStartsWith}`))

  // Tags, and both imports.
  const tagged = expected.annotations.find((note) => note.tags.length)!
  await search.fill(`tag:${tagged.tags[0]}`)
  await expect(results(page)).toHaveCount(1)
  await expect(results(page)).toContainText('Tide tables')
  const snapshot = await backend<{
    recentImportBatches: {
      sourceKind: string
      importedItems: number
      status: string
    }[]
  }>('app_snapshot')
  expect(
    snapshot.recentImportBatches.map(
      ({ sourceKind, importedItems, status }) => ({
        sourceKind,
        importedItems,
        status,
      }),
    ),
  ).toEqual(expected.importBatches)

  // Stars and the note, on the page they were saved for.
  const { items: stars } = await backend<{
    items: { entityKind: string; entityKey: string }[]
  }>('list_stars', {})
  expect(stars.map((star) => star.entityKind).sort()).toEqual(
    expected.stars.map((star) => star.entityKind).sort(),
  )
  const noted = expected.annotations.find((note) => note.notes)!
  await search.fill('')
  await page.getByRole('tab', { name: /^Starred/ }).click()
  const main = page.getByRole('main')
  await expect(main).toContainText(
    expected.stars.find((star) => star.entityKind === 'url')!.title,
  )
  await expect(main).toContainText(noted.notes)

  await page.goto('/#/ask')
  await page
    .getByRole('complementary', { name: 'Conversations' })
    .getByRole('button', { name: expected.chat.title })
    .click()
  await expect(page.getByRole('main')).toContainText(expected.chat.answer)

  // Once up, an open writes nothing: the same control, no record. v0.5.0 had
  // the archive's key remembered, so the password screen put it in this
  // computer's keychain, and a window loaded again, as after a restart, opens
  // the archive from it alone.
  const stamped = readFileSync(path.join(root(), 'config.json'))
  expect(await backend<boolean>('keyring_get_database_key')).toBe(true)
  await backend('clear_session_database_key')
  await page.goto('/#/browsing/history')
  await page.reload()
  await unlockAppLock(page)
  expect(await openArchiveVisitTotal()).toBe(expected.totalVisits)
  expect(readFileSync(path.join(root(), 'config.json'))).toEqual(stamped)
  expect(existsSync(path.join(root(), 'archive/.pk-install-v2.json'))).toBe(
    false,
  )

  await page.goto('/#/settings/about')
  await page.getByRole('button', { name: 'Check', exact: true }).click()
  await expect(
    page
      .getByRole('listitem')
      .filter({ hasText: 'Archive format' })
      .getByLabel('Passed'),
  ).toBeVisible({ timeout: 60_000 })
})

test('the safety copy PathKeep 0.5.0 made takes the archive back to it', async ({
  page,
}) => {
  test.setTimeout(600_000)
  const copies = await backend<Copy[]>('list_recovery_snapshots')
  // v0.5.0's copy from its switch to encryption is the only one.
  expect(copies).toHaveLength(1)
  expect(path.basename(copies[0].path)).toBe(expected.safetyCopy.name)
  expect(copies[0].sourceOp).toBe(expected.safetyCopy.sourceOp)
  expect(copies[0].verifiedOpenable).toBe(true)
  await page.goto('/#/settings/storage')
  await unlockIfAsked(page, expected.password)
  await page
    .getByText('Restore from a safety copy', { exact: true })
    .locator('xpath=../..')
    .getByRole('button', { name: 'Choose…' })
    .click()
  const dialog = page.getByRole('dialog')
  await dialog.getByRole('radio').first().click()
  await dialog.getByLabel('Password of this copy').fill(expected.password)
  await dialog.getByRole('button', { name: 'Restore', exact: true }).click()
  await expect(page.getByText('Browsing history restored.')).toBeVisible({
    timeout: 300_000,
  })
  await unlockIfAsked(page, expected.password)

  // Every visit was there before the switch; the stars, the note and the
  // tags came after it.
  await expect
    .poll(openArchiveVisitTotal, { timeout: 60_000 })
    .toBe(expected.totalVisits)
  const { items: stars } = await backend<{ items: unknown[] }>('list_stars', {})
  expect(stars).toEqual([])
  const noted = expected.annotations.find((note) => note.notes)!
  const note = await backend<{ notes: string } | null>('get_url_annotation', {
    url: noted.url,
  })
  expect(note?.notes ?? '').toBe('')
  // The archive keeps its own password.
  expect(await opensArchive(expected.password)).toBe(true)
})

test('a file exported by PathKeep 0.5.0 opens and replaces the archive', async ({
  page,
}) => {
  test.setTimeout(900_000)
  // v0.5.0's copy, and the one the restore above made. The file brings
  // v0.5.0's copy again: it was exported from this folder after the switch.
  const copiesBefore = await backend<Copy[]>('list_recovery_snapshots')
  expect(copiesBefore.map((copy) => copy.sourceOp).sort()).toEqual(
    [expected.safetyCopy.sourceOp, 'restore'].sort(),
  )
  await page.goto('/#/settings/storage')
  await unlockIfAsked(page, expected.password)
  await page.getByRole('button', { name: 'Import…' }).click()
  const importing = page.getByRole('dialog')
  await importing
    .getByLabel('File')
    .fill(path.join(FIXTURE, 'move-file.pathkeep'))
  await importing.getByRole('button', { name: 'Check file' }).click()
  // It says the copies come too. Format 2 needs no update note: v0.5.0 writes
  // the archive this version reads, without the two stores it adds.
  await expect(importing).toContainText(
    'Files from PathKeep 0.5 and earlier also hold the safety copies of the computer they were made on. Any in this file are added to the ones here.',
  )
  await expect(importing).not.toContainText('made by an older PathKeep')
  await expect(importing).toContainText(
    'The safety copies already on this computer stay as they are.',
  )
  const replace = importing.getByRole('button', { name: 'Replace my archive' })
  await importing.getByLabel('Password of this file').fill('not the password')
  await replace.click()
  await expect(
    importing.getByText('That password doesn’t open this file.'),
  ).toBeVisible({ timeout: 60_000 })

  await importing
    .getByLabel('Password of this file')
    .fill(expected.exportPassword)
  await replace.click()
  await expect(page.getByText('Import finished.')).toBeVisible({
    timeout: 600_000,
  })
  await unlockIfAsked(page, expected.exportPassword)
  await expect
    .poll(openArchiveVisitTotal, { timeout: 60_000 })
    .toBe(expected.totalVisits)
  const { items: stars } = await backend<{ items: unknown[] }>('list_stars', {})
  expect(stars).toHaveLength(expected.stars.length)
  const noted = expected.annotations.find((note) => note.notes)!
  const note = await backend<{ notes: string } | null>('get_url_annotation', {
    url: noted.url,
  })
  expect(note?.notes).toBe(noted.notes)
  const chats = await backend<{ id: string }[]>('list_ai_conversations', {
    request: { limit: 20 },
  })
  expect(JSON.stringify(chats)).toContain(expected.chat.id)
  for (const store of ADDED_STORES)
    expect(existsSync(path.join(root(), store)), store).toBe(true)
  // From now on the file's password is the archive's.
  expect(await opensArchive(expected.exportPassword)).toBe(true)

  // The copies here stay where they were, listed; v0.5.0's copy is not listed
  // twice; the archive the import replaced is one more, and holds no copy.
  const copiesAfter = await backend<Copy[]>('list_recovery_snapshots')
  const paths = copiesAfter.map((copy) => copy.path)
  for (const copy of copiesBefore) expect(paths).toContain(copy.path)
  const replaced = copiesAfter.filter((copy) => copy.sourceOp === 'import')
  expect(replaced).toHaveLength(1)
  expect(copiesAfter).toHaveLength(copiesBefore.length + 1)
  // It holds the browsers' history files that archive's backups copied, and
  // none of the copies.
  const inside = readdirSync(path.join(replaced[0].path, 'raw-snapshots'))
  expect(inside).toContain('chrome%3ADefault')
  expect(inside).not.toContain('maintenance')
})
