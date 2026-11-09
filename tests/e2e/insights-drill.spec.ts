/**
 * Insights drill-ins: a day, a site, a search and a page, reached the way a
 * user reaches them, show the archive's numbers, and lead on to History.
 *
 * Proves:
 * - Home's year heatmap opens the day view; for a fixture day its visit
 *   count, top site and searches equal a count of the fixture for that
 *   local day, its "Pages reopened" counts the pages its card lists, and a
 *   day before the fixture began says it has no visits. A day inside the
 *   fixture with no visits reopened no pages.
 *   The day's top site leads to History filtered to that site and day.
 * - Insights' Top sites row opens the site view, whose 30-day visit total,
 *   days visited and most visited page equal the fixture's.
 * - A Frequent searches chip opens the search view: how often it was
 *   searched in all and when last equal the fixture; its days lead to the
 *   day view, and back from the day to Insights in the range it came from.
 * - A "Pages you keep reopening" row opens the page view, whose visit total
 *   across browsers equals the fixture's.
 * - A site's view says how its visits from elsewhere began (typed, from a
 *   search, from a link), which sites linked to it and which it led on to,
 *   as the fixture's browsers recorded each page's opener.
 * - Google's site view lists what was searched on it, as Frequent searches
 *   counts it (every fixture search is on Google).
 * - Two sites on github.io are two sites, as in a browser: each site view
 *   counts only its own visits and pages.
 * - A Chinese search, with no spaces between its words, is one search with a
 *   longer wording of it: its view lists that wording among the others.
 * - A search that went back and forth between one product on two shops is
 *   the one set on its day's "Pages you compared": both pages, most opened
 *   first, how often each was opened and which the search opened first; the
 *   set leads to the page's view and the search's.
 * - A session is named after its main site and the search it made most
 *   often, as last typed: not its first search, and a tie goes to the
 *   search made first.
 *
 * Insights are computed by a background job after the first backup, so
 * every read waits for real numbers instead of reading a "computing" state.
 */
import { expect, test, type Locator, type Page } from '@playwright/test'
import {
  FIXTURE_COMPARE,
  FIXTURE_PAGES_SITE,
  FIXTURE_SESSION_TITLE,
  openedFrom,
} from '../../scripts/fixtures/synthetic-browsers.mjs'
import {
  archivedProfiles,
  archivedVisits,
  expectCount,
  fixture,
  parseCount,
} from './support/fixture'
import { siteOf } from './support/public-suffix'
import { searchFamilies } from './support/search-words'

const JOB_TIMEOUT = 60_000

