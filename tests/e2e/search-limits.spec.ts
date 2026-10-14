/**
 * History search reaches every match, and says so honestly when it can't yet.
 *
 * Proves:
 * - a word on every page lists every page: the exact count equals what the
 *   fixture holds, page for page and visit for visit;
 * - a regex finds every visit of a page, the oldest included;
 * - while the search index is still catching up, results say so: a note
 *   written while the indexer is held is not found yet, the screen says some
 *   recent changes aren't searchable yet (never just "nothing found"), and
 *   once the indexer runs again the page appears without searching again.
 *
 * The indexer is held through a debug-build bridge command
 * (`debug_set_search_indexer_paused`), so no other spec slows down.
 */
import { expect, test } from '@playwright/test'
import { archivedVisits, backend } from './support/fixture'

const count = (n: number) => n.toLocaleString('en-US')

test('a word on every page lists every page with an exact count', async ({
  page,
}) => {
  const visits = archivedVisits()
  expect(
    visits.every((visit) => visit.url.startsWith('https://')),
    'every fixture URL contains the word',
  ).toBe(true)
  const pages = new Set(visits.map((visit) => visit.url)).size

  await page.goto('/#/history')
  await page.getByRole('searchbox', { name: 'Search history' }).fill('https')

  await expect(
    page.getByText(
      `${count(pages)} pages · ${count(visits.length)} visits · Full text`,
      { exact: true },
    ),
  ).toBeVisible({ timeout: 30_000 })
  await expect(page.getByText(/^[\d,]+\+ pages/)).toHaveCount(0)
})

test('a regex finds every visit of a page, the oldest included', async ({
  page,
}) => {
  const url = 'https://news.ycombinator.com/item?id=41502281'
  const visits = archivedVisits().filter((visit) => visit.url === url)
  expect(visits.length, 'the page has several visits').toBeGreaterThan(1)

  await page.goto('/#/history')
  const search = page.getByRole('searchbox', { name: 'Search history' })
  // The mode switch shows once there is text to search.
  await search.fill('tokio')
  await page.getByRole('radio', { name: 'Regex' }).click()
  await search.fill('item\\?id=41502281$')

  await expect(
    page.getByText(`1 page · ${count(visits.length)} visits · Regex`, {
      exact: true,
    }),
  ).toBeVisible({ timeout: 30_000 })
  const rows = page
    .getByRole('listbox', { name: 'History results' })
    .getByRole('option')
    .filter({ hasText: 'Hacker News · Why I left tokio' })
  await expect(rows).toHaveCount(1)
  await expect(rows).toContainText(`${count(visits.length)} visits`)
})

test('results and the palette say so while the search index is catching up', async ({
  page,
}) => {
  const word = 'quokkacoverage'
  const target = archivedVisits().find((visit) =>
    visit.url.startsWith('https://news.ycombinator.com/'),
  )
  expect(target, 'a page to write a note on').toBeDefined()
  const { url, title } = target!

  await backend('debug_set_search_indexer_paused', { paused: true })
  try {
    await backend('set_url_notes', { request: { url, notes: word } })

    await page.goto('/#/history')
    await page.getByRole('searchbox', { name: 'Search history' }).fill(word)

    await expect(
      page.getByRole('status').filter({
        hasText:
          'Some recent changes aren’t searchable yet. They will be in a moment.',
      }),
    ).toBeVisible({ timeout: 30_000 })
    await expect(page.getByText('Nothing found yet')).toBeVisible()
    await expect(page.getByText('Nothing found', { exact: true })).toHaveCount(
      0,
    )

    // The command palette's page results say the same.
    await page.keyboard.press('ControlOrMeta+k')
    const palette = page.getByRole('dialog')
    await palette.getByRole('combobox').fill(word)
    await expect(
      palette.getByRole('status').filter({
        hasText: 'Some recent changes aren’t searchable yet.',
      }),
    ).toBeVisible()
    await page.keyboard.press('Escape')
    await expect(palette).toHaveCount(0)

    await backend('debug_set_search_indexer_paused', { paused: false })
    await expect(
      page
        .getByRole('listbox', { name: 'History results' })
        .getByRole('option')
        .filter({ hasText: title }),
    ).toHaveCount(1, { timeout: 30_000 })
    await expect(page.getByRole('status')).toHaveCount(0)
  } finally {
    await backend('debug_set_search_indexer_paused', { paused: false })
    await backend('set_url_notes', { request: { url, notes: '' } })
  }
})
