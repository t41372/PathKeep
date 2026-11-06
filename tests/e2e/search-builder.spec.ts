/**
 * History's sort menu and Advanced search.
 *
 * Proves: "Oldest first" lists what a search finds by each page's first
 * visit and the row shows that visit, "Newest first" by each page's latest
 * visit, "Most visited" by its visits, with the count the order goes by on
 * the row; the order is kept in the URL, survives a reload and holds in
 * Regex mode, which ranks by visits too and where "Best match" can't be
 * picked and the menu says why. Advanced
 * search's Words tab writes "all of", "any of" and "none of" as one query
 * that finds what Google would (tokio, and runtime or smol, but not google):
 * the page about smol alone, which an OR splitting the whole query would
 * also find, is not listed. Its Pattern tab turns the examples into regexes
 * that find exactly the fixture's matching pages, so typed punctuation is
 * escaped and "one of" and "anything up to the next /" mean what they say;
 * a pattern put together piece by piece does the same.
 */
import { expect, test, type Page, type TestInfo } from '@playwright/test'
import { unlockIfLocked } from './support/app'
import {
  archivedVisits,
  backend,
  expectCount,
  parseCount,
  type FixtureVisit,
} from './support/fixture'

interface FoundPage {
  title: string
  first: number
  last: number
  visits: number
}

/** The pages a search should find, worked out from the visits the fixture wrote. */
function pagesMatching(matches: (visit: FixtureVisit) => boolean) {
  const pages = new Map<string, FoundPage>()
  for (const visit of archivedVisits().filter(matches)) {
    const found = pages.get(visit.url)
    if (found) {
      found.first = Math.min(found.first, visit.at)
      found.last = Math.max(found.last, visit.at)
      found.visits += 1
    } else {
      pages.set(visit.url, {
        title: visit.title,
        first: visit.at,
        last: visit.at,
        visits: 1,
      })
    }
  }
  return [...pages.values()]
}

const words = (visit: FixtureVisit) =>
  `${visit.title} ${visit.url}`.toLowerCase()

interface Annotation {
  url: string
  notes: string
  tags: string[]
}

/**
 * Each address's note and tags as the archive holds them now. Full-text
 * search reads them too, and specs that ran earlier write some (history.spec
 * notes "runtime" on the Hacker News page).
 */
async function annotationWords() {
  const found = new Map<string, string>()
  let cursor: string | null = null
  do {
    const page: { items: Annotation[]; nextCursor: string | null } =
      await backend('list_url_annotations', { cursor })
    for (const item of page.items)
      found.set(item.url, `${item.notes} ${item.tags.join(' ')}`.toLowerCase())
    cursor = page.nextCursor
  } while (cursor)
  return found
}
const results = (page: Page) =>
  page.getByRole('listbox', { name: 'History results' }).getByRole('option')
const searchBox = (page: Page) =>
  page.getByRole('searchbox', { name: 'Search history' })
const builder = (page: Page) =>
  page.getByRole('dialog', { name: 'Advanced search' })
const preview = (page: Page) =>
  builder(page).getByRole('status', { name: 'Searches for' })

async function visit(page: Page, url: string) {
  await page.goto(url)
  await unlockIfLocked(page)
}

/** Waits for the exact "N pages · M visits · mode" line, then records both numbers. */
async function expectTotals(
  page: Page,
  testInfo: TestInfo,
  name: string,
  mode: 'Full text' | 'Regex',
  found: FoundPage[],
) {
  const want = {
    pages: found.length,
    visits: found.reduce((sum, page) => sum + page.visits, 0),
  }
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
    .toEqual(want)
  await expectCount(testInfo, `${name}: pages`, actual.pages, want.pages)
  await expectCount(testInfo, `${name}: visits`, actual.visits, want.visits)
}

/** Each of these pages is listed once (and, with the totals, nothing else is). */
async function expectListed(page: Page, found: FoundPage[]) {
  for (const { title } of found)
    await expect(results(page).filter({ hasText: title }), title).toHaveCount(1)
}

/** The first rows, in order, are these pages. */
async function expectOrder(page: Page, titles: string[]) {
  for (const [index, title] of titles.entries())
    await expect(results(page).nth(index), `row ${index + 1}`).toContainText(
      title,
    )
}

async function pickSort(page: Page, from: string, to: string) {
  await page.getByRole('button', { name: `Sort results: ${from}` }).click()
  await page.getByRole('menuitemradio', { name: new RegExp(`^${to}`) }).click()
  await expect(
    page.getByRole('button', { name: `Sort results: ${to}` }),
  ).toBeVisible()
}

const shortDay = (at: number) =>
  new Intl.DateTimeFormat('en', { month: 'short', day: 'numeric' }).format(
    new Date(at),
  )

