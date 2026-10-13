/**
 * Insights counts what the fixture holds, however a scope cuts it: one
 * browser profile, a range of days, all of them.
 *
 * Proves:
 * - Frequent searches, last 30 days: each search counted once however it
 *   was typed (letter case and spacing apart) and shown the way it was last
 *   typed, most searched first; an
 *   address typed into the search box
 *   ("news.ycombinator.com") is a way to reach a site, not a search, and is
 *   left out though it was typed more often than most.
 * - New sites: a site counts on the first day it was visited in the scope.
 *   One first visited in Personal and weeks later in Work is new on its
 *   Work day for Work, and for all profiles only once, on its Personal day.
 * - The rhythm heatmap: each weekday-and-hour cell holds the visits made in
 *   that hour of that weekday, for all profiles, one profile, and a week.
 *
 * Nothing here changes the archive. Days and hours are the app's, which runs
 * in UTC in E2E, as these specs do.
 */
import { expect, test, type Page, type TestInfo } from '@playwright/test'
import { FIXTURE_SCENES } from '../../scripts/fixtures/synthetic-browsers.mjs'
import {
  archivedProfiles,
  archivedVisits,
  expectCount,
  fixture,
  localDay,
  parseCount,
  type FixtureVisit,
  type ProfileId,
} from './support/fixture'
import { siteOf } from './support/public-suffix'
import { isAddress } from './support/search-words'

const JOB_TIMEOUT = 60_000
const DAY = 86_400_000

const card = (page: Page, title: string) =>
  page
    .getByRole('heading', { name: title, exact: true })
    .locator('xpath=ancestor::section[1]')

/** Local midnight `days` days before today. */
function startOfDayDaysAgo(days: number) {
  const date = new Date()
  date.setHours(0, 0, 0, 0)
  date.setDate(date.getDate() - days)
  return date.getTime()
}

/** The local day `days` days before the fixture's `now`, as the scenes count them. */
const sceneDay = (days: number) => localDay(fixture().now - days * DAY)

/** Insights for a range of days, all profiles or one. */
function insightsUrl(first: string, last: string, profile?: ProfileId) {
  const scope = profile ? `&profile=${encodeURIComponent(profile)}` : ''
  return `/#/insights?range=custom&start=${first}&end=${last}${scope}`
}

/** A KPI tile's number, once it has loaded. */
async function kpi(page: Page, label: string) {
  const tile = page.getByRole('group', { name: label, exact: true })
  await expect(tile).toHaveAttribute('aria-busy', 'false', {
    timeout: JOB_TIMEOUT,
  })
  return parseCount((await tile.innerText()).split('\n')[1] ?? '')
}

/** A search as typed, with runs of spaces made one, as Insights shows it. */
const typed = (term: string) => term.trim().split(/\s+/).join(' ')

/** How Insights groups searches: letter case and runs of spaces don't count. */
const normalized = (term: string) => typed(term).toLowerCase()

test('Frequent searches count a search however it was typed, and leave out typed addresses', async ({
  page,
}, testInfo) => {
  const since = startOfDayDaysAgo(29)
  // Per grouped search: how often, and the spelling of its latest visit.
  const counts = new Map<string, { count: number; at: number; query: string }>()
  let addresses = 0
  for (const visit of archivedVisits()) {
    if (!visit.term || visit.at < since) continue
    if (isAddress(visit.term)) addresses += 1
    else {
      const key = normalized(visit.term)
      const entry = counts.get(key) ?? { count: 0, at: -1, query: '' }
      entry.count += 1
      if (visit.at > entry.at) {
        entry.at = visit.at
        entry.query = typed(visit.term)
      }
      counts.set(key, entry)
    }
  }
  // Most searched first, then by the grouped spelling (as the backend sorts).
  const expected = [...counts]
    .sort(([a, x], [b, y]) => y.count - x.count || (a < b ? -1 : 1))
    .slice(0, 10)
    .map(([, { query, count }]) => ({ query, count }))
  await testInfo.attach('frequent searches by hand, 30 days', {
    body: JSON.stringify({ expected, addresses }, null, 2),
    contentType: 'application/json',
  })
  // The fixture says what these prove: the search typed three ways is listed
  // once with all of them, and the address would be listed if it counted.
  const { spellings } = FIXTURE_SCENES.respelled
  expect(new Set(spellings).size).toBe(spellings.length)
  expect(new Set(spellings.map(normalized)).size).toBe(1)
  // Shown as last typed: not the first spelling, not lowercased.
  expect(expected.map((entry) => entry.query)).toContain(spellings.at(-1))
  expect(spellings.at(-1)).not.toBe(normalized(spellings.at(-1)!))
  expect(addresses).toBeGreaterThan(expected.at(-1)!.count)

  await page.goto('/#/insights?range=d30')
  const chips = card(page, 'Frequent searches').locator(
    'a[href*="/insights/search/"]',
  )
  await expect(chips.first()).toBeVisible({ timeout: JOB_TIMEOUT })
  // Each chip's words as shown, and the count at the end of its text.
  const read = async () =>
    Promise.all(
      (await chips.all()).map(async (chip) => {
        const text = (await chip.innerText()).trim()
        const count = text.match(/(\d[\d,]*)\D*$/)
        return {
          query: typed(count ? text.slice(0, count.index) : text),
          count: parseCount(count?.[1] ?? ''),
        }
      }),
    )
  let shown: Awaited<ReturnType<typeof read>> = []
  await expect
    .poll(async () => (shown = await read()), { timeout: JOB_TIMEOUT })
    .toEqual(expected)
  for (const chip of await chips.all())
    await expect(chip).not.toContainText(FIXTURE_SCENES.addressSearch.term)
  await testInfo.attach('frequent searches shown', {
    body: JSON.stringify(shown, null, 2),
    contentType: 'application/json',
  })
})