function localDateKey(ms: number) {
  const date = new Date(ms)
  const pad = (value: number) => String(value).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`
}

function startOfDayDaysAgo(days: number) {
  const date = new Date()
  date.setHours(0, 0, 0, 0)
  date.setDate(date.getDate() - days)
  return date.getTime()
}

function countBy<T>(items: T[], keyOf: (item: T) => string) {
  const counts = new Map<string, number>()
  for (const item of items) {
    const key = keyOf(item)
    counts.set(key, (counts.get(key) ?? 0) + 1)
  }
  return counts
}

function leaders(counts: Map<string, number>) {
  const top = Math.max(...counts.values())
  return {
    top,
    keys: [...counts].filter(([, count]) => count === top).map(([key]) => key),
  }
}

/** The number a KPI tile shows, once it has loaded. */
async function kpi(page: Page, label: string) {
  const tile = page.getByRole('group', { name: label, exact: true })
  await expect(tile).toHaveAttribute('aria-busy', 'false', {
    timeout: JOB_TIMEOUT,
  })
  const lines = (await tile.innerText()).split('\n').map((line) => line.trim())
  return parseCount(lines[1] ?? '')
}

/**
 * Waits until the drill-in itself is on screen. Route chunks load lazily and
 * the previous screen stays up meanwhile, so the URL changes first.
 */
async function drillIn(page: Page, title: string | RegExp) {
  await expect(page.getByRole('heading', { level: 1 })).toHaveText(title, {
    timeout: JOB_TIMEOUT,
  })
}

const card = (page: Page, title: string): Locator =>
  page
    .getByRole('heading', { name: title, exact: true })
    .locator('xpath=ancestor::section[1]')

test('a day from the Home heatmap matches the fixture, then opens History', async ({
  page,
}, testInfo) => {
  const dayMs = startOfDayDaysAgo(3)
  const day = localDateKey(dayMs)
  const visits = archivedVisits().filter(
    (visit) => localDateKey(visit.at) === day,
  )
  expect(visits.length, 'the fixture has visits on that day').toBeGreaterThan(0)
  // Search result pages are searches, not sites (as on Insights' Top sites).
  const sites = leaders(
    countBy(
      visits.filter((visit) => !visit.term),
      (visit) => siteOf(visit.url),
    ),
  )
  const terms = new Set(
    visits
      .filter((visit) => visit.term)
      .map((visit) => visit.term!.toLowerCase()),
  )

  // A day before the fixture began has nothing, and says so.
  await page.goto('/#/insights/day/2020-01-15')
  await drillIn(page, 'Wednesday, January 15')
  // Each card that lists the day's visits says so; Top sites stands for them.
  await expect(
    card(page, 'Top sites').getByText('No visits on this day.'),
  ).toBeVisible({
    timeout: JOB_TIMEOUT,
  })
  await expectCount(
    testInfo,
    'visits before the fixture began',
    await kpi(page, 'Page visits'),
    0,
  )

  // A day inside the fixture with no visits: nothing was reopened on it,
  // though pages opened before and after it span it (it once said 58).
  const visited = new Set(
    archivedVisits().map((visit) => localDateKey(visit.at)),
  )
  const latest = Math.max(...archivedVisits().map((visit) => visit.at))
  const quiet = new Date(latest)
  quiet.setHours(0, 0, 0, 0)
  while (visited.has(localDateKey(quiet.getTime()))) {
    quiet.setDate(quiet.getDate() - 1)
  }
  await page.goto(`/#/insights/day/${localDateKey(quiet.getTime())}`)
  await drillIn(
    page,
    quiet.toLocaleDateString('en-US', {
      weekday: 'long',
      month: 'long',
      day: 'numeric',
    }),
  )
  await expectCount(
    testInfo,
    `pages reopened on ${localDateKey(quiet.getTime())}, a day without visits`,
    await kpi(page, 'Pages reopened'),
    0,
  )

  await page.goto('/#/')
  await page.locator(`[data-cell="${day}"]`).click()
  await expect(page).toHaveURL(new RegExp(`#/insights/day/${day}$`))
  await drillIn(
    page,
    new Date(dayMs).toLocaleDateString('en-US', {
      weekday: 'long',
      month: 'long',
      day: 'numeric',
    }),
  )

  await expectCount(
    testInfo,
    `visits on ${day}`,
    await kpi(page, 'Page visits'),
    visits.length,
  )
  await expectCount(
    testInfo,
    `searches on ${day}`,
    await kpi(page, 'Searches'),
    visits.filter((visit) => visit.term).length,
  )

  const firstSite = card(page, 'Top sites').getByRole('listitem').first()
  await expect(firstSite).toBeVisible({ timeout: JOB_TIMEOUT })
  const siteText = (await firstSite.innerText()).replace(/\s+/g, ' ').trim()
  await expectCount(
    testInfo,
    `visits to the top site on ${day}`,
    parseCount(siteText.match(/(\d[\d,]*)$/)?.[1] ?? ''),
    sites.top,
  )
  const topSite = sites.keys.find((site) => siteText.includes(site))
  expect(
    topSite,
    `${siteText} should name one of ${sites.keys.join(', ')}`,
  ).toBeTruthy()

  // One row per distinct search that day, each the query as typed.
  const searchRows = card(page, 'Searches').getByRole('listitem')
  await expect(searchRows).toHaveCount(terms.size)
  // By exact text: a reworded search ("… v2") contains the one before it.
  for (const term of terms) {
    await expect(
      searchRows.filter({ has: page.getByText(term, { exact: true }) }),
    ).toHaveCount(1)
  }

  // Pages you came back to: each one was opened that day (the read once took
  // every re-found page whose first and last visit spanned the day).
  const titles = new Set(visits.map((visit) => visit.title))
  const reopened = card(page, 'Pages you came back to').getByRole('listitem')
  await expect(reopened.first()).toBeVisible({ timeout: JOB_TIMEOUT })
  for (const row of await reopened.all()) {
    const title = (await row.innerText()).split('\n')[0].trim()
    expect(titles.has(title), `${title} was opened on ${day}`).toBe(true)
  }
  // The KPI counts the same pages; the card shows the first five.
  await expectCount(
    testInfo,
    `pages reopened on ${day}, as the card lists them`,
    Math.min(await kpi(page, 'Pages reopened'), 5),
    await reopened.count(),
  )

  // The top site leads to History, filtered to that site on that day.
  await firstSite.getByRole('link').click()
  await expect(page).toHaveURL(
    new RegExp(
      `#/history\\?date=${day}&domain=${topSite!.replace('.', '\\.')}$`,
    ),
  )
  // History's day header counts that site's visits that day.
  await expect(
    page.getByText(`${sites.top} visits`, { exact: true }),
  ).toBeVisible({
    timeout: JOB_TIMEOUT,
  })
})

