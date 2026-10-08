/**
 * Insights: the numbers on screen are the archive's numbers, in every range.
 *
 * Proves: after the background job that follows a backup settles, switching
 * the range with its toggle, as a person would, never breaks a card or the
 * screen; each range's "Top sites" ranks the same site first, with the same
 * visit count (in its short form), as counting the fixture by hand; the activity chart's axis
 * shows dates (one bar per week for a year). The top site's own screen does
 * the same in every range. Search result pages are not sites (Insights
 * counts searches separately), so they are left out of the count.
 */
import { expect, test, type Locator, type Page } from '@playwright/test'
import { archivedVisits } from './support/fixture'

const compact = new Intl.NumberFormat('en', {
  notation: 'compact',
  maximumFractionDigits: 1,
})

const ranges = [
  { label: '7d', days: 7 },
  { label: '30d', days: 30 },
  { label: '90d', days: 90 },
  { label: '1y', days: 365 },
]

function startOfDayDaysAgo(days: number) {
  const date = new Date()
  date.setHours(0, 0, 0, 0)
  date.setDate(date.getDate() - days)
  return date.getTime()
}

/** Visits per site over the last `days` days, by hand. */
function visitsPerSite(days: number) {
  const since = startOfDayDaysAgo(days - 1)
  const perSite = new Map<string, number>()
  for (const visit of archivedVisits()) {
    if (visit.at < since || visit.term) continue
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
 * ones). These charts have no y axis, so every SVG text is a date label.
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

test('every range shows the archive’s numbers, and the top site’s screen too', async ({
  page,
}, testInfo) => {
  test.setTimeout(300_000)
  await page.goto('/#/insights')
  const toggle = page.getByRole('group', { name: 'Time range' })
  let topSite = ''

  for (const { label, days } of ranges) {
    await toggle.getByRole('radio', { name: label }).click()
    const perSite = visitsPerSite(days)
    const top = Math.max(...perSite.values())
    const leaders = [...perSite]
      .filter(([, count]) => count === top)
      .map(([host]) => host)

    // The previous range's rows stay up until the new ones arrive, and stale
    // sections poll, so wait for the first row to settle on a leader with the
    // count by hand (the list shows counts in short form, "2.3K"). A row that
    // never gets there fails with what it showed last.
    const firstRow = card(page, 'Top sites').getByRole('listitem').first()
    const expected = leaders.map((host) => `${host} ${compact.format(top)}`)
    let text = ''
    try {
      await expect
        .poll(
          async () => {
            text = (await firstRow.innerText()).replace(/\s+/g, ' ').trim()
            return expected.some((row) => text.endsWith(row))
          },
          {
            message: `top site, ${label}: expected one of ${expected.join(', ')}`,
            timeout: 60_000,
          },
        )
        .toBe(true)
    } finally {
      await testInfo.attach(`top site, ${label}`, {
        body: JSON.stringify({ expected, shown: text }),
        contentType: 'application/json',
      })
    }
    topSite = leaders.find((host) => text.includes(host))!

    const labels = await axisLabels(card(page, 'Daily activity'))
    for (const tick of labels)
      expect(tick, label).toMatch(/^[A-Z][a-z]{2} \d{1,2}$/)
    await expectScreenIntact(page)
  }

  // The top site's screen, starting from the year the toggle was left on.
  await card(page, 'Top sites')
    .getByRole('link', { name: new RegExp(topSite) })
    .first()
    .click()
  await expect(page).toHaveURL(/#\/insights\/site\/.+\?range=y1$/)
  for (const { label } of [...ranges].reverse()) {
    await toggle.getByRole('radio', { name: label }).click()
    const trend = card(
      page,
      label === '1y' ? 'Visits per week' : 'Visits per day',
    )
    for (const tick of await axisLabels(trend))
      expect(tick, label).toMatch(/^[A-Z][a-z]{2} \d{1,2}$/)
    await expectScreenIntact(page)
  }
})
