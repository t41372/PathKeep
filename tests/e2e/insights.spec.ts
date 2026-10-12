/**
 * Insights: the numbers on screen are the archive's numbers, in every scope.
 *
 * Proves: Insights opens on all time. After the background job that follows a
 * backup settles, switching the range with its toggle, as a person would,
 * never breaks a card or the screen; in each range "Top sites" ranks the same
 * site first, with the same visit count (in its short form), as counting the
 * fixture by hand, and the chart's axis shows what each bar covers: days for
 * up to 90 days, weeks for a year, months for all time (the fixture spans
 * three years). All time shows the archive's total visits with no comparison,
 * since there is no previous period. Each card's ⓘ says how much it looked
 * at, in what that card counts: cards that look at the same searches share a
 * number, the one that looks only at searches that reached a site has its
 * own, and all time's comes with its snapshot. A custom range from a link
 * and the browser profile filter count only their visits. The top site's own screen
 * keeps the scope and does the same in every range. Search result pages are
 * not sites (Insights counts searches separately), so they are left out of
 * the count. "Kinds of sites" puts every visit under the kind of site it
 * went to, as decided by hand for each fixture site, with sites on no list
 * shown last as not classified, however their page titles read. A site
 * shows the icon its browser stored (the fixture's Chrome has icons for three
 * sites), and its letter when there is none.
 */
import {
  expect,
  test,
  type Locator,
  type Page,
  type TestInfo,
} from '@playwright/test'
import {
  FIXTURE_FAVICON_HOSTS,
  FIXTURE_PAGE_ICONS,
} from '../../scripts/fixtures/synthetic-browsers.mjs'
import {
  archivedProfiles,
  archivedVisits,
  fixture,
  localDay,
  type FixtureVisit,
} from './support/fixture'
// A visit's site as Insights counts it (`visit_taxonomy::registrable_domain_for_host`).
import { siteOf as site } from './support/public-suffix'
import { kindOf } from './support/site-kinds'
import {
  searchFamilies,
  searchTrails,
  searchWords,
} from './support/search-words'

const compact = new Intl.NumberFormat('en', {
  notation: 'compact',
  maximumFractionDigits: 1,
})

const dayLabel = /^[A-Z][a-z]{2} \d{1,2}$/
const monthLabel = /^[A-Z][a-z]{2} \d{4}$/

/** Longest first, as in the toggle; `days: null` is all time. */
const ranges = [
  { label: 'All time', days: null },
  { label: '1y', days: 365 },
  { label: '90d', days: 90 },
  { label: '30d', days: 30 },
  { label: '7d', days: 7 },
] as const

/**
 * What one bar covers for a span of days (as `buckets.ts` decides): the site
 * trend's title and the axis labels. All time spans from the first visit.
 */
function bars(days: number) {
  if (days <= 92) return { siteTrend: 'Visits per day', axis: dayLabel }
  if (days <= 548) return { siteTrend: 'Visits per week', axis: dayLabel }
  return { siteTrend: 'Visits per month', axis: monthLabel }
}

function daysSince(at: number) {
  const first = new Date(at)
  first.setHours(0, 0, 0, 0)
  return (
    Math.round(
      (startOfDayDaysAgo(0).getTime() - first.getTime()) / 86_400_000,
    ) + 1
  )
}

function startOfDayDaysAgo(days: number): Date {
  const date = new Date()
  date.setHours(0, 0, 0, 0)
  date.setDate(date.getDate() - days)
  return date
}

const dateKey = (date: Date) =>
  `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`

/** Visits per site, by hand, between two instants. */
function visitsPerSite(
  visits: FixtureVisit[],
  since = -Infinity,
  until = Infinity,
) {
  const perSite = new Map<string, number>()
  for (const visit of visits) {
    if (visit.at < since || visit.at >= until || visit.term) continue
    perSite.set(site(visit.url), (perSite.get(site(visit.url)) ?? 0) + 1)
  }
  return perSite
}

const card = (page: Page, title: string) =>
  page
    .getByRole('heading', { name: title, exact: true })
    .locator('xpath=ancestor::section[1]')

/** A KPI tile's number, as it reads on screen. */
async function kpiText(page: Page, label: string) {
  const tile = page.getByRole('group', { name: label, exact: true })
  await expect(tile).toHaveAttribute('aria-busy', 'false')
  return (await tile.innerText()).split('\n')[1]?.trim() ?? ''
}

/** What a card's ⓘ says it was computed from ("1,234 visits"); the header's, before any inner list's own. */
async function evidenceOf(page: Page, title: string) {
  await card(page, title)
    .getByRole('button', { name: 'About this number' })
    .first()
    .click()
  const about = page.getByRole('dialog')
  const based = about.getByText(/^[\d,]+ (visits?|searches?|days?)\b/)
  await expect(based).toBeVisible()
  const text = await based.innerText()
  await page.keyboard.press('Escape')
  await expect(about).toBeHidden()
  return text
}

async function expectScreenIntact(page: Page) {
  await expect(
    page.getByRole('heading', { name: 'This screen hit an error' }),
  ).toHaveCount(0)
  await expect(page.getByRole('main').getByRole('alert')).toHaveCount(0)
}

/**
 * The chart's x axis labels, once they are drawn (Recharts skips crowded
 * ones). These charts have no y axis, so every SVG text is an axis label.
 */