test('a site from Top sites shows its 30-day visits', async ({
  page,
}, testInfo) => {
  const since = startOfDayDaysAgo(29)
  const recent = archivedVisits().filter(
    (visit) => visit.at >= since && !visit.term,
  )
  const top = leaders(countBy(recent, (visit) => siteOf(visit.url)))

  await page.goto('/#/insights?range=d30')
  const firstRow = card(page, 'Top sites').getByRole('listitem').first()
  await expect(firstRow).toBeVisible({ timeout: JOB_TIMEOUT })
  const rowText = (await firstRow.innerText()).replace(/\s+/g, ' ').trim()
  const site = top.keys.find((key) => rowText.includes(key))
  expect(
    site,
    `${rowText} should name one of ${top.keys.join(', ')}`,
  ).toBeTruthy()

  await firstRow.getByRole('link').click()
  await expect(page).toHaveURL(
    new RegExp(`#/insights/site/${site!.replace('.', '\\.')}\\?range=d30$`),
  )
  await drillIn(page, site!)

  const onSite = archivedVisits().filter(
    (visit) => visit.at >= since && siteOf(visit.url) === site,
  )
  await expectCount(
    testInfo,
    `${site} visits, last 30 days`,
    await kpi(page, 'Page visits'),
    onSite.length,
  )
  await expectCount(
    testInfo,
    `${site} days visited, last 30 days`,
    await kpi(page, 'Days visited'),
    new Set(onSite.map((visit) => localDateKey(visit.at))).size,
  )
  await expectCount(
    testInfo,
    `${site} pages, last 30 days`,
    await kpi(page, 'Pages'),
    new Set(onSite.map((visit) => visit.url)).size,
  )

  const pages = leaders(countBy(onSite, (visit) => visit.url))
  const firstPage = card(page, 'Most visited pages')
    .getByRole('listitem')
    .first()
  await expect(firstPage).toBeVisible()
  await expectCount(
    testInfo,
    `${site} most visited page, last 30 days`,
    parseCount(
      (await firstPage.innerText()).match(/(\d[\d,]*)\s*$/)?.[1] ?? '',
    ),
    pages.top,
  )
})

