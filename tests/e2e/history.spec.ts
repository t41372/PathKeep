/**
 * History: finding one page among twenty thousand visits, three ways, then
 * keeping it with a star and a note.
 *
 * Proves: full-text and regex search return exactly the visits that match
 * (not a page of them, not near-misses), the browser filter narrows to one
 * profile's visits, the detail panel counts visits across browsers, and a
 * star and note survive a reload.
 */
import { expect, test, type Page } from '@playwright/test'
import {
  archivedVisits,
  backend,
  expectCount,
  fixture,
  parseCount,
} from './support/fixture'

const PAGE_URL = 'https://news.ycombinator.com/item?id=41502281'
const PAGE_TITLE = 'Hacker News · Why I left tokio'

function resultCount(page: Page, mode: string) {
  return page.getByText(new RegExp(`^[\\d,]+ results? · ${mode}$`))
}

test('search finds every visit to a page, and nothing else', async ({
  page,
}, testInfo) => {
  const expected = archivedVisits().filter(
    (visit) => visit.url === PAGE_URL,
  ).length
  expect(expected, 'the fixture must contain the page').toBeGreaterThan(1)

  await page.goto('/#/history')
  const search = page.getByRole('searchbox', { name: 'Search history' })

  // Every word of the title has to match; "tokio" alone matches many pages.
  await search.fill('why I left tokio')
  const fullText = resultCount(page, 'Full text')
  await expect(fullText).toBeVisible()
  await expectCount(
    testInfo,
    'full-text matches',
    parseCount(await fullText.innerText()),
    expected,
  )
  const rows = page
    .getByRole('list', { name: 'History results' })
    .getByRole('listitem')
  await expect(rows.first()).toContainText(PAGE_TITLE)

  // Regex over the URL: the escaped `?` must not turn into a wildcard.
  await page.getByRole('radio', { name: 'Regex' }).click()
  await search.fill('item\\?id=41502281$')
  const regex = resultCount(page, 'Regex')
  await expect(regex).toBeVisible()
  await expectCount(
    testInfo,
    'regex matches',
    parseCount(await regex.innerText()),
    expected,
  )

  // Only Firefox's visits to the same page.
  const firefoxVisits = fixture().visitsByProfile[
    'firefox:k3x9.default-release'
  ].filter((visit) => visit.url === PAGE_URL).length
  await page.getByRole('button', { name: /All browsers/ }).click()
  await page
    .getByRole('menuitemcheckbox', { name: /^Firefox/ })
    .or(page.getByRole('menuitemradio', { name: /^Firefox/ }))
    .first()
    .click()
  await page.keyboard.press('Escape')
  await expect(regex).toHaveText(
    new RegExp(`^${firefoxVisits.toLocaleString('en-US')} results? · Regex$`),
  )
  await expectCount(
    testInfo,
    'regex matches in Firefox only',
    parseCount(await regex.innerText()),
    firefoxVisits,
  )
})

test('a starred page keeps its note after a reload', async ({
  page,
}, testInfo) => {
  const totalVisits = archivedVisits().filter(
    (visit) => visit.url === PAGE_URL,
  ).length

  await page.goto('/#/history')
  await page
    .getByRole('searchbox', { name: 'Search history' })
    .fill('why I left tokio')
  await page
    .getByRole('list', { name: 'History results' })
    .getByText(PAGE_TITLE)
    .first()
    .click()

  const detail = page
    .getByRole('complementary', { name: 'Page details' })
    .or(page.getByRole('region', { name: 'Page details' }))
    .first()
  await expect(detail).toContainText(PAGE_URL)
  const visitsStat = detail.getByText('Total visits').locator('..')
  await expect(visitsStat).toContainText(/\d/)
  await expectCount(
    testInfo,
    'total visits in the detail panel',
    parseCount((await visitsStat.innerText()).replace('Total visits', '')),
    totalVisits,
  )

  await detail.getByRole('button', { name: 'Star this page' }).click()
  await expect(
    detail.getByRole('button', { name: 'Remove star' }),
  ).toBeVisible()
  const note = detail.getByRole('textbox', { name: 'Note' })
  await note.fill('read again before the runtime rewrite')
  // Notes save on their own after typing stops; wait until the archive has it.
  await expect
    .poll(
      async () =>
        (
          await backend<{ notes: string } | null>('get_url_annotation', {
            url: PAGE_URL,
          })
        )?.notes,
    )
    .toBe('read again before the runtime rewrite')

  await page.reload()
  await page.getByRole('radio', { name: /^Starred/ }).click()
  const starred = page.getByRole('main')
  await expect(starred).toContainText(PAGE_TITLE)
  await expect(starred).toContainText('read again before the runtime rewrite')
})