async function axisLabels(chart: Locator) {
  const labels = chart.locator('svg text')
  const read = async () =>
    (await labels.allTextContents()).map((text) => text.trim()).filter(Boolean)
  await expect
    .poll(async () => (await read()).length, { timeout: 60_000 })
    .toBeGreaterThan(1)
  return read()
}

/**
 * Waits for "Top sites" to rank a leader of `perSite` first, with its count.
 * The previous scope's rows stay up until the new ones arrive, and stale
 * sections poll, so this waits for the row to settle; a row that never gets
 * there fails, and the attachment says what it showed last.
 */
async function expectTopSite(
  page: Page,
  testInfo: TestInfo,
  name: string,
  perSite: Map<string, number>,
) {
  const top = Math.max(...perSite.values())
  const expected = [...perSite]
    .filter(([, count]) => count === top)
    .map(([host]) => `${host} ${compact.format(top)}`)
  const firstRow = card(page, 'Top sites').getByRole('listitem').first()
  let text = ''
  try {
    await expect
      .poll(
        async () => {
          text = (await firstRow.innerText()).replace(/\s+/g, ' ').trim()
          return expected.some((row) => text.endsWith(row))
        },
        {
          message: `top site, ${name}: expected one of ${expected.join(', ')}`,
          timeout: 60_000,
        },
      )
      .toBe(true)
  } finally {
    await testInfo.attach(`top site, ${name}`, {
      body: JSON.stringify({ expected, shown: text }),
      contentType: 'application/json',
    })
  }
  return expected
    .map((row) => row.split(' ')[0])
    .find((host) => text.includes(host))!
}