/**
 * How each site's visits began, counted by hand as the fixture's browsers
 * record openers (`openedFrom`): a page that was not typed or searched was
 * opened from the visit before it. A visit counts when its opener was not on
 * the same site: none means typed, a search's results a search, any other
 * page a link, and that page's site gains an onward site.
 */
function siteFlows() {
  type Flows = {
    direct: number
    search: number
    link: number
    referrers: Map<string, number>
    onward: Map<string, number>
  }
  const flows = new Map<string, Flows>()
  const of = (site: string) => {
    let entry = flows.get(site)
    if (!entry) {
      entry = {
        direct: 0,
        search: 0,
        link: 0,
        referrers: new Map(),
        onward: new Map(),
      }
      flows.set(site, entry)
    }
    return entry
  }
  const bump = (counts: Map<string, number>, key: string) =>
    counts.set(key, (counts.get(key) ?? 0) + 1)
  for (const profile of archivedProfiles) {
    const visits = fixture().visitsByProfile[profile]
    const from = openedFrom(visits)
    for (const [index, visit] of visits.entries()) {
      const site = siteOf(visit.url)
      const opener = from[index] >= 0 ? visits[from[index]] : null
      if (!opener) {
        of(site).direct += 1
        continue
      }
      const openerSite = siteOf(opener.url)
      if (openerSite === site) continue
      if (opener.term) of(site).search += 1
      else {
        of(site).link += 1
        bump(of(site).referrers, openerSite)
      }
      bump(of(openerSite).onward, site)
    }
  }
  return flows
}

test('a site says how its visits began and where its links led', async ({
  page,
}, testInfo) => {
  const flows = siteFlows()
  // The site with the most visits from elsewhere among those reached all three ways.
  const [site, expected] = [...flows]
    .filter(([, f]) => f.direct > 0 && f.search > 0 && f.link > 0)
    .sort(
      ([, a], [, b]) =>
        b.direct + b.search + b.link - (a.direct + a.search + a.link),
    )[0]
  await page.goto(`/#/insights/site/${site}`)
  await drillIn(page, site)

  const arrivals = card(page, 'How you got here')
  for (const [label, count] of [
    ['Typed or bookmarked', expected.direct],
    ['From a search', expected.search],
    ['From a link', expected.link],
  ] as const) {
    const row = arrivals.getByRole('listitem').filter({ hasText: label })
    await expect(row).toBeVisible({ timeout: JOB_TIMEOUT })
    const text = (await row.innerText()).replace(/\s+/g, ' ')
    await expectCount(
      testInfo,
      `${site}: ${label.toLowerCase()}, all time`,
      parseCount(text.match(new RegExp(`${label} (\\d[\\d,]*)`))?.[1] ?? ''),
      count,
    )
  }
  // The site that linked here most, and the one this site led to most.
  for (const [title, counts] of [
    ['How you got here', expected.referrers],
    ['Where you went next', expected.onward],
  ] as const) {
    const top = leaders(counts)
    const first = card(page, title).getByRole('link').first()
    await expect(first).toBeVisible({ timeout: JOB_TIMEOUT })
    const text = (await first.innerText()).replace(/\s+/g, ' ').trim()
    expect(
      // A row may start with the site's letter badge.
      top.keys.some((key) => text.includes(key)),
      `${title}: ${text} should be one of ${top.keys.join(', ')}`,
    ).toBe(true)
    await expectCount(
      testInfo,
      `${site}: ${title.toLowerCase()}, the first site`,
      parseCount(text.match(/(\d[\d,]*)$/)?.[1] ?? ''),
      top.top,
    )
  }
})

