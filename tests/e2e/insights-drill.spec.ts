/**
 * Insights drill-ins: a day, a site, a search and a page, reached the way a
 * user reaches them, show the archive's numbers, and lead on to History.
 *
 * Proves:
 * - Home's year heatmap opens the day view; for a fixture day its visit
 *   count, top site and searches equal a count of the fixture for that
 *   local day, and a day before the fixture began says it has no visits.
 *   The day's top site leads to History filtered to that site and day.
 * - Insights' Top sites row opens the site view, whose 30-day visit total,
 *   days visited and most visited page equal the fixture's.
 * - A Frequent searches chip opens the search view: how often it was
 *   searched in all and when last equal the fixture; its days lead to the
 *   day view.
 * - A "Pages you keep reopening" row opens the page view, whose visit total
 *   across browsers equals the fixture's.
 *
 * Insights are computed by a background job after the first backup, so
 * every read waits for real numbers instead of reading a "computing" state.
 */
import { expect, test, type Locator, type Page } from '@playwright/test'
import {
  archivedVisits,
  expectCount,
  parseCount,
  type FixtureVisit,
} from './support/fixture'

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

/** The registrable domain of a fixture URL (every fixture host is name.tld or sub.name.tld). */
function siteOf(url: string) {
  return new URL(url).hostname.split('.').slice(-2).join('.')
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
  await expect(page.getByText('No visits on this day.')).toBeVisible({
    timeout: JOB_TIMEOUT,
  })
  await expectCount(
    testInfo,
    'visits before the fixture began',
    await kpi(page, 'Page visits'),
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
  for (const term of terms) {
    await expect(searchRows.filter({ hasText: term })).toHaveCount(1)
  }

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
  const searches = archivedVisits().filter(
    (visit: FixtureVisit) => visit.term?.toLowerCase() === query.toLowerCase(),
  )
  expect(searches.length, `the fixture searched "${query}"`).toBeGreaterThan(0)

  await drillIn(page, `“${query}”`)
  await expectCount(
    testInfo,
    `times "${query}" was searched`,
    await kpi(page, 'Times searched'),
    searches.length,
  )

  // The newest time in the list is the fixture's newest search, on its day.
  const latest = Math.max(...searches.map((visit) => visit.at))
  const times = card(page, 'Each time, and where it led').getByRole('listitem')
  await expect(times.first()).toBeVisible({ timeout: JOB_TIMEOUT })
  const dayLink = times.first().getByRole('link').first()
  await expect(dayLink).toHaveAttribute(
    'href',
    `#/insights/day/${localDateKey(latest)}`,
  )
  await dayLink.click()
  await expect(page).toHaveURL(
    new RegExp(`#/insights/day/${localDateKey(latest)}$`),
  )
  await drillIn(
    page,
    new Date(latest).toLocaleDateString('en-US', {
      weekday: 'long',
      month: 'long',
      day: 'numeric',
    }),
  )
  await expect(
    card(page, 'Searches').getByRole('listitem').filter({ hasText: query }),
  ).toHaveCount(1, { timeout: JOB_TIMEOUT })
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
  await expect(
    card(page, 'Why it is listed').getByText(/Opened on \d+ different days/),
  ).toBeVisible({ timeout: JOB_TIMEOUT })
})