test('Insights opens on all time, and every range shows the archive’s numbers', async ({
  page,
}, testInfo) => {
  test.setTimeout(300_000)
  await page.goto('/#/insights')
  const toggle = page.getByRole('group', { name: 'Time range' })
  await expect(toggle.getByRole('radio', { name: 'All time' })).toBeChecked()

  // All time: the archive's own total, and nothing to compare it with.
  const visitsTile = page.getByRole('group', { name: 'Page visits' })
  await expect(visitsTile).toContainText(
    compact.format(archivedVisits().length),
    { timeout: 60_000 },
  )
  await expect(visitsTile).not.toContainText('%')

  const firstVisit = Math.min(...archivedVisits().map((visit) => visit.at))
  let topSite = ''
  for (const { label, days } of ranges) {
    await toggle.getByRole('radio', { name: label }).click()
    const since =
      days === null ? -Infinity : startOfDayDaysAgo(days - 1).getTime()
    topSite = await expectTopSite(
      page,
      testInfo,
      label,
      visitsPerSite(archivedVisits(), since),
    )
    const { axis } = bars(days ?? daysSince(firstVisit))
    for (const tick of await axisLabels(card(page, 'Browsing over time')))
      expect(tick, label).toMatch(axis)
    await expectScreenIntact(page)
  }

  // The top site's screen keeps the range the toggle was left on.
  await card(page, 'Top sites')
    .getByRole('link', { name: new RegExp(topSite) })
    .first()
    .click()
  await expect(page).toHaveURL(/#\/insights\/site\/.+\?range=d7$/)
  const siteFirstVisit = Math.min(
    ...archivedVisits()
      .filter((visit) => !visit.term && site(visit.url) === topSite)
      .map((visit) => visit.at),
  )
  for (const { label, days } of ranges) {
    await toggle.getByRole('radio', { name: label }).click()
    const { siteTrend, axis } = bars(days ?? daysSince(siteFirstVisit))
    for (const tick of await axisLabels(card(page, siteTrend)))
      expect(tick, label).toMatch(axis)
    await expectScreenIntact(page)
  }
})

test('a custom range and one browser profile count only their visits', async ({
  page,
}, testInfo) => {
  test.setTimeout(180_000)
  // Ten days, about half a year back.
  const start = startOfDayDaysAgo(200)
  const end = startOfDayDaysAgo(191)
  const until = startOfDayDaysAgo(190).getTime()
  await page.goto(
    `/#/insights?range=custom&start=${dateKey(start)}&end=${dateKey(end)}`,
  )
  const toggle = page.getByRole('group', { name: 'Time range' })
  await expect(
    toggle.getByRole('radio', { name: /^Custom range: / }),
  ).toBeChecked()
  await expectTopSite(
    page,
    testInfo,
    'custom range',
    visitsPerSite(archivedVisits(), start.getTime(), until),
  )
  // Each card says how much it was computed from, in what it counts: Top
  // sites the visits to websites other than searches, Frequent searches the
  // searches (the KPI row's), Habits the days with any visit.
  const inRange = archivedVisits().filter(
    (visit) => visit.at >= start.getTime() && visit.at < until,
  )
  const siteVisits = [
    ...visitsPerSite(archivedVisits(), start.getTime(), until).values(),
  ].reduce((sum, count) => sum + count, 0)
  expect(await evidenceOf(page, 'Top sites')).toBe(
    `${siteVisits.toLocaleString('en-US')} visits to websites, not counting searches`,
  )
  expect(await evidenceOf(page, 'Frequent searches')).toBe(
    `${await kpiText(page, 'Searches')} searches`,
  )
  const days = new Set(inRange.map((visit) => localDay(visit.at))).size
  expect(await evidenceOf(page, 'Habits')).toBe(`${days} days of browsing`)
  await expectScreenIntact(page)

  // Only the Work profile, over all time.
  await toggle.getByRole('radio', { name: 'All time' }).click()
  await page.getByRole('combobox', { name: 'Browser profile' }).click()
  await page
    .getByRole('option', { name: 'Google Chrome · Work', exact: true })
    .click()
  await expect(page).toHaveURL(/profile=chrome%3AProfile(\+|%20)1/)
  const workTop = await expectTopSite(
    page,
    testInfo,
    'Work profile, all time',
    visitsPerSite(fixture().visitsByProfile['chrome:Profile 1']),
  )
  await expectScreenIntact(page)

  // The site screen keeps the profile; back on Insights it is still there.
  await card(page, 'Top sites')
    .getByRole('link', { name: new RegExp(workTop) })
    .first()
    .click()
  await expect(page).toHaveURL(/#\/insights\/site\/.+profile=chrome/)
  await expect(
    page.getByRole('combobox', { name: 'Browser profile' }),
  ).toHaveText('Google Chrome · Work')
  // The drill-in's own way back, not the navigation rail's.
  await page.getByRole('main').getByRole('link', { name: 'Insights' }).click()
  await expect(
    page.getByRole('combobox', { name: 'Browser profile' }),
  ).toHaveText('Google Chrome · Work')
})

const batched = [
  'get_digest_summary',
  'get_top_sites',
  'get_refind_pages',
  'get_search_engine_ranking',
  'get_top_search_concepts',
  'get_habit_patterns',
  'get_reopened_investigations',
  'get_breadth_index',
  'get_multi_browser_diff',
  'get_path_flows',
  'get_compare_sets',
  'get_search_effectiveness',
  'get_friction_signals',
  'get_stable_sources',
  'get_activity_mix',
]

test('the cards brought back from 0.3 count what the archive holds, read in two batches', async ({
  page,
}, testInfo) => {
  test.setTimeout(240_000)
  const commands: string[] = []
  page.on('request', (sent) => {
    const name = new URL(sent.url()).pathname.match(/^\/commands\/(\w+)$/)?.[1]
    if (name) commands.push(name)
  })
  await page.goto('/#/insights')
  const searches = archivedVisits().filter((visit) => visit.term)

  // Where you search: every fixture search is on Google.
  await expect(
    card(page, 'Where you search').getByRole('listitem').first(),
  ).toHaveText(
    new RegExp(`Google\\s*${compact.format(searches.length)} · 100%`),
    {
      timeout: 60_000,
    },
  )

  // What you search for: the word in the most searches comes first.
  const perWord = new Map<string, number>()
  for (const visit of searches)
    for (const word of searchWords(visit.term!))
      perWord.set(word, (perWord.get(word) ?? 0) + 1)
  const topCount = Math.max(...perWord.values())
  const topWords = [...perWord].filter(([, count]) => count === topCount)
  const firstWord = (
    await card(page, 'What you search for')
      .getByRole('link')
      .first()
      .innerText()
  ).replace(/\s+/g, ' ')
  await testInfo.attach('top search word', {
    body: JSON.stringify({ expected: topWords, shown: firstWord }),
    contentType: 'application/json',
  })
  expect(
    topWords.map(([word]) => `${word} ${compact.format(topCount)}`),
  ).toContain(firstWord)
  // Every word shown has the count the searches give it: a Chinese search
  // shown whole, as one word, has none.
  for (const entry of await card(page, 'What you search for')
    .getByRole('link')
    .allInnerTexts()) {
    const [, word, count] =
      entry.replace(/\s+/g, ' ').match(/^(.+) (\S+)$/) ?? []
    expect(compact.format(perWord.get(word ?? '') ?? 0), entry).toBe(count)
  }

  // Opening the page made the two batch reads, not a read per card.
  await expect(
    page.getByRole('heading', { name: 'Browsers compared' }),
  ).toBeVisible()
  await expect
    .poll(
      () =>
        commands.filter(
          (name) => name === 'get_intelligence_secondary_overview',
        ).length,
    )
    .toBe(1)
  await testInfo.attach('commands on opening Insights', {
    body: JSON.stringify(commands),
    contentType: 'application/json',
  })
  expect(
    commands.filter((name) => name === 'get_intelligence_primary_overview'),
  ).toHaveLength(1)
  expect(commands.filter((name) => batched.includes(name))).toEqual([])

  // Any other range reads the same two batches, computed for it.
  await page.goto('about:blank')
  commands.length = 0
  await page.goto('/#/insights?range=d30')
  await expect
    .poll(
      () =>
        commands.filter(
          (name) => name === 'get_intelligence_secondary_overview',
        ).length,
      { timeout: 60_000 },
    )
    .toBe(1)
  await testInfo.attach('commands on opening 30 days', {
    body: JSON.stringify(commands),
    contentType: 'application/json',
  })
  expect(
    commands.filter((name) => name === 'get_intelligence_primary_overview'),
  ).toHaveLength(1)
  expect(commands.filter((name) => batched.includes(name))).toEqual([])
  await page.goto('/#/insights')

  // All time: every site is new, so there is no "New sites" tile.
  await expect(page.getByRole('group', { name: 'New sites' })).toHaveCount(0)

  // The archive's first 30 days: the sites first visited in them.
  const first = new Date(Math.min(...archivedVisits().map((visit) => visit.at)))
  first.setHours(0, 0, 0, 0)
  const last = new Date(first)
  last.setDate(last.getDate() + 29)
  const until = new Date(last)
  until.setDate(until.getDate() + 1)
  const firstSeen = new Map<string, number>()
  for (const visit of archivedVisits()) {
    const at = firstSeen.get(site(visit.url))
    if (at === undefined || visit.at < at)
      firstSeen.set(site(visit.url), visit.at)
  }
  const newInWindow = [...firstSeen.values()].filter(
    (at) => at >= first.getTime() && at < until.getTime(),
  ).length
  await page.goto(
    `/#/insights?range=custom&start=${dateKey(first)}&end=${dateKey(last)}`,
  )
  await expect(page.getByRole('group', { name: 'New sites' })).toContainText(
    String(newInWindow),
    { timeout: 60_000 },
  )
  await expect(card(page, 'New sites')).toContainText(
    `${newInWindow} site${newInWindow === 1 ? '' : 's'} visited for the first time`,
  )
  // And what share of the range's sites they were, against the tile above.
  const distinctTile = page.getByRole('group', { name: 'Distinct sites' })
  await expect(distinctTile).toHaveAttribute('aria-busy', 'false')
  // The tile's second line is its number (see insights-drill.spec's `kpi`).
  const distinct = Number(
    ((await distinctTile.innerText()).split('\n')[1] ?? '')
      .trim()
      .replace(/,/g, ''),
  )
  expect(distinct, 'the range has sites').toBeGreaterThan(0)
  await expect(card(page, 'New sites')).toContainText(
    `${new Intl.NumberFormat('en', { style: 'percent', maximumFractionDigits: 1 }).format(Math.min(1, newInWindow / distinct))} of the sites you visited`,
  )

  // Last 30 days, ordered by days visited.
  await page.goto('/#/insights?range=d30')
  const since = startOfDayDaysAgo(29)
  const daysPerSite = new Map<string, Set<string>>()
  for (const visit of archivedVisits()) {
    if (visit.at < since.getTime() || visit.term) continue
    const days = daysPerSite.get(site(visit.url)) ?? new Set<string>()
    days.add(dateKey(new Date(visit.at)))
    daysPerSite.set(site(visit.url), days)
  }
  const perSiteDays = new Map(
    [...daysPerSite].map(([host, days]) => [host, days.size]),
  )
  await card(page, 'Top sites')
    .getByRole('combobox', { name: 'Order sites by' })
    .click()
  await page.getByRole('option', { name: 'Days visited' }).click()
  await expectTopSite(page, testInfo, 'by days visited, 30d', perSiteDays)

  // An average weekday against an average weekend day, last 30 days: each
  // kind's visits over how many of its days the range holds.
  let weekdays = 0
  let weekdayVisits = 0
  let weekendVisits = 0
  for (let offset = 0; offset < 30; offset += 1) {
    const day = startOfDayDaysAgo(offset).getDay()
    if (day !== 0 && day !== 6) weekdays += 1
  }
  for (const visit of archivedVisits()) {
    if (visit.at < since.getTime()) continue
    const day = new Date(visit.at).getDay()
    if (day === 0 || day === 6) weekendVisits += 1
    else weekdayVisits += 1
  }
  const perDay = (visits: number, days: number) =>
    new Intl.NumberFormat('en').format(Math.round(visits / days))
  await expect(card(page, 'Rhythm')).toContainText(
    `Weekdays${perDay(weekdayVisits, weekdays)} a day`,
    { timeout: 60_000 },
  )
  await expect(card(page, 'Rhythm')).toContainText(
    `Weekends${perDay(weekendVisits, 30 - weekdays)} a day`,
  )
  // The three hours of the day with the most visits in those 30 days, most
  // first, on the clock the app uses here: Linux has no clock setting to
  // read, and English then uses 12 hours.
  const perHour = new Array<number>(24).fill(0)
  for (const visit of archivedVisits())
    if (visit.at >= since.getTime()) perHour[new Date(visit.at).getHours()] += 1
  const busiest = perHour
    .map((visits, hour) => ({ hour, visits }))
    .filter((entry) => entry.visits > 0)
    .sort((a, b) => b.visits - a.visits || a.hour - b.hour)
    .slice(0, 3)
  await testInfo.attach('busiest hours, 30d', {
    body: JSON.stringify({ perHour, busiest }, null, 2),
    contentType: 'application/json',
  })
  const spans = await page.evaluate(
    (hours) => {
      const time = new Intl.DateTimeFormat('en', {
        hour: 'numeric',
        minute: '2-digit',
        hourCycle: 'h12',
      })
      const at = (hour: number) => time.format(new Date(2000, 0, 1, hour))
      return hours.map((hour) => `${at(hour)}–${at((hour + 1) % 24)}`)
    },
    busiest.map((entry) => entry.hour),
  )
  await expect(card(page, 'Rhythm')).toContainText(
    `Busiest hours: ${spans.join(', ')}`,
  )
  await expectScreenIntact(page)

  // What the page looks like, section by section.
  for (const heading of [
    'Insights',
    'When you browse',
    'Searching',
    'Patterns',
  ]) {
    await page
      .getByRole('heading', { name: heading, exact: true })
      .scrollIntoViewIfNeeded()
    await testInfo.attach(`screen: ${heading}`, {
      body: await page.screenshot(),
      contentType: 'image/png',
    })
  }
})

/** Every archived profile's search trails, by hand (`support/search-words.ts`). */
function allSearchTrails() {
  return archivedProfiles.flatMap((profile) =>
    searchTrails(fixture().visitsByProfile[profile]),
  )
}

test('reworded searches are counted, and listed as searches that took several tries', async ({
  page,
}, testInfo) => {
  test.setTimeout(180_000)
  const trails = allSearchTrails()
  const reworded = trails.filter((trail) => trail.rewordings >= 2)
  const average =
    trails.reduce((sum, trail) => sum + trail.rewordings, 0) / trails.length
  await testInfo.attach('search trails by hand', {
    body: JSON.stringify({
      trails: trails.length,
      rewordedTwiceOrMore: reworded.length,
      average,
    }),
    contentType: 'application/json',
  })
  expect(reworded.length).toBeGreaterThan(10)

  await page.goto('/#/insights')
  const outcomes = card(page, 'How your searches go')
  await expect(outcomes).toContainText(
    `${new Intl.NumberFormat('en').format(trails.length)} searches`,
    { timeout: 60_000 },
  )
  await expect(outcomes).toContainText(
    `Reworded ${Math.round(average * 10) / 10}× on average`,
  )
  // The card lists the ten reworded most, most first.
  const expected = reworded
    .map((trail) => trail.rewordings)
    .sort((a, b) => b - a)
    .slice(0, 10)
  const tries = card(page, 'Searches that took several tries').getByRole(
    'listitem',
  )
  await expect(tries).toHaveCount(expected.length)
  for (const [index, row] of (await tries.all()).entries())
    await expect(row).toContainText(`Reworded ${expected[index]} times`)
  await expectScreenIntact(page)
})

test('the most frequent path between three sites is counted like the archive', async ({
  page,
}, testInfo) => {
  test.setTimeout(240_000)
  // Per browser: sessions break at 30 minutes; a run of one site is one step;
  // a path is three different sites in a row. A path belongs to the day its
  // last site was first visited, so a range counts the paths ending in it,
  // whenever they began.
  const countPaths = (from: number) => {
    const perPath = new Map<string, number>()
    for (const visits of Object.values(fixture().visitsByProfile)) {
      if (!archivedVisits().includes(visits[0])) continue
      let last = -Infinity
      let steps: string[] = []
      for (const visit of [...visits].sort((a, b) => a.at - b.at)) {
        if (visit.at - last > 30 * 60_000) steps = []
        last = visit.at
        const here = site(visit.url)
        if (steps.at(-1) === here) continue
        steps.push(here)
        const window = steps.slice(-3)
        if (
          visit.at >= from &&
          window.length === 3 &&
          new Set(window).size === 3
        ) {
          const path = window.join(' → ')
          perPath.set(path, (perPath.get(path) ?? 0) + 1)
        }
      }
    }
    const top = Math.max(...perPath.values())
    const leaders = [...perPath]
      .filter(([, count]) => count === top)
      .map(([path]) => path)
    return { top, leaders }
  }
  const now = new Date()
  // 90 days, today included, from local midnight: what "90 days" asks for.
  const ninetyDays = new Date(
    now.getFullYear(),
    now.getMonth(),
    now.getDate() - 89,
  ).getTime()

  for (const [url, from] of [
    ['/#/insights', -Infinity],
    ['/#/insights?range=d90', ninetyDays],
  ] as const) {
    const { top, leaders } = countPaths(from)
    await testInfo.attach(`most frequent paths by hand, ${url}`, {
      body: JSON.stringify({ top, leaders }),
      contentType: 'application/json',
    })
    // A fresh app, so no row from the range before can pass for this one.
    await page.goto('about:blank')
    await page.goto(url)
    const first = card(page, 'How you move between sites')
      .getByRole('listitem')
      .first()
    await expect(first).toContainText(
      `${new Intl.NumberFormat('en').format(top)} times`,
      { timeout: 60_000 },
    )
    const shown = (await first.innerText()).replace(/\s+/g, ' ')
    expect(
      leaders.some((path) =>
        path.split(' → ').every((step) => shown.includes(step)),
      ),
      `${url}: ${shown} should be one of ${leaders.join(', ')}`,
    ).toBe(true)
  }
})

test('where searches end up is counted like the archive', async ({
  page,
}, testInfo) => {
  test.setTimeout(180_000)
  const perSite = new Map<string, number>()
  for (const trail of allSearchTrails())
    if (trail.landing)
      perSite.set(trail.landing, (perSite.get(trail.landing) ?? 0) + 1)
  const top = Math.max(...perSite.values())
  const leaders = [...perSite].filter(([, count]) => count === top)
  await testInfo.attach('search landings by hand', {
    body: JSON.stringify({ top, leaders: leaders.map(([host]) => host) }),
    contentType: 'application/json',
  })

  await page.goto('/#/insights')
  const first = card(page, 'Where your searches end up')
    .getByRole('listitem')
    .first()
  await expect(first).toContainText(
    `${compact.format(top)} search${top === 1 ? '' : 'es'}`,
    { timeout: 60_000 },
  )
  const shown = (await first.innerText()).replace(/\s+/g, ' ')
  expect(
    leaders.some(([host]) => shown.includes(host)),
    `${shown} should name one of ${leaders.map(([host]) => host).join(', ')}`,
  ).toBe(true)
})

test('browsers compared name the sites they share and what each mostly visits', async ({
  page,
}) => {
  const percent = new Intl.NumberFormat('en', {
    style: 'percent',
    maximumFractionDigits: 1,
  })
  // Each browser profile's two biggest kinds of sites, unclassified left out;
  // a tie at the cut leaves the second open.
  const kindLines = archivedProfiles.map((profile) => {
    const visits = fixture().visitsByProfile[profile]
    const perKind = new Map<string, number>()
    for (const visit of visits)
      perKind.set(kindOf(visit), (perKind.get(kindOf(visit)) ?? 0) + 1)
    const ranked = [...perKind]
      .filter(([kind]) => kind !== 'Not classified')
      .sort((a, b) => b[1] - a[1])
    const part = ([kind, count]: [string, number]) =>
      `${kind} ${percent.format(count / visits.length)}`
    return ranked.length > 2 && ranked[1][1] === ranked[2][1]
      ? `Mostly ${part(ranked[0])}, `
      : `Mostly ${ranked.slice(0, 2).map(part).join(', ')}`
  })
  // Sites more than one profile visited, most visits in all of them first.
  // Unlike Top sites, the card counts every visit, a search's included.
  const perProfile = archivedProfiles.map((profile) => {
    const perSite = new Map<string, number>()
    for (const visit of fixture().visitsByProfile[profile])
      perSite.set(site(visit.url), (perSite.get(site(visit.url)) ?? 0) + 1)
    return perSite
  })
  const shared = [...new Set(perProfile.flatMap((sites) => [...sites.keys()]))]
    .filter((key) => perProfile.filter((sites) => sites.has(key)).length > 1)
    .map((key) => ({
      key,
      visits: perProfile.reduce((sum, sites) => sum + (sites.get(key) ?? 0), 0),
    }))
    // Ties by name in byte order, as the backend sorts them.
    .sort((a, b) => b.visits - a.visits || (a.key < b.key ? -1 : 1))

  await page.goto('/#/insights')
  const browsers = card(page, 'Browsers compared')
  for (const line of kindLines)
    await expect(browsers.getByText(line).first()).toBeVisible({
      timeout: 60_000,
    })
  await expect(browsers).toContainText(
    `${shared.length} sites were visited in more than one browser. Most visited of them: ${shared
      .slice(0, 3)
      .map((entry) => entry.key)
      .join(', ')}`,
  )
})

test('how searches go averages rewordings and pages per search engine', async ({
  page,
}) => {
  // All time; every fixture search is on Google.
  const trails = allSearchTrails()
  const average = (value: (trail: (typeof trails)[number]) => number) =>
    new Intl.NumberFormat('en').format(
      Math.round(
        (trails.reduce((sum, trail) => sum + value(trail), 0) / trails.length) *
          10,
      ) / 10,
    )
  await page.goto('/#/insights')
  const google = card(page, 'How your searches go')
    .getByRole('listitem')
    .filter({ hasText: 'Google' })
    .first()
  await expect(google).toContainText(
    `${new Intl.NumberFormat('en').format(trails.length)} searches`,
    { timeout: 60_000 },
  )
  await expect(google).toContainText(
    `Reworded ${average((trail) => trail.rewordings)}× on average · ${average((trail) => trail.pages)} pages opened per search`,
  )
})

test('where searches end up counts each search on the site it led to', async ({
  page,
}, testInfo) => {
  // All time: each trail counts for its landing site, with how many
  // different searches (as first typed) led there.
  const perSite = new Map<string, { searches: number; queries: Set<string> }>()
  for (const trail of allSearchTrails()) {
    if (!trail.landing) continue
    const entry = perSite.get(trail.landing) ?? {
      searches: 0,
      queries: new Set<string>(),
    }
    entry.searches += 1
    entry.queries.add(trail.query)
    perSite.set(trail.landing, entry)
  }
  const expected = [...perSite]
    .sort(([a, x], [b, y]) => y.searches - x.searches || (a < b ? -1 : 1))
    .slice(0, 3)
    .map(
      ([host, entry]) =>
        `${host} ${entry.searches} search${entry.searches === 1 ? '' : 'es'} · ${entry.queries.size} different`,
    )
  await page.goto('/#/insights')
  const rows = card(page, 'Where your searches end up').getByRole('listitem')
  await expect(rows.first()).toBeVisible({ timeout: 60_000 })
  const shown = (await rows.allInnerTexts())
    .slice(0, 3)
    .map((text) => text.replace(/\s+/g, ' ').trim())
  await testInfo.attach('landings, all time', {
    body: JSON.stringify({ expected, shown }, null, 2),
    contentType: 'application/json',
  })
  // A row may start with the site's letter badge.
  expect(shown.map((text) => text.replace(/^\S\s/, ''))).toEqual(expected)

  // All time opens from the kept snapshot, which carries each card's count.
  // This card looks only at the searches that reached a site; the other
  // search cards look at every search and where it led.
  const trails = allSearchTrails()
  const landed = trails.filter((trail) => trail.landing).length
  expect(landed).toBeLessThan(trails.length)
  expect(await evidenceOf(page, 'Where your searches end up')).toBe(
    `${landed.toLocaleString('en-US')} searches that led to a website`,
  )
  expect(await evidenceOf(page, 'Searches that took several tries')).toBe(
    `${trails.length.toLocaleString('en-US')} searches and where they led`,
  )
})

test('things you came back to list searches and pages by when you last came back', async ({
  page,
}, testInfo) => {
  // All time, per profile: search families with searches on more than one
  // day (counted in searches), and pages opened on more than one day
  // (counted in days, named by their first visit's title).
  type Thread = {
    key: string
    label: string
    count: number
    last: number
    search: boolean
  }
  const threads: Thread[] = []
  for (const profile of archivedProfiles) {
    const visits = fixture().visitsByProfile[profile]
    for (const family of searchFamilies(visits))
      if (family.days.size > 1)
        threads.push({
          key: `${profile}:${family.anchor}`,
          label: family.anchor,
          count: family.members,
          last: family.latest,
          search: true,
        })
    const pages = new Map<
      string,
      { title: string; first: number; last: number; days: Set<string> }
    >()
    for (const visit of visits) {
      if (visit.term) continue
      const entry = pages.get(visit.url) ?? {
        title: visit.title,
        first: visit.at,
        last: visit.at,
        days: new Set<string>(),
      }
      entry.days.add(dateKey(new Date(visit.at)))
      if (visit.at < entry.first) {
        entry.first = visit.at
        entry.title = visit.title
      }
      entry.last = Math.max(entry.last, visit.at)
      pages.set(visit.url, entry)
    }
    for (const [url, entry] of pages)
      if (entry.days.size > 1)
        threads.push({
          key: url,
          label: entry.title,
          count: entry.days.size,
          last: entry.last,
          search: false,
        })
  }
  // The card reads the ten newest, keeps one row per page (the profile that
  // opened it on most days; a search's family is its profile's own), newest
  // first, six at most.
  const kept = new Map<string, Thread>()
  for (const thread of threads
    .sort((a, b) => b.last - a.last || b.count - a.count)
    .slice(0, 10)) {
    const old = kept.get(thread.key)
    if (!old || thread.count > old.count) kept.set(thread.key, thread)
  }
  const expected = [...kept.values()]
    .sort((a, b) => b.last - a.last)
    .slice(0, 6)
    .map((thread) =>
      thread.search
        ? `${thread.label} Searched ${thread.count} time${thread.count === 1 ? '' : 's'}`
        : `${thread.label} Opened on ${thread.count} day${thread.count === 1 ? '' : 's'}`,
    )

  await page.goto('/#/insights')
  const rows = card(page, 'Things you came back to').getByRole('listitem')
  await expect(rows).toHaveCount(expected.length, { timeout: 60_000 })
  const shown = (await rows.allInnerTexts()).map((text) =>
    text
      .replace(/\s+/g, ' ')
      .replace(/ · last .*$/, '')
      .trim(),
  )
  await testInfo.attach('things you came back to, all time', {
    body: JSON.stringify({ expected, shown }, null, 2),
    contentType: 'application/json',
  })
  expect(shown).toEqual(expected)
})

test('habits are sites visited on steady days, stopped ones first', async ({
  page,
}, testInfo) => {
  // All time, every profile: a site's local days with a visit (a search's
  // included). A habit needs 5 days over 14 or more, and steady gaps: daily
  // under 2 days apart with little spread, weekly 5 to 10, periodic beyond 10
  // (`classify_habit_pattern`). It has stopped when the time since the last
  // visit is more than twice the usual gap.
  const DAY = 86_400_000
  const perSite = new Map<string, { days: Set<number>; last: number }>()
  for (const visit of archivedVisits()) {
    const at = new Date(visit.at)
    const day = Date.UTC(at.getFullYear(), at.getMonth(), at.getDate()) / DAY
    const entry = perSite.get(site(visit.url)) ?? {
      days: new Set<number>(),
      last: 0,
    }
    entry.days.add(day)
    entry.last = Math.max(entry.last, visit.at)
    perSite.set(site(visit.url), entry)
  }
  const now = Date.now()
  const habits = [...perSite].flatMap(([host, { days, last }]) => {
    const sorted = [...days].sort((a, b) => a - b)
    const span = sorted.at(-1)! - sorted[0]
    if (sorted.length < 5 || span < 14) return []
    const gaps = sorted.slice(1).map((day, index) => day - sorted[index])
    const mean = span / gaps.length
    const variance = Math.max(
      0,
      gaps.reduce((sum, gap) => sum + gap * gap, 0) / gaps.length - mean * mean,
    )
    const cv = mean === 0 ? 0 : Math.sqrt(variance) / mean
    const rhythm =
      mean < 2 && cv < 0.5
        ? 'Almost every day'
        : mean >= 5 && mean <= 10 && cv < 0.6
          ? 'About once a week'
          : mean > 10 && cv < 0.8
            ? `About every ${Math.round(mean)} days`
            : null
    if (!rhythm) return []
    return [
      {
        host,
        last,
        days: sorted.length,
        stopped: (now - last) / DAY > mean * 2,
        text: `${host} ${rhythm} · ${sorted.length} days`,
      },
    ]
  })
  // The read orders by last visit, then site; the card puts stopped ones
  // first, then the most days, and shows six.
  const expected = habits
    .sort((a, b) => b.last - a.last || (a.host < b.host ? -1 : 1))
    .sort((a, b) => Number(b.stopped) - Number(a.stopped) || b.days - a.days)
    .slice(0, 6)
    .map((habit) => habit.text)

  await page.goto('/#/insights')
  const rows = card(page, 'Habits').getByRole('listitem')
  await expect(rows).toHaveCount(expected.length, { timeout: 60_000 })
  // A row: the site's letter badge (when it has no icon), its name, its
  // rhythm and days; then when it was last visited or that it stopped.
  const shown = (await rows.allInnerTexts()).map(
    (text) =>
      text
        .replace(/\s+/g, ' ')
        .trim()
        .match(/^(?:\S )?(.+? · [\d,]+ days?)/)?.[1] ?? text,
  )
  await testInfo.attach('habits, all time', {
    body: JSON.stringify({ expected, shown }, null, 2),
    contentType: 'application/json',
  })
  expect(shown).toEqual(expected)
})

test('explore or focus counts the sites that took half the visits', async ({
  page,
}) => {
  // Last 30 days, every visit (a search's included): the fewest sites,
  // most visited first, that hold at least half of them.
  const since = startOfDayDaysAgo(29).getTime()
  const perSite = new Map<string, number>()
  for (const visit of archivedVisits())
    if (visit.at >= since)
      perSite.set(site(visit.url), (perSite.get(site(visit.url)) ?? 0) + 1)
  const counts = [...perSite.values()].sort((a, b) => b - a)
  const total = counts.reduce((sum, count) => sum + count, 0)
  let running = 0
  const half = counts.findIndex((count) => (running += count) * 2 >= total) + 1
  await page.goto('/#/insights?range=d30')
  await expect(card(page, 'Explore or focus')).toContainText(
    `Half of your visits went to ${half} site${half === 1 ? '' : 's'}.`,
    { timeout: 60_000 },
  )
})

test('kinds of sites count every visit by the kind of site it went to', async ({
  page,
}, testInfo) => {
  const perKind = new Map<string, number>()
  for (const visit of archivedVisits())
    perKind.set(kindOf(visit), (perKind.get(kindOf(visit)) ?? 0) + 1)
  const total = archivedVisits().length
  const percent = new Intl.NumberFormat('en', {
    style: 'percent',
    maximumFractionDigits: 1,
  })
  const expected = [...perKind].map(
    ([kind, visits]) => `${kind} ${percent.format(visits / total)}`,
  )

  await page.goto('/#/insights')
  const rows = card(page, 'Kinds of sites').getByRole('listitem')
  await expect(rows).toHaveCount(expected.length, { timeout: 60_000 })
  const shown = (await rows.allInnerTexts()).map((text) =>
    text.replace(/\s+/g, ' ').trim(),
  )
  await testInfo.attach('kinds of sites, all time', {
    body: JSON.stringify({ expected, shown }, null, 2),
    contentType: 'application/json',
  })
  expect([...shown].sort()).toEqual([...expected].sort())
  // Most visits first; sites on no list last, whatever their share.
  expect(shown.at(-1)).toMatch(/^Not classified /)
  await expectScreenIntact(page)
})

test('a site shows the icon its browser stored, and its letter when there is none', async ({
  page,
}) => {
  // History shows a page's own icon: the issue page has GitHub's status icon,
  // stored after GitHub's own.
  const issue = 'https://github.com/tokio-rs/tokio/issues/5124'
  await page.goto('/#/history')
  await page
    .getByRole('searchbox', { name: 'Search history' })
    .fill('spawn_blocking pool exhaustion')
  await expect(
    page
      .getByRole('option', { name: /spawn_blocking pool exhaustion/ })
      .first()
      // The row's site icon; a row also shows its browser's badge.
      .locator('img[src^="data:image"]'),
  ).toHaveAttribute(
    'src',
    `data:image/png;base64,${FIXTURE_PAGE_ICONS[issue]}`,
    {
      timeout: 30_000,
    },
  )
  // Insights, opened next in the same window, still shows each site's own.
  await page.goto('/#/insights')
  const rows = card(page, 'Top sites').getByRole('listitem')
  await expect(rows.first()).toBeVisible({ timeout: 60_000 })
  // The fixture's Chrome stored a different icon for github.com,
  // www.youtube.com and docs.rs; a site's icon is its own host's (its front
  // page's, else the one most of its pages have), else www.'s, and never
  // another site's or one page's.
  for (const [site, host] of [
    ['github.com', 'github.com'],
    ['youtube.com', 'www.youtube.com'],
    ['docs.rs', 'docs.rs'],
  ] as const) {
    const icon = rows.filter({ hasText: site }).locator('img')
    await expect(icon, site).toHaveAttribute(
      'src',
      `data:image/png;base64,${FIXTURE_FAVICON_HOSTS[host]}`,
    )
  }
  const strava = rows.filter({ hasText: 'strava.com' })
  await expect(strava).toHaveCount(1)
  await expect(strava.locator('img')).toHaveCount(0)
  await expectScreenIntact(page)
})