test('the site of a search engine lists what was searched on it', async ({
  page,
}) => {
  // The query a search link opens, and the count at the end of its text.
  const read = async (link: Locator) => {
    const href = (await link.getAttribute('href')) ?? ''
    const query = decodeURIComponent(href.match(/search\/([^?]+)/)?.[1] ?? '')
    const text = (await link.innerText()).replace(/\s+/g, ' ').trim()
    return { query, count: parseCount(text.match(/(\d[\d,]*)\D*$/)?.[1] ?? '') }
  }
  await page.goto('/#/insights')
  const frequent = card(page, 'Frequent searches')
    .locator('a[href*="/insights/search/"]')
    .first()
  await expect(frequent).toBeVisible({ timeout: JOB_TIMEOUT })
  const expected = await read(frequent)

  await page.goto('/#/insights/site/google.com')
  await drillIn(page, 'google.com')
  const first = card(page, 'Searched on this site')
    .locator('a[href*="/insights/search/"]')
    .first()
  await expect(first).toBeVisible({ timeout: JOB_TIMEOUT })
  expect(await read(first)).toEqual(expected)
})

test('two sites on github.io are two sites, as in a browser', async ({
  page,
}, testInfo) => {
  const sites = ['rust-lang.github.io', FIXTURE_PAGES_SITE.site]
  for (const site of sites) {
    const visits = archivedVisits().filter(
      (visit) => !visit.term && siteOf(visit.url) === site,
    )
    expect(visits.length, `the fixture visited ${site}`).toBeGreaterThan(0)
    await page.goto(`/#/insights/site/${site}`)
    await drillIn(page, site)
    await expectCount(
      testInfo,
      `${site} visits, all time`,
      await kpi(page, 'Page visits'),
      visits.length,
    )
    await expectCount(
      testInfo,
      `${site} pages, all time`,
      await kpi(page, 'Pages'),
      new Set(visits.map((visit) => visit.url)).size,
    )
  }
})

test('a Chinese search and a longer wording of it are one search', async ({
  page,
}) => {
  // 台北天氣 and 台北天氣預報 share 台北, 北天 and 天氣 compared by pairs of
  // characters; split only at spaces, each was one word and they had nothing
  // in common.
  await page.goto(`/#/insights/search/${encodeURIComponent('台北天氣')}`)
  await drillIn(page, '“台北天氣”')
  await expect(card(page, 'Other ways you searched it')).toContainText(
    '台北天氣預報',
    { timeout: JOB_TIMEOUT },
  )
})

test('a search that went back and forth between two shops is a set of compared pages on its day', async ({
  page,
}, testInfo) => {
  const { term, pages, order } = FIXTURE_COMPARE
  const search = archivedVisits().find((visit) => visit.term === term)
  expect(search, 'the fixture made the search').toBeTruthy()
  // By hand: how often the search opened each page, most first; the first
  // page it opened is where it landed.
  const opened = countBy(
    order.map((index) => pages[index]),
    ([url]) => url,
  )
  const expected = [...opened]
    .sort((a, b) => b[1] - a[1])
    .map(([url, times]) => ({
      url,
      title: pages.find(([candidate]) => candidate === url)![1],
      times,
      landing: url === pages[order[0]][0],
    }))
  const sites = new Set(pages.map(([url]) => siteOf(url))).size
  await testInfo.attach('compared pages', {
    body: JSON.stringify(expected, null, 2),
    contentType: 'application/json',
  })

  await page.goto(`/#/insights/day/${localDateKey(search!.at)}`)
  const sets = card(page, 'Pages you compared').getByRole('listitem')
  await expect(sets).toHaveCount(1, { timeout: JOB_TIMEOUT })
  const set = sets.first()
  await expect(set.getByRole('link', { name: term, exact: true })).toBeVisible()
  await expect(set).toContainText(`${expected.length} pages on ${sites} sites`)
  const chips = set.locator('a[href*="/insights/page/"]')
  await expect(chips).toHaveCount(expected.length)
  for (const [index, want] of expected.entries()) {
    const chip = chips.nth(index)
    await expect(chip).toContainText(want.title)
    await expect(chip).toContainText(`${want.times}×`)
    if (want.landing)
      await expect(chip).toHaveAttribute(
        'title',
        'Opened first from the search',
      )
    else await expect(chip).not.toHaveAttribute('title', /./)
  }

  // The page the search opened most leads to its own view, with its visits.
  await chips.first().click()
  await drillIn(page, expected[0].title)
  await expectCount(
    testInfo,
    `visits to ${expected[0].url}`,
    await kpi(page, 'Visits'),
    archivedVisits().filter((visit) => visit.url === expected[0].url).length,
  )
  await page.goBack()
  await set.getByRole('link', { name: term, exact: true }).click()
  await drillIn(page, `“${term}”`)
})

