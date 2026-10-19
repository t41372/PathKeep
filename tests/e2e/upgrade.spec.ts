/**
 * Updating from PathKeep 0.3.0, on data made by v0.3.0's own app
 * (tests/fixtures/v0.3-archive): its data folder, and its "move to another
 * computer" file.
 *
 * Proves, for the data folder: the update screen appears instead of
 * onboarding (after v0.3.0's App Lock passcode); a wrong password is refused
 * and creates nothing; with the right one every kind of data arrives with the
 * numbers v0.3.0 reported: every visit and browser profile, both spellings of
 * one address with their own visit counts, a page with a Chinese address, the
 * hand-imported batch, visit durations in milliseconds (v0.3.0 kept Chrome's
 * microseconds), the page and domain stars, the note and tags, the saved page
 * text (its fetch record unchanged, its text in the file the record points
 * to), the chat.
 * The old files are left exactly as they were.
 *
 * For the v0.3.0 file: the preview says it was made by an older PathKeep and
 * that its chats stayed behind; it opens with the archive password and
 * replaces the archive with the same visits, stars and note.
 *
 * Runs after wipe.spec, when the data folder is empty and nothing is open, so
 * the running app finds the old folder on its next launch check, as an
 * updated app would.
 */
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { existsSync, readFileSync } from 'node:fs'
import path from 'node:path'
import { expect, test, type Page } from '@playwright/test'
import { backend, fixture } from './support/fixture'

const FIXTURE = path.resolve('tests/fixtures/v0.3-archive')

interface Expected {
  password: string
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
  importBatch: { sourceKind: string; importedItems: number; status: string }
  chat: { title: string; answer: string }
}

const expected = JSON.parse(
  readFileSync(path.join(FIXTURE, 'expected.json'), 'utf8'),
) as Expected

const root = () => fixture().dirs.projectRoot
const oldFiles = [
  'archive/history-vault.sqlite',
  'archive/source-evidence.sqlite',
  'derived/history-intelligence.sqlite',
  'derived/agent.sqlite',
  expected.savedPageText.blob,
]
const hashes = () =>
  oldFiles.map((file) =>
    createHash('sha256')
      .update(readFileSync(path.join(root(), file)))
      .digest('hex'),
  )

const results = (page: Page) =>
  page.getByRole('listbox', { name: 'History results' }).getByRole('option')

async function unlockAppIfLocked(page: Page) {
  const lock = page.getByRole('heading', { name: 'PathKeep is locked' })
  if (await lock.isVisible().catch(() => false)) {
    await page.getByPlaceholder('Passcode').fill(expected.appLockPasscode)
    await page.getByPlaceholder('Passcode').press('Enter')
    await expect(lock).toBeHidden()
  }
}

interface HistoryPage {
  items: {
    id: number
    url: string
    title?: string | null
    durationMs?: number | null
  }[]
}

