/**
 * Updating from PathKeep 0.3: the data folder an old install leaves behind
 * (an encrypted SQLCipher archive, saved page text, a starred page with a
 * note, an Ask chat) opens on the update screen, which copies it into the new
 * format with the archive password.
 *
 * Proves: the update screen appears instead of onboarding; a wrong password
 * is refused and creates nothing; with the right one every kind of data
 * arrives (both spellings of one address, the star and note saved under the old
 * spelling's key, saved page text found by search, the chat), and the old
 * files are left exactly as they were.
 *
 * Runs after wipe.spec, when the data folder is empty and nothing is open, so
 * the running app finds the old folder on its next launch check, as an
 * updated app would. The folder is written by the converter's own fixture
 * code (`examples/v0-3-archive.rs`), from the SQL the old releases shipped.
 */
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { existsSync, readFileSync } from 'node:fs'
import path from 'node:path'
import { expect, test, type Page } from '@playwright/test'
import { fixture } from './support/fixture'

const PASSWORD = 'tide pool 0.3'

interface Written {
  pages: string[]
  starred: string
  note: string
  savedText: { query: string; page: string }
  chat: { title: string; answer: string }
}

const root = () => fixture().dirs.projectRoot
const oldFiles = [
  'archive/history-vault.sqlite',
  'derived/history-intelligence.sqlite',
  'derived/agent.sqlite',
]
const hashes = () =>
  oldFiles.map((file) =>
    createHash('sha256')
      .update(readFileSync(path.join(root(), file)))
      .digest('hex'),
  )

const results = (page: Page) =>
  page.getByRole('listbox', { name: 'History results' }).getByRole('option')

test('an archive left by PathKeep 0.3 is updated with all its data', async ({
  page,
}, testInfo) => {
  test.setTimeout(900_000)
  expect(existsSync(path.join(root(), 'config.json'))).toBe(false)
  const written = JSON.parse(
    execFileSync(
      'cargo',
      [
        'run',
        '--quiet',
        '--manifest-path',
        'src-tauri/Cargo.toml',
        '-p',
        'vault-convert-sqlcipher',
        '--features',
        'test-fixtures',
        '--example',
        'v0-3-archive',
        '--',
        root(),
        PASSWORD,
      ],
      { encoding: 'utf8', timeout: 600_000 },
    ),
  ) as Written
  await testInfo.attach('old data folder', {
    body: JSON.stringify(written, null, 2),
    contentType: 'application/json',
  })
  const before = hashes()

  await page.goto('/')
  await expect(
    page.getByRole('heading', { name: 'Updating your archive' }),
  ).toBeVisible()
  const password = page.getByLabel('Archive password')
  const start = page.getByRole('button', { name: 'Update archive' })

  await password.fill('not the password')
  await start.click()
  await expect(page.getByRole('alert')).toHaveText(
    'That password doesn’t open this archive.',
  )
  expect(existsSync(path.join(root(), 'archive/history-vault.db'))).toBe(false)

  await password.fill(PASSWORD)
  await start.click()
  await expect(
    page.getByRole('navigation', { name: 'Main navigation' }),
  ).toBeVisible({ timeout: 300_000 })
  expect(existsSync(path.join(root(), 'archive/history-vault.db'))).toBe(true)

  await page.goto('/#/history')
  const search = page.getByRole('searchbox', { name: 'Search history' })
  await search.fill('quokka')
  // Both spellings of the address stay as visited, one visit each.
  const quokka = results(page).filter({ hasText: written.pages[0] })
  await expect(quokka).toHaveCount(2)
  await expect(quokka.first()).toContainText('1 visit')

  await search.fill(written.savedText.query)
  await expect(results(page)).toHaveCount(1)
  await expect(results(page)).toContainText(written.savedText.page)

  await search.fill('')
  await page.getByRole('tab', { name: /^Starred/ }).click()
  const main = page.getByRole('main')
  await expect(main).toContainText(written.starred)
  await expect(main).toContainText(written.note)

  await page.goto('/#/ask')
  await page
    .getByRole('complementary', { name: 'Conversations' })
    .getByRole('button', { name: written.chat.title })
    .click()
  await expect(page.getByRole('main')).toContainText(written.chat.answer)

  expect(hashes(), 'the old files must stay as they were').toEqual(before)
})
