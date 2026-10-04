/**
 * Insights: the numbers on screen are the archive's numbers.
 *
 * Proves: after the background job that follows a backup settles, "Top
 * sites" for the last 30 days ranks the same site first, with the same visit
 * count, as counting the fixture by hand. Search result pages are not sites
 * (Insights counts searches separately), so they are left out of the count.
 */
import { expect, test } from '@playwright/test'
import { archivedVisits, expectCount, parseCount } from './support/fixture'

function startOfDayDaysAgo(days: number) {
  const date = new Date()
  date.setHours(0, 0, 0, 0)
  date.setDate(date.getDate() - days)
  return date.getTime()
}

test('top sites for the last 30 days match the archive', async ({
  page,
}, testInfo) => {
  const since = startOfDayDaysAgo(29)
  const perSite = new Map<string, number>()
  for (const visit of archivedVisits()) {
    if (visit.at < since || visit.term) continue
    const host = new URL(visit.url).hostname.replace(/^www\./, '')
    perSite.set(host, (perSite.get(host) ?? 0) + 1)
  }
  const top = Math.max(...perSite.values())
  const leaders = [...perSite]
    .filter(([, count]) => count === top)
    .map(([host]) => host)

  await page.goto('/#/insights?range=d30')
  const card = page
    .getByRole('heading', { name: 'Top sites' })
    .locator('xpath=ancestor::*[.//ul][1]')
  // Stale sections say so and poll; wait for real rows.
  const firstRow = card.getByRole('listitem').first()
  await expect(firstRow).toBeVisible({ timeout: 60_000 })
  const text = (await firstRow.innerText()).replace(/\s+/g, ' ').trim()
  const count = parseCount(text.match(/(\d[\d,]*)$/)?.[1] ?? '')

  await expectCount(
    testInfo,
    'visits to the top site, last 30 days',
    count,
    top,
  )
  expect(
    leaders.some((host) => text.includes(host)),
    `${text} should name one of ${leaders.join(', ')}`,
  ).toBe(true)
})