/** Sites whose first visit in `visits` falls between two local days, inclusive. */
function newSites(visits: FixtureVisit[], first: string, last: string) {
  const firstSeen = new Map<string, number>()
  for (const visit of visits) {
    const site = siteOf(visit.url)
    const at = firstSeen.get(site)
    if (at === undefined || visit.at < at) firstSeen.set(site, visit.at)
  }
  return [...firstSeen].filter(
    ([, at]) => localDay(at) >= first && localDay(at) <= last,
  )
}

test('a site is new on its first day in the scope: per profile, and once for all profiles', async ({
  page,
}, testInfo) => {
  const { url, visits } = FIXTURE_SCENES.newSite
  const site = siteOf(url)
  const [[personal, firstDay], [work, workDay]] = visits
  expect([personal, work]).toEqual(['chrome:Default', 'chrome:Profile 1'])
  const personalDay = sceneDay(firstDay[0])
  const workFirstDay = sceneDay(workDay[0])
  const ranges = [
    // Only the day Work first went there.
    [workFirstDay, workFirstDay],
    // Only the day Personal first went there.
    [personalDay, personalDay],
    // Both.
    [personalDay, workFirstDay],
  ] as const
  const scopes: [string, FixtureVisit[], ProfileId | undefined][] = [
    ['all profiles', archivedVisits(), undefined],
    ...archivedProfiles.map(
      (profile) =>
        [profile, fixture().visitsByProfile[profile], profile] as [
          string,
          FixtureVisit[],
          ProfileId,
        ],
    ),
  ]
  const isNew = (visits: FixtureVisit[], [first, last]: readonly string[]) =>
    newSites(visits, first, last).some(([name]) => name === site)
  // What the scene means: new for Work on its day, not for all profiles.
  expect(isNew(fixture().visitsByProfile[work], ranges[0])).toBe(true)
  expect(isNew(archivedVisits(), ranges[0])).toBe(false)
  expect(isNew(archivedVisits(), ranges[1])).toBe(true)
  expect(isNew(archivedVisits(), ranges[2])).toBe(true)

  for (const [name, scopeVisits, profile] of scopes) {
    for (const [first, last] of ranges) {
      const want = newSites(scopeVisits, first, last)
      await page.goto('about:blank')
      await page.goto(insightsUrl(first, last, profile))
      await expectCount(
        testInfo,
        `new sites, ${name}, ${first}..${last}: ${want.map(([host]) => host).join(', ')}`,
        await kpi(page, 'New sites'),
        want.length,
      )
    }
  }
})

/** Visits per `weekday-hour` (0 = Sunday), as the heatmap keys its cells. */
function perWeekdayHour(visits: FixtureVisit[]) {
  const cells = new Map<string, number>()
  for (const visit of visits) {
    const at = new Date(visit.at)
    const key = `${at.getDay()}-${at.getHours()}`
    cells.set(key, (cells.get(key) ?? 0) + 1)
  }
  return cells
}

/**
 * Waits until every one of the heatmap's 168 cells holds what the fixture
 * gives it, and records both: the previous scope's grid stays up until the
 * new one arrives.
 */
async function expectRhythm(
  page: Page,
  testInfo: TestInfo,
  name: string,
  visits: FixtureVisit[],
) {
  const expected = perWeekdayHour(visits)
  const cells = card(page, 'Rhythm')
    .getByRole('group', { name: 'Visits by hour of the day, across the week' })
    .locator('[data-cell]')
  await expect(cells).toHaveCount(168, { timeout: JOB_TIMEOUT })
  let wrong: { cell: string; expected: number; shown: number }[] = []
  await expect
    .poll(
      async () => {
        const shown = await cells.evaluateAll((list) =>
          list.map((cell) => [
            cell.getAttribute('data-cell') ?? '',
            cell.getAttribute('data-label') ?? '',
          ]),
        )
        wrong = shown.flatMap(([key, label]) => {
          const count = Number(
            (label.match(/· ([\d,]+) visits?$/)?.[1] ?? 'NaN').replace(
              /,/g,
              '',
            ),
          )
          const want = expected.get(key) ?? 0
          return count === want
            ? []
            : [{ cell: key, expected: want, shown: count }]
        })
        return wrong.length
      },
      { message: `rhythm, ${name}`, timeout: JOB_TIMEOUT },
    )
    .toBe(0)
  await testInfo.attach(`rhythm, ${name}`, {
    body: JSON.stringify(
      { visits: visits.length, cells: Object.fromEntries(expected), wrong },
      null,
      2,
    ),
    contentType: 'application/json',
  })
}

test('the rhythm heatmap holds each weekday-hour’s visits, for all profiles, one profile and a week', async ({
  page,
}, testInfo) => {
  test.setTimeout(180_000)
  await page.goto('/#/insights')
  await expectRhythm(page, testInfo, 'all profiles, all time', archivedVisits())

  const work: ProfileId = 'chrome:Profile 1'
  await page.goto('about:blank')
  await page.goto(`/#/insights?profile=${encodeURIComponent(work)}`)
  await expectRhythm(
    page,
    testInfo,
    'Work, all time',
    fixture().visitsByProfile[work],
  )

  // A week with the scenes' early-morning visits in it.
  const [first, last] = [sceneDay(13), sceneDay(7)]
  await page.goto('about:blank')
  await page.goto(insightsUrl(first, last))
  await expectRhythm(
    page,
    testInfo,
    `${first}..${last}`,
    archivedVisits().filter(
      (visit) => localDay(visit.at) >= first && localDay(visit.at) <= last,
    ),
  )
})
