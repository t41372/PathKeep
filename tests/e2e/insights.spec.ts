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

/** Visits per site, by hand, between two instants. */
function visitsPerSite(
  visits: FixtureVisit[],
  since = -Infinity,
  until = Infinity,
) {
  const perSite = new Map<string, number>()
  for (const visit of visits) {
    if (visit.at < since || visit.at >= until || visit.term) continue
    const host = new URL(visit.url).hostname.replace(/^www\./, '')
    perSite.set(host, (perSite.get(host) ?? 0) + 1)
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
      .filter(
        (visit) =>
          !visit.term &&
          new URL(visit.url).hostname.replace(/^www\./, '') === topSite,
      )
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