test('a session is named after its main site and its most-used search', async ({
  page,
}, testInfo) => {
  const { steps, pages } = FIXTURE_SESSION_TITLE
  const terms = steps.flatMap((step) => ('term' in step ? [step.term] : []))
  const start = archivedVisits().find((visit) => visit.term === terms[0])
  expect(start, 'the fixture made the session').toBeTruthy()
  const day = localDateKey(start!.at)
  expect(
    archivedVisits().filter((visit) => localDateKey(visit.at) === day),
    'the session is alone on its day',
  ).toHaveLength(steps.length)
  // By hand: "write ahead checkpoint" and "busy timeout retry" were each
  // searched twice, "btree page layout" once. The tie goes to the one
  // searched first, shown as last typed. The pages, all on sqlite.org,
  // outnumber the searches, so that site comes first.
  const counts = countBy(terms, (term) => term.toLowerCase())
  expect(counts.get('write ahead checkpoint')).toBe(2)
  expect(counts.get('busy timeout retry')).toBe(2)
  expect(steps.length - terms.length).toBeGreaterThan(terms.length)
  expect(new Set(pages.map(([url]) => siteOf(url)))).toEqual(
    new Set(['sqlite.org']),
  )
  const title = 'SQLite · write ahead checkpoint'
  await testInfo.attach('session', {
    body: JSON.stringify({ day, terms, title }, null, 2),
    contentType: 'application/json',
  })

  await page.goto(`/#/insights/day/${day}`)
  const sessions = card(page, 'Sessions').getByRole('listitem')
  await expect(sessions).toHaveCount(1, { timeout: JOB_TIMEOUT })
  await expect(sessions.first()).toContainText(title)
  await expect(sessions.first()).toContainText(
    `${steps.length} pages · 2 sites · ${terms.length} searches`,
  )
  await testInfo.attach('the session on its day', {
    body: await sessions.first().screenshot(),
    contentType: 'image/png',
  })
})

