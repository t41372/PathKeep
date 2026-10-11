/**
 * Insights in a time zone of their own (docs/architecture/insights-time-zone.md).
 *
 * Proves:
 * - Choosing a zone in Settings builds Insights again in it: a day then
 *   holds the fixture's visits of that day in that zone, counted here by
 *   hand, and its times say the zone, because the computer (this browser,
 *   in UTC) is elsewhere. No notice, since the user chose it here.
 * - On a computer in another zone (a browser in Los Angeles) Insights say so;
 *   keeping Insights' zone hides the notice, also after a reload.
 * - When the computer moves again (Berlin) the notice is back, and switching
 *   to the computer's zone builds Insights in it: the day holds Berlin's
 *   visits, and its times need no zone.
 *
 * Runs first among the specs that change the archive, so the archive holds
 * exactly the fixture's visits, and ends in UTC, where the other specs count
 * days.
 */
import {
  expect,
  test,
  type Browser,
  type Page,
  type TestInfo,
} from '@playwright/test'
import { archivedVisits, expectCount, parseCount } from './support/fixture'

const REBUILD_TIMEOUT = 120_000
const TAIPEI = 'Asia/Taipei'
const LOS_ANGELES = 'America/Los_Angeles'
const BERLIN = 'Europe/Berlin'

/** The day the instant falls on in `zone`. */
function dayIn(ms: number, zone: string) {
  return new Intl.DateTimeFormat('en-CA', {
    timeZone: zone,
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
  }).format(ms)
}

function visitsOn(day: string, zone: string) {
  return archivedVisits().filter((visit) => dayIn(visit.at, zone) === day)
    .length
}

/**
 * A recent day whose visits differ most between Taipei's day and UTC's, so
 * the count shows which zone Insights used.
 */
function telltaleDay() {
  let best = { day: '', difference: -1 }
  for (let back = 3; back <= 12; back += 1) {
    const day = dayIn(Date.now() - back * 86_400_000, TAIPEI)
    const difference = Math.abs(visitsOn(day, TAIPEI) - visitsOn(day, 'UTC'))
    if (difference > best.difference) best = { day, difference }
  }
  expect(best.difference, 'a day the zone changes').toBeGreaterThan(0)
  return best.day
}

async function openIn(browser: Browser, testInfo: TestInfo, zone: string) {
  const context = await browser.newContext({
    baseURL: testInfo.project.use.baseURL,
    viewport: { width: 1440, height: 900 },
    locale: 'en-US',
    timezoneId: zone,
  })
  return { context, page: await context.newPage() }
}

async function chooseZone(page: Page, search: string, place: RegExp) {
  await page.goto('/#/settings/general')
  await page.getByRole('combobox', { name: 'Insights time zone' }).click()
  await page.getByPlaceholder('Search for a city or zone').fill(search)
  await page.getByRole('option', { name: place }).first().click()
}

/**
 * Settings shows the chosen zone at once, and says Insights are being built
 * again in it until they are.
 */
async function waitForRebuild(page: Page, place: string) {
  await page.goto('/#/settings/general')
  await expect(
    page.getByRole('combobox', { name: 'Insights time zone' }),
  ).toContainText(place)
  await expect(page.getByText(/^Building Insights again in/)).toBeHidden({
    timeout: REBUILD_TIMEOUT,
  })
}

async function dayVisits(page: Page, day: string) {
  await page.goto(`/#/insights/day/${day}`)
  const tile = page.getByRole('group', { name: 'Page visits', exact: true })
  await expect(tile).toHaveAttribute('aria-busy', 'false', {
    timeout: REBUILD_TIMEOUT,
  })
  const lines = (await tile.innerText()).split('\n').map((line) => line.trim())
  return parseCount(lines[1] ?? '')
}

const notice = (page: Page) => page.getByText(/^Insights use /)

test('Insights keep their own time zone until the user switches', async ({
  page,
  browser,
}, testInfo) => {
  test.setTimeout(600_000)
  const day = telltaleDay()

  // Chosen in Settings: Insights are built again in Taipei.
  await chooseZone(page, 'Taipei', /^Taipei/)
  await expect(
    page.getByText(/^Building Insights again in .*GMT\+8/),
  ).toBeVisible()
  await waitForRebuild(page, 'Taipei')
  await expectCount(
    testInfo,
    `visits on ${day} in Taipei`,
    await dayVisits(page, day),
    visitsOn(day, TAIPEI),
  )
  // The browser is in UTC, so Insights' times carry their zone.
  await expect(page.getByText(/GMT\+8/).first()).toBeVisible()
  await page.goto('/#/insights')
  await expect(page.getByRole('heading', { name: 'Insights' })).toBeVisible()
  await expect(notice(page)).toHaveCount(0)

  // A computer in Los Angeles: the notice, and keeping Taipei hides it.
  const losAngeles = await openIn(browser, testInfo, LOS_ANGELES)
  try {
    await losAngeles.page.goto('/#/insights')
    await expect(notice(losAngeles.page)).toHaveText(
      /^Insights use .*GMT\+8.*; this computer is now on Pacific Time/,
      { timeout: REBUILD_TIMEOUT },
    )
    await losAngeles.page.getByRole('button', { name: /^Keep / }).click()
    await expect(notice(losAngeles.page)).toHaveCount(0)
    await losAngeles.page.reload()
    await expect(
      losAngeles.page.getByRole('heading', { name: 'Insights' }),
    ).toBeVisible()
    await expect(notice(losAngeles.page)).toHaveCount(0)
  } finally {
    await losAngeles.context.close()
  }

  // Then Berlin: the notice is back; switching builds Insights there.
  const berlin = await openIn(browser, testInfo, BERLIN)
  try {
    await berlin.page.goto('/#/insights')
    await expect(notice(berlin.page)).toHaveText(
      /this computer is now on Central European Time/,
      { timeout: REBUILD_TIMEOUT },
    )
    await berlin.page.getByRole('button', { name: /^Use / }).click()
    await expect(notice(berlin.page)).toHaveCount(0)
    await waitForRebuild(berlin.page, 'Berlin')
    await expectCount(
      testInfo,
      `visits on ${day} in Berlin`,
      await dayVisits(berlin.page, day),
      visitsOn(day, BERLIN),
    )
    await expect(berlin.page.getByText(/GMT[+-]\d/)).toHaveCount(0)
  } finally {
    await berlin.context.close()
  }

  // Back to UTC for the specs after this one.
  await chooseZone(page, 'UTC', /^UTC/)
  await waitForRebuild(page, 'UTC')
  await expectCount(
    testInfo,
    `visits on ${day} in UTC`,
    await dayVisits(page, day),
    visitsOn(day, 'UTC'),
  )
})
