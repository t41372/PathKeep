/**
 * History's tools around search: tags, the search syntax, the regex dialect,
 * link previews, semantic status and the search tips.
 *
 * Proves: a tag added in the detail panel filters History to exactly that
 * page (and stops doing so once removed); a word that is only in a note, in
 * Latin or Chinese script, finds exactly that page, as do `note:` and
 * `-note:`, until the note changes; stars on pages from two profiles show in
 * the Starred list, on History's rows and in `is:starred`, and one removed
 * is gone from all three after a reload; `site:` with `-word` returns the
 * exact page set the fixture predicts, and so do a quoted phrase,
 * `intitle:`, `inurl:`, `filetype:pdf`, `A OR B` and `site:` with `after:`
 * and `before:`; removing the last tag of a page with a note keeps the note
 * (after a reload too, and `note:` still finds it) while `tag:` finds
 * nothing; a tag matches in any letter case, and `-tag:` leaves out its page
 * and keeps the untagged ones; the browser and date filters set both the
 * visits a row counts and the visit time it shows, and a range after every
 * visit lists nothing; a star made on an address with tracking parameters
 * stars every spelling of it, Starred lists it once, and a note made on
 * another spelling shows there; a Traditional Chinese query finds a
 * Simplified title and the reverse, and full-width letters find half-width
 * ones; a regex Rust cannot run is explained
 * in plain words and never sent, while the previous results stay; the link
 * preview box keeps its size from first paint to its final state, which
 * matches what the archive holds; a stored image is drawn and reported as
 * shown; the tips fill the search box with a query that works; semantic
 * search that is off says so; the palette calls its search hits pages.
 */
import { expect, test, type Page, type TestInfo } from '@playwright/test'
import { FIXTURE_SCENES } from '../../scripts/fixtures/synthetic-browsers.mjs'
import {
  archivedVisits,
  backend,
  expectCount,
  fixture,
  localDay,
  parseCount,
  type FixtureVisit,
  type ProfileId,
} from './support/fixture'

const PREVIEW_URL = 'https://news.ycombinator.com/item?id=41502281'
const PREVIEW_TITLE = 'Hacker News · Why I left tokio'
const TAG_URL = 'https://www.sqlite.org/fts5.html'
const TAG_TITLE = 'SQLite FTS5 Extension'
/** A space in the tag makes the filter quote it: `tag:"e2e fts"`. */
const TAG = 'e2e fts'
/** A page that gets a note and a tag, then loses the tag. */
const NOTED_URL = 'https://jlptsensei.com/jlpt-n3-grammar-list/'
/** A page tagged "Rust", in other letter case than the filters type it. */
const RUST_TAG_URL = 'https://docs.rs/smol/latest/smol/'

/** A 40×21 PNG, standing in for a downloaded og:image. */
const PNG =
  'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAACgAAAAVCAIAAAC7eDtJAAAAJUlEQVR4nGP4UKExIIhh1OJRi0ctHrV41OJRi0ctHrV41GKaIwDjSCDM9x1XGwAAAABJRU5ErkJggg=='

interface Expected {
  pages: number
  visits: number
  titles: string[]
}

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

const results = (page: Page) =>
  page.getByRole('listbox', { name: 'History results' }).getByRole('option')
const searchBox = (page: Page) =>
  page.getByRole('searchbox', { name: 'Search history' })
const detail = (page: Page) =>
  page.getByRole('complementary', { name: 'Page details' })