test('a search from Frequent searches shows how often and when', async ({
  page,
}, testInfo) => {
  await page.goto('/#/insights?range=d30')
  const chip = card(page, 'Frequent searches')
    .locator('a[href*="/insights/search/"]')
    .first()
  await expect(chip).toBeVisible({ timeout: JOB_TIMEOUT })
  await chip.click()
  await expect(page).toHaveURL(/#\/insights\/search\/[^?]+\?range=d30$/)
  const query = decodeURIComponent(
    new URL(page.url()).hash.match(/search\/([^?]+)/)?.[1] ?? '',
  )
  // The page counts the search's family in every profile, made by hand with
  // the backend's rule (the fixture rewords some searches as "… v2", "… v2 eg").
  const families = archivedProfiles.flatMap((profile) =>
    searchFamilies(fixture().visitsByProfile[profile]).filter((family) =>
      family.queries.has(query.toLowerCase()),
    ),
  )
  const timesSearched = families.reduce(
    (sum, family) => sum + family.members,
    0,
  )
  expect(timesSearched, `the fixture searched "${query}"`).toBeGreaterThan(0)

  await drillIn(page, `“${query}”`)
  await expectCount(
    testInfo,
    `times "${query}" was searched`,
    await kpi(page, 'Times searched'),
    timesSearched,
  )

  // The newest time in the list is the fixture's newest search, on its day.
  const latest = Math.max(...families.map((family) => family.latest))
  const times = card(page, 'Each time, and where it led').getByRole('listitem')
  await expect(times.first()).toBeVisible({ timeout: JOB_TIMEOUT })
  // The day keeps the range it was opened from, for the way back.
  const dayLink = times.first().getByRole('link').first()
  await expect(dayLink).toHaveAttribute(
    'href',
    `#/insights/day/${localDateKey(latest)}?range=d30`,
  )
  await dayLink.click()
  await expect(page).toHaveURL(
    new RegExp(`#/insights/day/${localDateKey(latest)}\\?range=d30$`),
  )
  await drillIn(
    page,
    new Date(latest).toLocaleDateString('en-US', {
      weekday: 'long',
      month: 'long',
      day: 'numeric',
    }),
  )
  // One row per wording; this one's row by its exact text.
  await expect(
    card(page, 'Searches')
      .getByRole('listitem')
      .filter({ has: page.getByText(query, { exact: true }) }),
  ).toHaveCount(1, { timeout: JOB_TIMEOUT })

  await page.getByRole('main').getByRole('link', { name: 'Insights' }).click()
  await expect(page).toHaveURL(/#\/insights\?range=d30$/)
  await expect(
    page
      .getByRole('group', { name: 'Time range' })
      .getByRole('radio', { name: '30d' }),
  ).toBeChecked()
})

test('a page you keep reopening shows its visits across browsers', async ({
  page,
}, testInfo) => {
  await page.goto('/#/insights?range=d30')
  // "Pages you keep reopening" is the second list in the searches card.
  const pageRow = page
    .getByRole('heading', { name: 'Pages you keep reopening' })
    .locator('xpath=ancestor::section[1]')
    .getByRole('listitem')
    .first()
  await expect(pageRow).toBeVisible({ timeout: JOB_TIMEOUT })
  const title = (await pageRow.locator('.truncate').first().innerText()).trim()
  await pageRow.getByRole('link').click()
  await expect(page).toHaveURL(/#\/insights\/page\/[^?]+\?.*range=d30/)
  await drillIn(page, title)
  const canonical = decodeURIComponent(
    new URL(page.url()).hash.match(/page\/([^?]+)/)?.[1] ?? '',
  )
  const visits = archivedVisits().filter(
    (visit) => visit.url.replace(/\/$/, '') === canonical.replace(/\/$/, ''),
  )
  expect(visits.length, `the fixture visited ${canonical}`).toBeGreaterThan(1)
  await expectCount(
    testInfo,
    `visits to ${canonical}`,
    await kpi(page, 'Visits'),
    visits.length,
  )
  await expect(page.getByText('Why it is listed')).toBeVisible()
  const why = card(page, 'Why it is listed')
  await expect(why.getByText(/Opened on \d+ different days/)).toBeVisible({
    timeout: JOB_TIMEOUT,
  })
  // Opened directly (typed, a bookmark: nothing opened it), in the profile
  // the score is counted in. Chrome and Firefox record "nothing" as 0.
  const scores = (new URLSearchParams(
    new URL(page.url()).hash.split('?')[1] ?? '',
  ).get('scores') ?? '') as (typeof archivedProfiles)[number]
  const inProfile = fixture().visitsByProfile[scores]
  expect(inProfile, `${scores} is an archived profile`).toBeTruthy()
  const from = openedFrom(inProfile)
  const direct = inProfile.filter(
    (visit, index) =>
      visit.url.replace(/\/$/, '') === canonical.replace(/\/$/, '') &&
      from[index] < 0 &&
      !visit.term,
  ).length
  if (direct > 0) {
    const factor = why.getByText(/Opened directly \d+ times?/)
    await expect(factor).toBeVisible()
    await expectCount(
      testInfo,
      `${canonical} opened directly in ${scores}`,
      parseCount((await factor.innerText()).match(/(\d[\d,]*)/)?.[1] ?? ''),
      direct,
    )
  }
})