test('a data folder made by PathKeep 0.3.0 is updated with all its data', async ({
  page,
}) => {
  test.setTimeout(900_000)
  expect(existsSync(path.join(root(), 'config.json'))).toBe(false)
  execFileSync('tar', ['-xzf', path.join(FIXTURE, 'data.tar.gz'), '-C', root()])
  const before = hashes()

  await page.goto('/')
  const updating = page.getByRole('heading', { name: 'Updating your archive' })
  // v0.3.0 had an App Lock passcode; the lock screen may come first.
  await expect(
    updating.or(page.getByRole('heading', { name: 'PathKeep is locked' })),
  ).toBeVisible()
  await unlockAppIfLocked(page)
  await expect(updating).toBeVisible()
  const password = page.getByLabel('Archive password')
  const start = page.getByRole('button', { name: 'Update archive' })

  await password.fill('not the password')
  await start.click()
  await expect(page.getByRole('alert')).toHaveText(
    'That password doesn’t open this archive.',
  )
  expect(existsSync(path.join(root(), 'archive/history-vault.db'))).toBe(false)

  await password.fill(expected.password)
  await start.click()
  await expect(
    page.getByRole('navigation', { name: 'Main navigation' }),
  ).toBeVisible({ timeout: 600_000 })
  expect(existsSync(path.join(root(), 'archive/history-vault.db'))).toBe(true)

  const dashboard = await backend<{
    totalVisits: number
    totalProfiles: number
  }>('load_dashboard_snapshot')
  expect(dashboard.totalVisits).toBe(expected.totalVisits)
  expect(dashboard.totalProfiles).toBe(expected.totalProfiles)

  // Each address keeps its own visits, the two spellings of one included.
  // A row's visit count is its own element ("2 visits"); the row's text runs
  // the time straight into it, so match the element, not the row's text.
  const visits = (url: string) =>
    page.getByText(new RegExp(`^${expected.pages[url]} visits?$`))
  await page.goto('/#/history')
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
  await expect(results(page)).toHaveCount(1)
  await expect(
    results(page).filter({ has: visits('https://zh.example.net/文章/潮汐') }),
  ).toHaveCount(1)
  await search.fill('Tide tables')
  await expect(
    results(page)
      .filter({ hasText: 'Tide tables' })
      .filter({ has: visits('https://tides.example.net/coast') }),
  ).toHaveCount(1)

  // Saved page text: v0.3.0's fetch record, and the words of the page.
  const saved = (
    await backend<HistoryPage>('query_history', {
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
  // The text itself stays in the file the record points to. (Search indexes a
  // page's summary, not its whole text, so a word from the body isn't found.)
  expect(
    readFileSync(path.join(root(), expected.savedPageText.blob), 'utf8'),
  ).toMatch(new RegExp(`^${expected.savedPageText.textStartsWith}`))

  // v0.3.0 kept Chrome's visit durations in microseconds; each of these visits
  // lasted 20 seconds, and the update stores that in milliseconds.
  const latest = await backend<HistoryPage>('query_history', {
    query: { limit: 200 },
  })
  expect(latest.items).toHaveLength(200)
  expect(new Set(latest.items.map((item) => item.durationMs))).toEqual(
    new Set([20_000]),
  )

  // Tags and the hand-imported batch.
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
  ).toEqual([expected.importBatch])

  // Stars and the note, on the page they were saved for.
  const { items: stars } = await backend<{
    items: { entityKind: string; entityKey: string }[]
  }>('list_stars', {})
  expect(stars.map((star) => star.entityKind).sort()).toEqual(
    expected.stars.map((star) => star.entityKind).sort(),
  )
  expect(stars.find((star) => star.entityKind === 'domain')?.entityKey).toBe(
    expected.stars.find((star) => star.entityKind === 'domain')!.entityKey,
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

  expect(hashes(), 'the old files must stay as they were').toEqual(before)
})

test('a file exported by PathKeep 0.3.0 opens and replaces the archive', async ({
  page,
}) => {
  test.setTimeout(900_000)
  await page.goto('/#/settings/storage')
  await unlockAppIfLocked(page)
  await page.getByRole('button', { name: 'Import…' }).click()
  const importing = page.getByRole('dialog')
  await importing.getByLabel('File').fill(path.join(FIXTURE, 'bundle.zip'))
  await importing.getByRole('button', { name: 'Check file' }).click()
  await expect(importing).toContainText(
    'It was made by an older PathKeep and will be updated as it opens.',
  )
  await expect(importing).toContainText(
    'Files from PathKeep 0.3 don’t include Ask chats; they stay on the other computer.',
  )
  const replace = importing.getByRole('button', { name: 'Replace my archive' })
  await importing.getByLabel('Password of this file').fill('not the password')
  await replace.click()
  await expect(
    importing.getByText('That password doesn’t open this file.'),
  ).toBeVisible({ timeout: 60_000 })

  await importing.getByLabel('Password of this file').fill(expected.password)
  await replace.click()
  await expect(page.getByText('Import finished.')).toBeVisible({
    timeout: 600_000,
  })
  await unlockAppIfLocked(page)
  await expect
    .poll(
      async () =>
        (await backend<{ totalVisits: number }>('load_dashboard_snapshot'))
          .totalVisits,
      { timeout: 60_000 },
    )
    .toBe(expected.totalVisits)
  const { items: stars } = await backend<{ items: { entityKind: string }[] }>(
    'list_stars',
    {},
  )
  expect(stars).toHaveLength(expected.stars.length)
  const noted = expected.annotations.find((note) => note.notes)!
  const note = await backend<{ notes: string } | null>('get_url_annotation', {
    url: noted.url,
  })
  expect(note?.notes).toBe(noted.notes)
})
