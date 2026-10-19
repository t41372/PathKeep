/**
 * History: finding one page among twenty thousand visits, three ways, then
 * keeping it with a star and a note.
 *
 * Proves: search lists each matching page once, with exactly the visits that
 * match it (not a page of them, not near-misses), in full-text and regex; the
 * browser filter narrows the count to one profile's visits; a word on several
 * pages lists every one of them once; the detail panel counts visits across
 * browsers; and a star and note survive a reload.
 */
import { expect, test, type Page, type TestInfo } from '@playwright/test'
import {
  archivedVisits,
  backend,
  expectCount,
  fixture,
  parseCount,
  type FixtureVisit,
} from './support/fixture'

const PAGE_URL = 'https://news.ycombinator.com/item?id=41502281'
const PAGE_TITLE = 'Hacker News · Why I left tokio'

interface Expected {
  pages: number
  visits: number
  /** One title per page, sorted. */
  titles: string[]
}

/** What a search should find, worked out from the visits the fixture wrote. */
function expected(
  visits: FixtureVisit[],
  matches: (visit: FixtureVisit) => boolean,
): Expected {
  const hits = visits.filter(matches)
  const titles = new Map(hits.map((visit) => [visit.url, visit.title]))
  return {
    pages: titles.size,
    visits: hits.length,
    titles: [...titles.values()].sort(),
  }
}

/**
 * Waits until the result header reads exactly "N pages · M visits · mode",
 * then records both numbers. The header first describes the loaded rows and
 * fills in the totals when the separate count query lands, so a single read
 * would race it.
 */
async function expectResults(
  page: Page,
  testInfo: TestInfo,
  name: string,
  mode: string,
  want: Expected,
) {
  const pattern = new RegExp(`^([\\d,]+) pages? · ([\\d,]+) visits? · ${mode}$`)
  const header = page.getByText(new RegExp(`visits? · ${mode}$`))
  let actual = { pages: Number.NaN, visits: Number.NaN }
  await expect
    .poll(async () => {
      const match = (await header.innerText().catch(() => '')).match(pattern)
      actual = match
        ? { pages: parseCount(match[1]), visits: parseCount(match[2]) }
        : { pages: Number.NaN, visits: Number.NaN }
      return actual
    })
    .toEqual({ pages: want.pages, visits: want.visits })
  await expectCount(testInfo, `${name}: pages`, actual.pages, want.pages)
  await expectCount(testInfo, `${name}: visits`, actual.visits, want.visits)
}

const results = (page: Page) =>
  page.getByRole('listbox', { name: 'History results' }).getByRole('option')

/** The page's single row, showing its visit count. */
async function expectOnlyRow(page: Page, title: string, visits: number) {
  const rows = results(page).filter({ hasText: title })
  await expect(rows).toHaveCount(1)
  await expect(rows).toContainText(
    `${visits.toLocaleString('en-US')} visit${visits === 1 ? '' : 's'}`,
  )
}

test('search lists each matching page once with its visits', async ({
  page,
}, testInfo) => {
  const toPage = (visit: FixtureVisit) => visit.url === PAGE_URL
  const everywhere = expected(archivedVisits(), toPage)
  expect(
    everywhere.visits,
    'the fixture must revisit the page',
  ).toBeGreaterThan(1)

  await page.goto('/#/history')
  const search = page.getByRole('searchbox', { name: 'Search history' })

  // Every word of the title has to match; "tokio" alone matches many pages.
  await search.fill('why I left tokio')
  await expectResults(page, testInfo, 'full text', 'Full text', everywhere)
  await expect(results(page)).toHaveCount(everywhere.pages)
  await expectOnlyRow(page, PAGE_TITLE, everywhere.visits)

  // Regex over the URL: the escaped `?` must not turn into a wildcard.
  await page.getByRole('radio', { name: 'Regex' }).click()
  await search.fill('item\\?id=41502281$')
  await expectResults(page, testInfo, 'regex', 'Regex', everywhere)
  await expectOnlyRow(page, PAGE_TITLE, everywhere.visits)

  // Only Firefox's visits to the same page.
  await page.getByRole('button', { name: /All browsers/ }).click()
  await page
    .getByRole('menuitemcheckbox', { name: /^Firefox/ })
    .or(page.getByRole('menuitemradio', { name: /^Firefox/ }))
    .first()
    .click()
  await page.keyboard.press('Escape')
  const firefox = expected(
    fixture().visitsByProfile['firefox:k3x9.default-release'],
    toPage,
  )
  await expectResults(page, testInfo, 'regex, Firefox only', 'Regex', firefox)
  await expectOnlyRow(page, PAGE_TITLE, firefox.visits)
})

test('a word on several pages lists every page once', async ({
  page,
}, testInfo) => {
  const want = expected(archivedVisits(), (visit) =>
    `${visit.title} ${visit.url}`.toLowerCase().includes('tokio'),
  )
  expect(want.pages, 'the fixture has several tokio pages').toBeGreaterThan(2)

  await page.goto('/#/history')
  await page.getByRole('searchbox', { name: 'Search history' }).fill('tokio')
  await expectResults(page, testInfo, 'tokio', 'Full text', want)
  // As many rows as pages, and each page's title on exactly one of them.
  await expect(results(page)).toHaveCount(want.pages)
  for (const title of want.titles) {
    await expect(results(page).filter({ hasText: title }), title).toHaveCount(1)
  }
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
  // Until the search lands the list is still the timeline, which can show
  // this page twice near the top (two visits yesterday, say).
  const row = results(page).filter({ hasText: PAGE_TITLE })
  await expect(row).toHaveCount(1)
  await expect(row).toContainText(/\d visits?/)
  await row.click()

  const detail = page.getByRole('complementary', { name: 'Page details' })
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
  await page.getByRole('tab', { name: /^Starred/ }).click()
  const starred = page.getByRole('main')
  await expect(starred).toContainText(PAGE_TITLE)
  await expect(starred).toContainText('read again before the runtime rewrite')
})
