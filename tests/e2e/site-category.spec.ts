/**
 * A site's kind, set by the user (`WORK-SITE-CATEGORY-REVIEW`).
 *
 * Proves: "Kinds of sites" opens the sites on no list, most visited first,
 * and leads to one's page. Choosing a kind there builds Insights again with
 * it: the card shows that kind with the site's visits (counted from the
 * fixture by hand) and "Not classified" with that many fewer, and the site is
 * no longer among the unclassified ones. Going back to PathKeep's list puts
 * everything back as it was. A part of a site the lists sort on their own
 * (Hacker News in ycombinator.com) is listed on the site's page and keeps its
 * kind when the site gets another; a site no one changed never says it is
 * being updated.
 *
 * Changes the archive's Insights and puts them back, so it runs with the
 * specs that change the archive and leaves nothing behind.
 */
import { expect, test, type Page } from '@playwright/test'
import { archivedVisits, backend } from './support/fixture'
import { siteOf } from './support/public-suffix'
import { kindOf } from './support/site-kinds'

const card = (page: Page, title: string) =>
  page
    .getByRole('heading', { name: title, exact: true })
    .locator('xpath=ancestor::section[1]')

/** The fixture site on no list with the most visits, all of them unclassified. */
function unlistedSite() {
  const visits = new Map<string, number>()
  const mixed = new Set<string>()
  for (const visit of archivedVisits()) {
    const site = siteOf(visit.url)
    if (kindOf(visit) === 'Not classified')
      visits.set(site, (visits.get(site) ?? 0) + 1)
    else mixed.add(site)
  }
  const [site, count] = [...visits]
    .filter(([site]) => !mixed.has(site))
    .sort((a, b) => b[1] - a[1])[0]
  return { site, count }
}

interface Site {
  data: { categoryPending: boolean }
}

/** Insights are built with the category set for `site` now. */
async function built(site: string) {
  await expect
    .poll(
      async () =>
        (
          await backend<Site>('get_domain_deep_dive', {
            request: {
              dateRange: { start: '1970-01-01', end: '2100-01-01' },
              registrableDomain: site,
            },
          })
        ).data.categoryPending,
      { timeout: 120_000 },
    )
    .toBe(false)
}

interface Mix {
  data: { categories: { domainCategory: string; visitCount: number }[] }
}

/** Visits per kind of site over all time, as the backend counts them. */
async function mix() {
  const result = await backend<Mix>('get_activity_mix', {
    request: { dateRange: { start: '1970-01-01', end: '2100-01-01' } },
  })
  return new Map(
    result.data.categories.map((entry) => [
      entry.domainCategory,
      entry.visitCount,
    ]),
  )
}

const percent = new Intl.NumberFormat('en', {
  style: 'percent',
  maximumFractionDigits: 1,
})

test('a site on no list gets a kind from its page, and gives it back', async ({
  page,
}) => {
  test.setTimeout(300_000)
  const { site, count } = unlistedSite()
  const before = await mix()
  const total = [...before.values()].reduce((sum, visits) => sum + visits, 0)
  const unclassified = before.get('unknown') ?? 0
  expect(unclassified).toBeGreaterThan(count)
  expect(before.has('news'), 'no fixture site is news').toBe(false)

  // "Not classified" opens the sites on no list; the site is there.
  await page.goto('/#/insights')
  const kinds = card(page, 'Kinds of sites')
  await kinds
    .getByRole('button', { name: 'Show the sites that are not classified' })
    .click({ timeout: 60_000 })
  const list = page.getByRole('dialog', { name: 'Sites not on any list' })
  const entry = list.getByRole('link', { name: new RegExp(`^${site}`) })
  await expect(entry).toBeVisible()
  await entry.click()
  await expect(page).toHaveURL(new RegExp(`#/insights/site/${site}`))

  // Its kind, chosen on its page.
  await page.getByRole('combobox', { name: 'Kind of site' }).click()
  await page.getByRole('option', { name: 'News', exact: true }).click()
  await expect(page.getByRole('combobox', { name: 'Kind of site' })).toHaveText(
    'News',
  )
  // Insights built again: the site's visits moved from not classified to news.
  await expect
    .poll(async () => (await mix()).get('news') ?? 0, { timeout: 120_000 })
    .toBe(count)
  expect((await mix()).get('unknown')).toBe(unclassified - count)
  await expect(page.getByText('Updating Insights…')).toBeHidden({
    timeout: 30_000,
  })

  // The card says so.
  await page.goto('/#/insights')
  await expect(
    kinds.getByRole('listitem').filter({ hasText: 'News' }),
  ).toHaveText(new RegExp(`News\\s*${percent.format(count / total)}`), {
    timeout: 60_000,
  })
  await kinds
    .getByRole('button', { name: 'Show the sites that are not classified' })
    .click()
  await expect(list.getByRole('link').first()).toBeVisible()
  await expect(
    list.getByRole('link', { name: new RegExp(`^${site}`) }),
  ).toHaveCount(0)
  await page.keyboard.press('Escape')

  // Back to PathKeep's list: as it was.
  await page.goto(`/#/insights/site/${site}`)
  await page.getByRole('combobox', { name: 'Kind of site' }).click()
  await page
    .getByRole('option', { name: "PathKeep's list: Not classified" })
    .click()
  await expect.poll(mix, { timeout: 120_000 }).toEqual(before)
})

test('the parts of a site the lists sort on their own keep their kinds', async ({
  page,
}) => {
  test.setTimeout(300_000)
  const hn = archivedVisits().filter(
    (visit) => !visit.term && siteOf(visit.url) === 'ycombinator.com',
  )
  expect(hn.length, 'the fixture visited Hacker News').toBeGreaterThan(0)
  expect(hn.every((visit) => kindOf(visit) === 'Forums and Q&A')).toBe(true)
  const before = await mix()

  // A site no one changed, whose pages a path rule sorts: nothing to update.
  await page.goto('/#/insights/site/mio-rs.github.io')
  await expect(
    page.getByRole('combobox', { name: 'Kind of site' }),
  ).toBeVisible({ timeout: 60_000 })
  await expect(page.getByText('Updating Insights…')).toBeHidden()

  await page.goto('/#/insights/site/ycombinator.com')
  await page.getByRole('button', { name: '1 part sorted on its own' }).click()
  const part = page
    .getByRole('dialog')
    .getByRole('listitem')
    .filter({ hasText: 'news.ycombinator.com' })
  await expect(part).toContainText('Forums and Q&A')
  await page.keyboard.press('Escape')

  // ycombinator.com is news; Hacker News stays a forum.
  await page.getByRole('combobox', { name: 'Kind of site' }).click()
  await page.getByRole('option', { name: 'News', exact: true }).click()
  await expect(page.getByRole('combobox', { name: 'Kind of site' })).toHaveText(
    'News',
  )
  await built('ycombinator.com')
  expect(await mix()).toEqual(before)
  await expect(page.getByText('Updating Insights…')).toBeHidden({
    timeout: 30_000,
  })

  await page.getByRole('combobox', { name: 'Kind of site' }).click()
  await page.getByRole('option', { name: /^PathKeep's list: / }).click()
  await built('ycombinator.com')
  expect(await mix()).toEqual(before)
})