test('the sort menu orders results by first or latest visit and keeps the order', async ({
  page,
}, testInfo) => {
  const notes = await annotationWords()
  const found = pagesMatching((visit) =>
    `${words(visit)} ${notes.get(visit.url) ?? ''}`.includes('tokio'),
  )
  const byFirst = [...found].sort((a, b) => a.first - b.first)
  const byLatest = [...found].sort((a, b) => b.last - a.last)
  const byVisits = [...found].sort(
    (a, b) => b.visits - a.visits || b.last - a.last,
  )
  const top = (pages: FoundPage[]) => pages.slice(0, 5).map((p) => p.title)
  expect(found.length, 'enough pages to see an order').toBeGreaterThan(5)
  // Sorting by the latest visit, ascending, must not pass for "oldest first",
  // nor the latest visit for "most visited".
  expect(top(byFirst)).not.toEqual(top([...byLatest].reverse()))
  expect(top(byVisits)).not.toEqual(top(byLatest))

  await visit(page, '/#/history?q=tokio')
  await expectTotals(page, testInfo, 'tokio', 'Full text', found)

  await pickSort(page, 'Best match', 'Oldest first')
  await expect(page).toHaveURL(/[?&]sort=oldest(&|$)/)
  await expectOrder(page, top(byFirst))
  // The row shows the page's first visit, not its latest.
  await expect(results(page).first()).toContainText(shortDay(byFirst[0].first))

  await pickSort(page, 'Oldest first', 'Most visited')
  await expect(page).toHaveURL(/[?&]sort=visits(&|$)/)
  await expectOrder(page, top(byVisits))
  // The count on the row is the one the order goes by.
  await expect(results(page).first()).toContainText(
    `${byVisits[0].visits.toLocaleString('en-US')} visits`,
  )

  await pickSort(page, 'Most visited', 'Newest first')
  await expect(page).toHaveURL(/[?&]sort=newest(&|$)/)
  await expectOrder(page, top(byLatest))

  await page.reload()
  await unlockIfLocked(page)
  await expect(
    page.getByRole('button', { name: 'Sort results: Newest first' }),
  ).toBeVisible()
  await expectOrder(page, top(byLatest))

  // A regex has no best match: the order holds, and the menu says why.
  await page.getByRole('radio', { name: 'Regex' }).click()
  await expectTotals(page, testInfo, 'regex tokio', 'Regex', found)
  await page.getByRole('button', { name: 'Sort results: Newest first' }).click()
  await expect(
    page.getByRole('menuitemradio', { name: /^Best match/ }),
  ).toBeDisabled()
  await expect(
    page.getByText(
      'A regex either matches or it doesn’t, so there’s no best match.',
    ),
  ).toBeVisible()
  await page.getByRole('menuitemradio', { name: /^Oldest first/ }).click()
  await expectOrder(page, top(byFirst))
  await pickSort(page, 'Oldest first', 'Most visited')
  await expectOrder(page, top(byVisits))
  await pickSort(page, 'Most visited', 'Oldest first')

  await page.getByRole('radio', { name: 'Full text' }).click()
  await expect(
    page.getByRole('button', { name: 'Sort results: Oldest first' }),
  ).toBeVisible()
  await expectOrder(page, top(byFirst))
})

test('Advanced search writes all, any and none of these words as one query', async ({
  page,
}, testInfo) => {
  const notes = await annotationWords()
  const has = (visit: FixtureVisit, word: string) =>
    `${words(visit)} ${notes.get(visit.url) ?? ''}`.includes(word)
  const want = pagesMatching(
    (visit) =>
      has(visit, 'tokio') &&
      (has(visit, 'runtime') || has(visit, 'smol')) &&
      !has(visit, 'google'),
  )
  // Read with OR splitting the whole query ("tokio runtime", or "smol"), the
  // same text would also find pages about smol alone.
  const splitWhole = pagesMatching(
    (visit) =>
      ((has(visit, 'tokio') && has(visit, 'runtime')) || has(visit, 'smol')) &&
      !has(visit, 'google'),
  )
  expect(want.length).toBeGreaterThan(0)
  expect(splitWhole.length).toBeGreaterThan(want.length)
  const smolOnly = splitWhole.find(
    (page) => !want.some((other) => other.title === page.title),
  )!

  await visit(page, '/#/history')
  await page.getByRole('button', { name: 'Advanced search' }).click()
  const form = builder(page)
  await form.getByLabel('All of these words', { exact: true }).fill('tokio')
  await form
    .getByLabel('Any of these words', { exact: true })
    .fill('runtime smol')
  await form.getByLabel('None of these words', { exact: true }).fill('google')
  const query = 'tokio runtime OR smol -google'
  await expect(preview(page)).toHaveText(query)
  await form.getByRole('button', { name: 'Search', exact: true }).click()
  await expect(form).toBeHidden()

  await expect(searchBox(page)).toHaveValue(query)
  await expectTotals(page, testInfo, 'words', 'Full text', want)
  await expectListed(page, want)
  await expect(results(page).filter({ hasText: smolOnly.title })).toHaveCount(0)

  // A word that starts and ends with a slash stays a word, not a regex.
  await page.getByRole('button', { name: 'Advanced search' }).click()
  await form.getByRole('button', { name: 'Clear', exact: true }).click()
  await form.getByLabel('All of these words', { exact: true }).fill('/docs/')
  await expect(preview(page)).toHaveText('"/docs/"')
  await form.getByRole('button', { name: 'Search', exact: true }).click()
  await expect(page.getByRole('radio', { name: 'Full text' })).toBeChecked()
  await expect(page.getByText(/visits? · Full text$/)).toBeVisible()
})

