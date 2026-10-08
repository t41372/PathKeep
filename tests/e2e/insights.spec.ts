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
 * since there is no previous period. A custom range from a link and the
 * browser profile filter count only their visits. The top site's own screen
 * keeps the scope and does the same in every range. Search result pages are
 * not sites (Insights counts searches separately), so they are left out of
 * the count.
 */
import {
  expect,
  test,
  type Locator,
  type Page,
  type TestInfo,
} from '@playwright/test'
import { archivedVisits, fixture, type FixtureVisit } from './support/fixture'

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

/**
 * A visit's site as Insights counts it: the registrable domain. The fixture's
 * hosts all sit under one-label suffixes except `github.io`, which the public
 * suffix list makes a suffix of its own.
 */
function site(url: string) {
  const labels = new URL(url).hostname.split('.')
  const suffix = labels.slice(-2).join('.') === 'github.io' ? 3 : 2
  return labels.slice(-suffix).join('.')
}

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

/** The words Insights counts for a search (see `search_words` in the backend). */
const fillerWords = new Set(
  'a an and com edu for from how html http https in is net of on org the to what when where with www'.split(
    ' ',
  ),
)
function searchWords(term: string) {
  return new Set(
    term
      .toLowerCase()
      .split(/[^\p{L}\p{N}]+/u)
      .filter(
        (word) =>
          word &&
          !fillerWords.has(word) &&
          word.length > 1 &&
          !/^\d+$/.test(word),
      ),
  )
}

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

  // An average weekday against an average weekend day, last 30 days.
  let weekdays = 0
  let weekdayVisits = 0
  for (let offset = 0; offset < 30; offset += 1) {
    const day = startOfDayDaysAgo(offset).getDay()
    if (day !== 0 && day !== 6) weekdays += 1
  }
  for (const visit of archivedVisits()) {
    const day = new Date(visit.at).getDay()
    if (visit.at >= since.getTime() && day !== 0 && day !== 6)
      weekdayVisits += 1
  }
  await expect(card(page, 'Rhythm')).toContainText(
    `Weekdays${new Intl.NumberFormat('en').format(Math.round(weekdayVisits / weekdays))} a day`,
    { timeout: 60_000 },
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

/**
 * Search trails by hand, by the documented rule: a search continues the
 * current trail when it is in the same browser and session (30 minutes), the
 * trail's last visit is under 15 minutes old, and the two searches share at
 * least half their words or one's words are all in the other.
 */
function searchTrails() {
  const SESSION_GAP = 30 * 60_000
  const TRAIL_GAP = 15 * 60_000
  const trails: {
    rewordings: number
    latest: Set<string>
    landing?: string
  }[] = []
  for (const visits of Object.values(fixture().visitsByProfile)) {
    if (!archivedVisits().includes(visits[0])) continue
    let lastVisit = -Infinity
    let trail: {
      rewordings: number
      latest: Set<string>
      lastAt: number
      landing?: string
      landsNext: boolean
    } | null = null
    for (const visit of [...visits].sort((a, b) => a.at - b.at)) {
      const sameSession = visit.at - lastVisit <= SESSION_GAP
      lastVisit = visit.at
      const open = trail && sameSession && visit.at - trail.lastAt <= TRAIL_GAP
      if (visit.term) {
        // The search's words as the backend compares them: filler words out.
        const words = new Set(
          visit.term
            .toLowerCase()
            .split(/[^\p{L}\p{N}]+/u)
            .filter((word) => word && !fillerWords.has(word)),
        )
        const shared = trail
          ? [...words].filter((word) => trail!.latest.has(word)).length
          : 0
        const union = trail ? new Set([...words, ...trail.latest]).size : 0
        const rewords =
          open &&
          (shared === words.size ||
            shared === trail!.latest.size ||
            shared / union >= 0.5)
        if (rewords) {
          if ([...words].join(' ') !== [...trail!.latest].join(' '))
            trail!.rewordings += 1
          trail!.latest = words
          trail!.lastAt = visit.at
          // The page opened after the last wording becomes the landing.
          trail!.landsNext = true
        } else {
          trail = {
            rewordings: 0,
            latest: words,
            lastAt: visit.at,
            landsNext: false,
          }
          trails.push(trail)
        }
      } else if (open) {
        trail!.lastAt = visit.at
        if (trail!.landing === undefined || trail!.landsNext) {
          trail!.landing = site(visit.url)
          trail!.landsNext = false
        }
      } else {
        trail = null
      }
    }
  }
  return trails
}

test('reworded searches are counted, and listed as searches that took several tries', async ({
  page,
}, testInfo) => {
  test.setTimeout(180_000)
  const trails = searchTrails()
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
  test.setTimeout(180_000)
  // Per browser: sessions break at 30 minutes; a run of one site is one step;
  // a path is three different sites in a row.
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
      if (window.length === 3 && new Set(window).size === 3) {
        const path = window.join(' → ')
        perPath.set(path, (perPath.get(path) ?? 0) + 1)
      }
    }
  }
  const top = Math.max(...perPath.values())
  const leaders = [...perPath].filter(([, count]) => count === top)
  await testInfo.attach('most frequent paths by hand', {
    body: JSON.stringify({ top, leaders: leaders.map(([path]) => path) }),
    contentType: 'application/json',
  })

  await page.goto('/#/insights')
  const first = card(page, 'How you move between sites')
    .getByRole('listitem')
    .first()
  await expect(first).toContainText(
    `${new Intl.NumberFormat('en').format(top)} times`,
    {
      timeout: 60_000,
    },
  )
  const shown = (await first.innerText()).replace(/\s+/g, ' ')
  expect(
    leaders.some(([path]) =>
      path.split(' → ').every((step) => shown.includes(step)),
    ),
    `${shown} should be one of ${leaders.map(([path]) => path).join(', ')}`,
  ).toBe(true)
})

test('where searches end up is counted like the archive', async ({
  page,
}, testInfo) => {
  test.setTimeout(180_000)
  const perSite = new Map<string, number>()
  for (const trail of searchTrails())
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