/** Waits for the exact "N pages · M visits · Full text" header, then records it. */
async function expectResults(
  page: Page,
  testInfo: TestInfo,
  name: string,
  want: Expected,
) {
  const pattern = /^([\d,]+) pages? · ([\d,]+) visits? · Full text$/
  const header = page.getByText(/visits? · Full text$/)
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

const RUN_URL = 'https://www.strava.com/activities/1001'
const RUN_TITLE = 'Strava · Afternoon run'
const LOG_URL = 'https://www.strava.com/athlete/training'
const LOG_TITLE = 'Strava · Training log'

/** Opens a listed page's details and waits until the panel shows it, not the page before. */
async function select(page: Page, title: string) {
  await results(page).filter({ hasText: title }).click()
  await expect(
    detail(page).getByRole('heading', { name: title, exact: true }),
  ).toBeVisible()
}

/** Waits until the archive holds `notes` for `url` (notes save after typing stops). */
async function expectSavedNote(url: string, notes: string) {
  await expect
    .poll(
      async () =>
        (await backend<{ notes: string } | null>('get_url_annotation', { url }))
          ?.notes,
    )
    .toBe(notes)
}

test('a note is found by its words, in either script, until it changes', async ({
  page,
}, testInfo) => {
  const onStrava = expected(archivedVisits(), (visit) =>
    visit.url.includes('strava.com/'),
  )
  const run = expected(archivedVisits(), (visit) => visit.url === RUN_URL)
  const log = expected(archivedVisits(), (visit) => visit.url === LOG_URL)
  expect(run.pages * log.pages, 'the fixture visited both pages').toBe(1)

  // No fixture page has these words; after the notes, only the noted pages do.
  await page.goto('/#/history')
  await searchBox(page).fill('site:strava.com')
  await expectResults(page, testInfo, 'site:strava.com', onStrava)
  await select(page, RUN_TITLE)
  const note = detail(page).getByRole('textbox', { name: 'Note' })
  await note.fill('quokka sighting')
  await expectSavedNote(RUN_URL, 'quokka sighting')
  await select(page, LOG_TITLE)
  await note.fill('设计系统')
  await expectSavedNote(LOG_URL, '设计系统')

  for (const [word, want] of [
    ['quokka', run],
    ['设计系统', log],
    ['note:quokka', run],
  ] as const) {
    await searchBox(page).fill(word)
    await expectResults(page, testInfo, word, want)
  }
  await searchBox(page).fill('site:strava.com -note:quokka')
  await expectResults(page, testInfo, 'site:strava.com -note:quokka', {
    ...onStrava,
    pages: onStrava.pages - 1,
    visits: onStrava.visits - run.visits,
  })
  await expect(results(page).filter({ hasText: RUN_TITLE })).toHaveCount(0)

  // Changed, the note answers to its new words and not the old ones.
  await searchBox(page).fill('site:strava.com')
  await select(page, RUN_TITLE)
  await note.fill('wombat sighting')
  await expectSavedNote(RUN_URL, 'wombat sighting')
  await searchBox(page).fill('quokka')
  await expect(page.getByText('Nothing found')).toBeVisible({
    timeout: 30_000,
  })
  await searchBox(page).fill('wombat')
  await expectResults(page, testInfo, 'wombat', run)
})

/**
 * A page `profile` visited whose title no other page has or contains, so a
 * quoted search for it lists one row; none of `taken`, and none another test
 * uses.
 */
function pageOf(profile: ProfileId, taken: string[]): FixtureVisit {
  const titles = new Map<string, Set<string>>()
  for (const visit of archivedVisits()) {
    titles.set(
      visit.title,
      (titles.get(visit.title) ?? new Set()).add(visit.url),
    )
  }
  const avoid = new Set([
    PREVIEW_URL,
    TAG_URL,
    RUN_URL,
    LOG_URL,
    NOTED_URL,
    RUST_TAG_URL,
    ...taken,
  ])
  const found = fixture().visitsByProfile[profile].find(
    (visit) =>
      !avoid.has(visit.url) &&
      !visit.url.includes('/search?') &&
      !visit.title.includes('"') &&
      titles.get(visit.title)?.size === 1 &&
      [...titles.keys()].every(
        (title) => title === visit.title || !title.includes(visit.title),
      ),
  )
  if (!found) throw new Error(`no page only ${profile} names this way`)
  return found
}

/** Finds `visit`'s page in History and opens its details. */
async function openDetails(page: Page, visit: FixtureVisit) {
  await page.goto('/#/history')
  await searchBox(page).fill(`"${visit.title}"`)
  const row = results(page).filter({ hasText: visit.title })
  await expect(row).toHaveCount(1)
  await row.click()
  await expect(detail(page)).toContainText(visit.url)
  return row
}

test('stars from two profiles: the Starred list and History agree after a reload', async ({
  page,
}) => {
  const work = pageOf('chrome:Profile 1', [])
  const firefox = pageOf('firefox:k3x9.default-release', [work.url])
  const personal = pageOf('chrome:Default', [work.url, firefox.url])
  for (const visit of [work, firefox, personal]) {
    await openDetails(page, visit)
    await detail(page).getByRole('button', { name: 'Star this page' }).click()
    await expect(
      detail(page).getByRole('button', { name: 'Remove star' }),
    ).toBeVisible()
  }
  await openDetails(page, firefox)
  await detail(page).getByRole('button', { name: 'Remove star' }).click()
  await expect(
    detail(page).getByRole('button', { name: 'Star this page' }),
  ).toBeVisible()

  await page.reload()
  await page.getByRole('tab', { name: /^Starred/ }).click()
  // The list itself: the details of the page last opened stay beside it.
  const starred = page.getByRole('listbox', { name: 'Starred' })
  await expect(starred).toContainText(work.title)
  await expect(starred).toContainText(personal.title)
  await expect(starred).not.toContainText(firefox.title)

  // History's rows say the same, and so does is:starred.
  for (const [visit, isStarred] of [
    [work, true],
    [firefox, false],
    [personal, true],
  ] as const) {
    const row = await openDetails(page, visit)
    await expect(row.getByRole('img', { name: 'Starred' })).toHaveCount(
      isStarred ? 1 : 0,
    )
    await searchBox(page).fill(`"${visit.title}" is:starred`)
    if (isStarred) {
      await expect(results(page).filter({ hasText: visit.title })).toHaveCount(
        1,
      )
    } else {
      await expect(page.getByText('Nothing found')).toBeVisible({
        timeout: 30_000,
      })
    }
  }
})

async function tagsOf(url: string) {
  const annotation = await backend<{ tags: string[] } | null>(
    'get_url_annotation',
    { url },
  )
  return annotation?.tags ?? []
}

test('a tag filters History to exactly the tagged page', async ({
  page,
}, testInfo) => {
  const want = expected(archivedVisits(), (visit) => visit.url === TAG_URL)
  expect(want.pages, 'the fixture visits the page to tag').toBe(1)

  // Before tagging, the filter finds nothing.
  await page.goto(`/#/history?q=${encodeURIComponent(`tag:"${TAG}"`)}`)
  await expect(page.getByText('Nothing found')).toBeVisible()

  await searchBox(page).fill('fts5 extension')
  await results(page).filter({ hasText: TAG_TITLE }).click()
  const panel = detail(page)
  await expect(panel).toContainText(TAG_URL)

  // Two tags by keyboard, then the second removed by keyboard: Backspace in
  // the empty input moves to the last chip, Backspace again removes it.
  const input = panel.getByRole('textbox', { name: 'Tags' })
  await input.fill(TAG)
  await input.press('Enter')
  await input.fill('temporary')
  await input.press(',')
  await expect.poll(() => tagsOf(TAG_URL)).toEqual([TAG, 'temporary'])
  await input.press('Backspace')
  await page.keyboard.press('Backspace')
  await expect.poll(() => tagsOf(TAG_URL)).toEqual([TAG])

  // The chip filters History to the tag.
  await panel.getByRole('button', { name: `Show pages tagged ${TAG}` }).click()
  await expect(searchBox(page)).toHaveValue(`tag:"${TAG}"`)
  await expectResults(page, testInfo, 'tag chip', want)
  await expect(results(page)).toHaveCount(1)
  await expect(results(page)).toContainText(TAG_TITLE)

  // Typed by hand in other letter case, the same page.
  await searchBox(page).fill('tag:"E2E FTS"')
  await expectResults(page, testInfo, 'typed tag', want)
  await expect(results(page)).toHaveCount(1)

  // Removed, the filter finds nothing again.
  await panel.getByRole('button', { name: `Remove tag ${TAG}` }).click()
  await expect.poll(() => tagsOf(TAG_URL)).toEqual([])
  await searchBox(page).fill(`tag:"${TAG}"`)
  await expect(page.getByText('Nothing found')).toBeVisible()
})

test('site: with -word returns the pages the fixture predicts', async ({
  page,
}, testInfo) => {
  const onSite = (visit: FixtureVisit) =>
    visit.url.toLowerCase().includes('docs.rs')
  const mentionsSmol = (visit: FixtureVisit) =>
    `${visit.title} ${visit.url}`.toLowerCase().includes('smol')
  const want = expected(
    archivedVisits(),
    (visit) => onSite(visit) && !mentionsSmol(visit),
  )
  const excluded = expected(
    archivedVisits(),
    (visit) => onSite(visit) && mentionsSmol(visit),
  )
  expect(want.pages, 'some docs.rs pages remain').toBeGreaterThan(0)
  expect(excluded.pages, 'the exclusion removes something').toBeGreaterThan(0)

  await page.goto('/#/history')
  await searchBox(page).fill('site:docs.rs -smol')
  await expectResults(page, testInfo, 'site:docs.rs -smol', want)
  await expect(results(page)).toHaveCount(want.pages)
  for (const title of want.titles) {
    await expect(results(page).filter({ hasText: title }), title).toHaveCount(1)
  }
  for (const title of excluded.titles) {
    await expect(results(page).filter({ hasText: title }), title).toHaveCount(0)
  }
})

/** The fixture's visit to `url` (its first), to open its details by title. */
function visitTo(url: string): FixtureVisit {
  const found = archivedVisits().find((visit) => visit.url === url)
  if (!found) throw new Error(`the fixture never visited ${url}`)
  return found
}

async function expectNothingFound(page: Page) {
  await expect(page.getByText('Nothing found', { exact: true })).toBeVisible({
    timeout: 30_000,
  })
}

test('removing the last tag of a noted page keeps its note', async ({
  page,
}, testInfo) => {
  const note = 'pangolin ledger'
  const tag = 'grammar drill'
  const want = expected(archivedVisits(), (visit) => visit.url === NOTED_URL)
  expect(want.pages, 'the fixture visits the page').toBe(1)

  await openDetails(page, visitTo(NOTED_URL))
  const panel = detail(page)
  await panel.getByRole('textbox', { name: 'Note' }).fill(note)
  await expectSavedNote(NOTED_URL, note)
  const input = panel.getByRole('textbox', { name: 'Tags' })
  await input.fill(tag)
  await input.press('Enter')
  await expect.poll(() => tagsOf(NOTED_URL)).toEqual([tag])
  await searchBox(page).fill(`tag:"${tag}"`)
  await expectResults(page, testInfo, 'tagged', want)

  // The page's only tag goes; its note stays, after a reload too.
  await panel.getByRole('button', { name: `Remove tag ${tag}` }).click()
  await expect.poll(() => tagsOf(NOTED_URL)).toEqual([])
  await page.reload()
  await openDetails(page, visitTo(NOTED_URL))
  await expect(panel.getByRole('textbox', { name: 'Note' })).toHaveValue(note)
  await expect(panel.getByRole('button', { name: /^Remove tag / })).toHaveCount(
    0,
  )
  await searchBox(page).fill(`note:"${note}"`)
  await expectResults(page, testInfo, `note:"${note}" after the tag went`, want)
  await expect(results(page)).toHaveCount(1)
  await searchBox(page).fill(`tag:"${tag}"`)
  await expectNothingFound(page)
})

test('a tag matches in any letter case, and -tag: leaves out only its page', async ({
  page,
}, testInfo) => {
  const onDocs = (visit: FixtureVisit) => visit.url.includes('://docs.rs/')
  const tagged = expected(
    archivedVisits(),
    (visit) => visit.url === RUST_TAG_URL,
  )
  const untagged = expected(
    archivedVisits(),
    (visit) => onDocs(visit) && visit.url !== RUST_TAG_URL,
  )
  expect(tagged.pages, 'the fixture visits the page to tag').toBe(1)
  expect(untagged.pages, 'docs.rs has other pages').toBeGreaterThan(0)

  await page.goto('/#/history?q=tag%3Arust')
  await expectNothingFound(page)

  const row = await openDetails(page, visitTo(RUST_TAG_URL))
  const input = detail(page).getByRole('textbox', { name: 'Tags' })
  await input.fill('Rust')
  await input.press('Enter')
  await expect.poll(() => tagsOf(RUST_TAG_URL)).toEqual(['Rust'])
  await expect(row).toBeVisible()

  for (const query of ['tag:rust', 'tag:RUST']) {
    await searchBox(page).fill(query)
    await expectResults(page, testInfo, query, tagged)
    await expect(results(page)).toHaveCount(1)
    await expect(results(page)).toContainText(tagged.titles[0])
  }

  await searchBox(page).fill('site:docs.rs -tag:rust')
  await expectResults(page, testInfo, 'site:docs.rs -tag:rust', untagged)
  await expect(results(page)).toHaveCount(untagged.pages)
  for (const title of untagged.titles)
    await expect(results(page).filter({ hasText: title }), title).toHaveCount(1)
  await expect(results(page).filter({ hasText: tagged.titles[0] })).toHaveCount(
    0,
  )

  // Put back: plain words also find a page by its tags.
  await detail(page).getByRole('button', { name: 'Remove tag Rust' }).click()
  await expect.poll(() => tagsOf(RUST_TAG_URL)).toEqual([])
})

test('each search operator lists exactly the pages the fixture predicts', async ({
  page,
}, testInfo) => {
  const DAY = 86_400_000
  const words = (visit: FixtureVisit) =>
    `${visit.title} ${visit.url}`.toLowerCase()
  const from = localDay(fixture().now - 60 * DAY)
  const to = localDay(fixture().now - 54 * DAY)
  const cases: [string, (visit: FixtureVisit) => boolean][] = [
    ['"async runtime"', (visit) => words(visit).includes('async runtime')],
    ['intitle:review', (visit) => visit.title.toLowerCase().includes('review')],
    ['inurl:review', (visit) => visit.url.toLowerCase().includes('review')],
    [
      'filetype:pdf',
      (visit) => new URL(visit.url).pathname.toLowerCase().endsWith('.pdf'),
    ],
    [
      'vibrancy OR strava',
      (visit) =>
        words(visit).includes('vibrancy') || words(visit).includes('strava'),
    ],
    [
      `site:strava.com after:${from} before:${to}`,
      (visit) =>
        visit.url.includes('://www.strava.com/') &&
        localDay(visit.at) >= from &&
        localDay(visit.at) <= to,
    ],
  ]
  const want = new Map(
    cases.map(([query, matches]) => [
      query,
      expected(archivedVisits(), matches),
    ]),
  )
  // Each operator must change the answer, or a search that ignored it
  // would pass: the words of the phrase apart find more, a title and an
  // address name different pages, and the dates leave some visits out.
  const bothWords = expected(
    archivedVisits(),
    (visit) =>
      words(visit).includes('async') && words(visit).includes('runtime'),
  )
  expect(bothWords.pages).toBeGreaterThan(want.get('"async runtime"')!.pages)
  expect(want.get('intitle:review')!.titles).not.toEqual(
    want.get('inurl:review')!.titles,
  )
  expect(want.get('filetype:pdf')!.titles).toEqual([FIXTURE_SCENES.pdf.title])
  expect(
    want.get(`site:strava.com after:${from} before:${to}`)!.visits,
  ).toBeLessThan(
    archivedVisits().filter((visit) => visit.url.includes('strava.com/'))
      .length,
  )
  for (const [query, { pages }] of want)
    expect(pages, `${query} finds something`).toBeGreaterThan(0)

  await page.goto('/#/history')
  for (const [query, expectedPages] of want) {
    await searchBox(page).fill(query)
    await expectResults(page, testInfo, query, expectedPages)
    await expect(results(page)).toHaveCount(expectedPages.pages)
    for (const title of expectedPages.titles)
      await expect(
        results(page).filter({ hasText: title }),
        `${query}: ${title}`,
      ).toHaveCount(1)
  }
})

/** Picks a range of days in the date filter's calendar. */
async function pickDays(page: Page, first: number, last: number) {
  await page.getByRole('button', { name: /^Date filter: / }).click()
  await page.getByRole('menuitem', { name: 'Custom range…' }).click()
  for (const at of [first, last]) {
    // The calendar's days, as the browser writes dates in its language.
    const day = page.locator(
      `[data-day="${new Date(at).toLocaleDateString('en-US')}"]`,
    )
    for (let back = 0; (await day.count()) === 0 && back < 3; back += 1)
      await page.getByRole('button', { name: /previous month/i }).click()
    await day.first().click()
  }
  await page.getByRole('button', { name: 'Apply', exact: true }).click()
}

async function chooseBrowser(page: Page, name: string) {
  await page.getByRole('button', { name: /^Browser filter: / }).click()
  await page.getByRole('menuitemradio', { name }).click()
  await expect(
    page.getByRole('button', { name: `Browser filter: ${name}` }),
  ).toBeVisible()
}

/**
 * The page's one row: its visit count, and the day and minute of the visit
 * it shows. The scene's visits are 8 days old or more, so the day reads as
 * a date, not "Yesterday".
 */
async function expectRow(
  page: Page,
  title: string,
  visits: number,
  at: number,
) {
  const row = results(page).filter({ hasText: title })
  await expect(row).toHaveCount(1)
  await expect(row).toContainText(`${visits} visit${visits === 1 ? '' : 's'}`)
  const when = new Date(at)
  await expect(row).toContainText(
    new Intl.DateTimeFormat('en', { month: 'short', day: 'numeric' }).format(
      when,
    ),
  )
  await expect(row).toContainText(
    new RegExp(`:${String(when.getMinutes()).padStart(2, '0')}\\b`),
  )
}

test('the browser and date filters set the visits a row counts and the time it shows', async ({
  page,
}, testInfo) => {
  const { url, title } = FIXTURE_SCENES.twoBrowsers
  const visits = archivedVisits()
    .filter((visit) => visit.url === url)
    .sort((a, b) => a.at - b.at)
  const isFirefox = (visit: FixtureVisit) =>
    visit.profile === 'firefox:k3x9.default-release'
  const firefox = visits.filter(isFirefox)
  const newest = (list: FixtureVisit[]) => list[list.length - 1]
  // A range with one visit from each browser: the first Chrome visit and the
  // Firefox one before it.
  const chrome = visits.find((visit) => !isFirefox(visit))!
  const firefoxBefore = newest(firefox.filter((visit) => visit.at < chrome.at))
  const inRange = visits.filter(
    (visit) =>
      localDay(visit.at) >= localDay(firefoxBefore.at) &&
      localDay(visit.at) <= localDay(chrome.at),
  )
  expect(inRange, 'one visit from each browser').toEqual([
    firefoxBefore,
    chrome,
  ])
  const page1 = (count: number) => ({ pages: 1, visits: count, titles: [] })

  await page.goto('/#/history')
  await searchBox(page).fill(`"${title}"`)
  await expectResults(page, testInfo, 'every browser', page1(visits.length))
  await expectRow(page, title, visits.length, newest(visits).at)

  await chooseBrowser(page, 'Firefox')
  await expectResults(page, testInfo, 'Firefox', page1(firefox.length))
  await expectRow(page, title, firefox.length, newest(firefox).at)

  await chooseBrowser(page, 'All browsers')
  await pickDays(page, firefoxBefore.at, chrome.at)
  await expectResults(page, testInfo, 'two days', page1(inRange.length))
  await expectRow(page, title, inRange.length, chrome.at)

  await chooseBrowser(page, 'Firefox')
  await expectResults(page, testInfo, 'two days, Firefox', page1(1))
  await expectRow(page, title, 1, firefoxBefore.at)

  // The last seven days come after every visit to it.
  await chooseBrowser(page, 'All browsers')
  await page.getByRole('button', { name: /^Date filter: / }).click()
  await page.getByRole('menuitemradio', { name: 'Last 7 days' }).click()
  expect(
    visits.every((visit) => visit.at < fixture().now - 7 * 86_400_000),
  ).toBe(true)
  await expectNothingFound(page)
})

test('a star made on an address with tracking parameters is the star of every spelling', async ({
  page,
}) => {
  const { title, visits } = FIXTURE_SCENES.spellings
  const [tracked, clean, other] = visits.map(([url]) => url)
  const archived = archivedVisits().filter((visit) => visit.title === title)
  expect(new Set(archived.map((visit) => visit.url))).toEqual(
    new Set([tracked, clean, other]),
  )
  /** A spelling's row: they share a title, so it is told by its visit's minute. */
  const rowOf = (url: string) => {
    const at = new Date(archived.find((visit) => visit.url === url)!.at)
    return results(page)
      .filter({ hasText: title })
      .filter({
        hasText: new RegExp(`:${String(at.getMinutes()).padStart(2, '0')}\\b`),
      })
  }
  const listSpellings = async () => {
    await page.goto('/#/history')
    await searchBox(page).fill(`"${title}"`)
    await expect(results(page).filter({ hasText: title })).toHaveCount(3)
  }

  await listSpellings()
  await rowOf(tracked).click()
  await expect(detail(page)).toContainText(tracked)
  await detail(page).getByRole('button', { name: 'Star this page' }).click()
  await expect(
    detail(page).getByRole('button', { name: 'Remove star' }),
  ).toBeVisible()

  // After a reload every spelling is starred, and Starred lists it once.
  await page.reload()
  await listSpellings()
  for (const url of [tracked, clean, other])
    await expect(
      rowOf(url).getByRole('img', { name: 'Starred' }),
      url,
    ).toHaveCount(1)
  await rowOf(clean).click()
  await expect(detail(page)).toContainText(clean)
  await expect(
    detail(page).getByRole('button', { name: 'Remove star' }),
  ).toBeVisible()

  // A note made on a third spelling shows on that one entry.
  const note = 'run it after the big import'
  await rowOf(other).click()
  await expect(detail(page)).toContainText(other)
  await detail(page).getByRole('textbox', { name: 'Note' }).fill(note)
  await expectSavedNote(other, note)
  await page.getByRole('tab', { name: /^Starred/ }).click()
  const entries = page
    .getByRole('listbox', { name: 'Starred' })
    .getByRole('option')
    .filter({ hasText: title })
  await expect(entries).toHaveCount(1)
  await expect(entries).toContainText(note)
})

test('Chinese search finds Traditional and Simplified alike, and full-width letters as half-width', async ({
  page,
}, testInfo) => {
  const { title: simplified } = FIXTURE_SCENES.simplified
  const inTitle = (part: string) =>
    expected(archivedVisits(), (visit) => visit.title.includes(part))
  const tanstack = expected(archivedVisits(), (visit) =>
    `${visit.title} ${visit.url}`.toLowerCase().includes('tanstack'),
  )
  // The fixture's searches are titled in Traditional, the scene's page in
  // Simplified; neither spelling of either is in the other's title.
  const cases: [string, Expected][] = [
    ['台北天气', inTitle('台北天氣')],
    ['搜索說明', inTitle('搜索说明')],
    ['ＴａｎＳｔａｃｋ', tanstack],
    ['tanstack', tanstack],
  ]
  expect(inTitle('台北天气').pages).toBe(0)
  expect(inTitle('搜索說明').pages).toBe(0)
  expect(inTitle('搜索说明').titles).toEqual([simplified])
  expect(inTitle('台北天氣').pages).toBeGreaterThan(1)

  await page.goto('/#/history')
  for (const [query, want] of cases) {
    await searchBox(page).fill(query)
    await expectResults(page, testInfo, query, want)
    await expect(results(page)).toHaveCount(want.pages)
    for (const title of want.titles)
      await expect(
        results(page).filter({ hasText: title }),
        `${query}: ${title}`,
      ).toHaveCount(1)
  }
})

test('a regex Rust cannot run is explained and never sent', async ({
  page,
}) => {
  const sent: string[] = []
  page.on('request', (request) => {
    if (request.url().endsWith('/commands/query_history'))
      sent.push(request.postData() ?? '')
  })

  await page.goto('/#/history?mode=regex')
  await searchBox(page).fill('tokio')
  // The header counts once the regex rows have replaced the timeline.
  const header = page.getByText(/^[\d,]+ pages? · [\d,]+ visits? · Regex$/)
  await expect(header).toBeVisible()
  const before = parseCount((await header.innerText()).split(' · ')[0])
  await expect(results(page)).toHaveCount(before)
  expect(before).toBeGreaterThan(0)

  sent.length = 0
  await searchBox(page).fill('tokio(?=::)')
  await expect(page.getByText(/Look-ahead and look-behind/)).toBeVisible()
  // Past the 250 ms debounce: still nothing sent, and the old rows stay.
  await page.waitForTimeout(1_000)
  expect(sent.filter((body) => body.includes('(?='))).toEqual([])
  await expect(results(page)).toHaveCount(before)
  await expect(page.getByText('The search didn’t run')).toHaveCount(0)

  await searchBox(page).fill('(tokio)\\1')
  await expect(page.getByText(/Back-references/)).toBeVisible()
  await page.waitForTimeout(1_000)
  expect(sent.filter((body) => body.includes('\\\\1'))).toEqual([])

  // A pattern JavaScript rejects but Rust accepts still runs.
  await searchBox(page).fill('tok+io')
  await expect(
    page.getByText(/^[\d,]+ pages? · [\d,]+ visits? · Regex$/),
  ).toBeVisible()
  await expect(page.getByText('Invalid regex')).toHaveCount(0)
})

/** Size of the preview box and the gap to what follows it, rounded to pixels. */
function measurePreview(page: Page) {
  return detail(page)
    .getByRole('figure', { name: 'Link preview' })
    .evaluate((figure) => {
      const box = figure.getBoundingClientRect()
      const next = figure.nextElementSibling?.getBoundingClientRect()
      return {
        width: Math.round(box.width),
        height: Math.round(box.height),
        gapBelow: Math.round((next?.top ?? 0) - box.bottom),
      }
    })
}

/** The preview state the archive's status should produce once the panel settles. */
function stateFor(status: string) {
  if (status === 'ok') return 'ok'
  if (status === 'missing' || status === 'blocked') return status
  return 'failed'
}

test('the link preview keeps its size and shows what the archive holds', async ({
  page,
}, testInfo) => {
  // Hold the preview lookup so the loading state can be measured.
  let release = () => {}
  const held = new Promise<void>((resolve) => (release = resolve))
  await page.route('**/commands/load_history_og_images', async (route) => {
    await held
    await route.continue()
  })

  await page.goto('/#/history')
  await searchBox(page).fill('why I left tokio')
  await results(page).filter({ hasText: PREVIEW_TITLE }).click()
  const figure = detail(page).getByRole('figure', { name: 'Link preview' })
  await expect(figure).toHaveAttribute('data-preview-state', 'loading')
  // Let the panel finish sliding in before measuring.
  await page.waitForTimeout(400)
  const loading = await measurePreview(page)
  expect(loading.width / loading.height).toBeCloseTo(1.91, 1)

  release()
  // A page never tried before is downloaded now (up to the 12 s fetch limit).
  await expect(figure).not.toHaveAttribute(
    'data-preview-state',
    /loading|fetching/,
    { timeout: 30_000 },
  )
  const settled = await measurePreview(page)
  await testInfo.attach('preview box: loading vs settled', {
    body: JSON.stringify({ loading, settled }),
    contentType: 'application/json',
  })
  expect(settled).toEqual(loading)

  const [stored] = await backend<{ fetchStatus: string }[]>(
    'load_history_og_images',
    { entries: [{ url: PREVIEW_URL }] },
  )
  await testInfo.attach('archive preview status', {
    body: JSON.stringify(stored),
    contentType: 'application/json',
  })
  await expect(figure).toHaveAttribute(
    'data-preview-state',
    stateFor(stored.fetchStatus),
  )
  if (stored.fetchStatus === 'missing') {
    await expect(figure).toContainText('This page has no preview image')
  }
})

test('a stored preview image is drawn and reported as shown', async ({
  page,
}) => {
  // The archive answers as usual, except that this page now holds an image.
  await page.route('**/commands/load_history_og_images', async (route) => {
    const response = await route.fetch()
    const rows = (await response.json()) as { url: string }[]
    await route.fulfill({
      response,
      json: rows.map((row) =>
        row.url === PREVIEW_URL
          ? { ...row, fetchStatus: 'ok', ogImage: { dataUrl: PNG } }
          : row,
      ),
    })
  })
  const shown = page.waitForRequest(
    (request) =>
      request.url().endsWith('/commands/mark_og_images_shown') &&
      (request.postData() ?? '').includes(PREVIEW_URL),
  )

  await page.goto('/#/history')
  await searchBox(page).fill('why I left tokio')
  await results(page).filter({ hasText: PREVIEW_TITLE }).click()
  const figure = detail(page).getByRole('figure', { name: 'Link preview' })
  const image = figure.getByRole('img', {
    name: `Preview image of ${PREVIEW_TITLE}`,
  })
  await expect(image).toBeVisible()
  await expect(figure).toHaveAttribute('data-preview-state', 'ok')
  expect(
    await image.evaluate((img: HTMLImageElement) => img.naturalWidth),
  ).toBe(40)
  const box = await measurePreview(page)
  expect(box.width / box.height).toBeCloseTo(1.91, 1)
  await shown
})

test('search tips, semantic status and the palette', async ({
  page,
}, testInfo) => {
  await page.goto('/#/history')
  await page.getByRole('button', { name: 'Search tips' }).click()
  await page
    .getByRole('dialog')
    .getByRole('button', { name: /site:docs\.rs tokio/ })
    .click()
  await expect(searchBox(page)).toHaveValue('site:docs.rs tokio')
  const want = expected(
    archivedVisits(),
    (visit) =>
      visit.url.toLowerCase().includes('docs.rs') &&
      `${visit.title} ${visit.url}`.toLowerCase().includes('tokio'),
  )
  expect(want.pages).toBeGreaterThan(0)
  await expectResults(page, testInfo, 'tip: site:docs.rs tokio', want)

  // Semantic search is off in this archive: the page says so and why.
  await page.goto('/#/history?q=tokio&mode=semantic')
  await expect(
    page.getByText(
      'Semantic search is off. These are full-text results instead.',
    ),
  ).toBeVisible()
  await expect(page.getByText(/· Full text$/)).toBeVisible()

  await page.keyboard.press('ControlOrMeta+k')
  await page.getByRole('dialog').getByRole('combobox').fill('tokio')
  await expect(
    page.getByRole('dialog').getByText('Pages', { exact: true }),
  ).toBeVisible()
})