test('Advanced search turns pieces into a regex that finds exactly the matching pages', async ({
  page,
}, testInfo) => {
  const matching = (regex: RegExp) =>
    pagesMatching((visit) => regex.test(visit.url) || regex.test(visit.title))
  // Unescaped, `item?id=` would ask for "iteid=" or "itemid=", and find nothing.
  const hackerNews = matching(/news\.ycombinator\.com\/item\?id=\d+/i)
  const githubThreads = matching(
    /github\.com\/[^/]+\/[^/]+\/(?:pull|issues)\/\d+/i,
  )
  const docsPages = matching(/docs\.rs\/[^/]+\/latest\//i)
  // A piece ending in a space keeps it: "Rust " is not "Rust" ("smol - Rust").
  const rustWord = matching(/rust /i)
  for (const found of [hackerNews, githubThreads, docsPages, rustWord])
    expect(found.length).toBeGreaterThan(0)
  expect(rustWord.length).toBeLessThan(matching(/rust/i).length)

  await visit(page, '/#/history')
  await page.getByRole('button', { name: 'Advanced search' }).click()
  await builder(page).getByRole('tab', { name: 'Pattern' }).click()
  await builder(page)
    .getByRole('button', { name: 'Hacker News threads' })
    .click()
  await expect(preview(page)).toHaveText(
    'news\\.ycombinator\\.com/item\\?id=\\d+',
  )
  await builder(page)
    .getByRole('button', { name: 'Search', exact: true })
    .click()
  await expect(searchBox(page)).toHaveValue(
    'news\\.ycombinator\\.com/item\\?id=\\d+',
  )
  await expect(page.getByRole('radio', { name: 'Regex' })).toBeChecked()
  await expectTotals(page, testInfo, 'Hacker News threads', 'Regex', hackerNews)
  await expectListed(page, hackerNews)

  // In Regex mode the form opens on Pattern.
  await page.getByRole('button', { name: 'Advanced search' }).click()
  await builder(page)
    .getByRole('button', { name: 'GitHub pull requests and issues' })
    .click()
  await expect(preview(page)).toHaveText(
    'github\\.com/[^/]+/[^/]+/(?:pull|issues)/\\d+',
  )
  await builder(page)
    .getByRole('button', { name: 'Search', exact: true })
    .click()
  await expectTotals(page, testInfo, 'GitHub threads', 'Regex', githubThreads)
  await expectListed(page, githubThreads)

  // Piece by piece: docs.rs/, anything up to the next /, then /latest/.
  await page.getByRole('button', { name: 'Advanced search' }).click()
  const form = builder(page)
  await form.getByRole('button', { name: 'Clear', exact: true }).click()
  await form.getByLabel('Piece 1: This text').fill('docs.rs/')
  await form.getByRole('button', { name: 'Add a piece' }).click()
  await page
    .getByRole('menuitem', { name: 'Anything up to the next /' })
    .click()
  await form.getByRole('button', { name: 'Add a piece' }).click()
  await page.getByRole('menuitem', { name: 'This text' }).click()
  await form.getByLabel('Piece 3: This text').fill('/latest/')
  await expect(preview(page)).toHaveText('docs\\.rs/[^/]+/latest/')
  await form.getByRole('button', { name: 'Search', exact: true }).click()
  await expectTotals(page, testInfo, 'docs.rs pieces', 'Regex', docsPages)
  await expectListed(page, docsPages)

  await page.getByRole('button', { name: 'Advanced search' }).click()
  await form.getByRole('button', { name: 'Clear', exact: true }).click()
  await form.getByLabel('Piece 1: This text').fill('Rust ')
  await form.getByRole('button', { name: 'Search', exact: true }).click()
  // The box trims what it holds, so the pattern comes wrapped in slashes.
  await expect(searchBox(page)).toHaveValue('/Rust /')
  await expectTotals(
    page,
    testInfo,
    'a piece ending in a space',
    'Regex',
    rustWord,
  )
})
