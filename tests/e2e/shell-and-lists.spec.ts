/**
 * The window's back and forward, History's timeline rows, Backup's sources
 * and the fading edge of Insights' lists (`WORK-UI-POLISH-2026-10`).
 *
 * Proves:
 * - Back and forward step through the screens visited, from the arrows at
 *   the top of the nav rail and from the keyboard, and forward is off when
 *   there is nothing ahead.
 * - A page opened several times in a row is one timeline row marked ×N; its
 *   details list every time, each can be chosen, and a link to one of them
 *   (`?visit=`) selects that row again after a reload. The row's time fits
 *   on one line.
 * - Backup lists Chrome's two profiles under one row that says how many
 *   there are and how many are backed up, and folds away; Firefox's single
 *   profile is a row of its own.
 * - A list that scrolls inside its card fades by a mask on the list itself,
 *   not a layer painted over it.
 *
 * The window buttons drawn on Windows and Linux are not here: the browser
 * this runs in is no desktop window (`drawsWindowControls`).
 */
import { expect, test, type Page } from '@playwright/test'
import { unlockIfLocked } from './support/app'
import { archivedVisits, backend, localDay } from './support/fixture'

const SESSION_GAP_MS = 30 * 60_000
const mac = process.platform === 'darwin'

/** The most recent page the fixture opened several times in a row, as the timeline merges it. */
function latestRepeat() {
  const visits = [...archivedVisits()].sort((a, b) => b.at - a.at)
  for (let index = 0; index < visits.length; ) {
    let end = index + 1
    while (
      end < visits.length &&
      visits[end].url === visits[index].url &&
      visits[end - 1].at - visits[end].at <= SESSION_GAP_MS &&
      localDay(visits[end].at) === localDay(visits[index].at)
    )
      end += 1
    if (end - index > 1) return visits.slice(index, end)
    index = end
  }
  throw new Error('the fixture opens no page twice in a row')
}

const step = (page: Page, name: 'Back' | 'Forward') =>
  page
    .getByRole('navigation', { name: 'Main navigation' })
    .getByRole('button', { name: new RegExp(`^${name}`) })

test('back and forward step through the screens visited', async ({ page }) => {
  await page.goto('/#/')
  await unlockIfLocked(page)
  const nav = page.getByRole('navigation', { name: 'Main navigation' })
  await nav.getByRole('link', { name: 'History' }).click()
  await expect(page).toHaveURL(/#\/history/)
  await nav.getByRole('link', { name: 'Insights' }).click()
  await expect(page).toHaveURL(/#\/insights/)
  await expect(step(page, 'Forward')).toHaveAttribute('aria-disabled', 'true')

  await step(page, 'Back').click()
  await expect(page).toHaveURL(/#\/history/)
  await page.keyboard.press(mac ? 'Meta+BracketLeft' : 'Alt+ArrowLeft')
  await expect(page).toHaveURL(/#\/$/)

  await step(page, 'Forward').click()
  await expect(page).toHaveURL(/#\/history/)
  await page.keyboard.press(mac ? 'Meta+BracketRight' : 'Alt+ArrowRight')
  await expect(page).toHaveURL(/#\/insights/)
  await expect(step(page, 'Forward')).toHaveAttribute('aria-disabled', 'true')
  await expect(step(page, 'Back')).toHaveAttribute('aria-disabled', 'false')
})

test('a page opened several times in a row is one timeline row, each time still reachable', async ({
  page,
}, testInfo) => {
  const run = latestRepeat()
  await testInfo.attach('the run', {
    body: JSON.stringify(run, null, 2),
    contentType: 'application/json',
  })
  await page.goto(`/#/history?date=${localDay(run[0].at)}`)
  await unlockIfLocked(page)
  const rows = page
    .getByRole('listbox', { name: /timeline/i })
    .getByRole('option')
    .filter({ hasText: run[0].title })
    .filter({ hasText: `×${run.length}` })
  await expect(rows.first()).toBeVisible({ timeout: 30_000 })
  const row = rows.first()

  // Its time on one line, however the language writes it.
  const time = row.locator('span').first()
  const height = await time.evaluate(
    (element) => element.getBoundingClientRect().height,
  )
  const line = await time.evaluate((element) =>
    Number.parseFloat(getComputedStyle(element).lineHeight),
  )
  expect(height, 'the time does not wrap').toBeLessThanOrEqual(line + 1)

  await row.click()
  const detail = page.getByRole('complementary', { name: 'Page details' })
  await expect(
    detail.getByText(`Opened ${run.length} times in a row here:`),
  ).toBeVisible()
  const times = detail
    .getByText(`Opened ${run.length} times in a row here:`)
    .locator('xpath=..')
    .getByRole('button')
  await expect(times).toHaveCount(run.length)
  await expect(times.first()).toHaveAttribute('aria-current', 'true')

  // The oldest of them, chosen: the same row stays selected, and a link to
  // that visit brings it back.
  await times.last().click()
  await expect(times.last()).toHaveAttribute('aria-current', 'true')
  await expect(row).toHaveAttribute('aria-selected', 'true')
  const url = page.url()
  expect(url).toMatch(/visit=\d+/)
  await page.reload()
  await expect(
    page
      .getByRole('listbox', { name: /timeline/i })
      .getByRole('option', { selected: true }),
  ).toContainText(`×${run.length}`, { timeout: 30_000 })
  await expect(
    page
      .getByRole('complementary', { name: 'Page details' })
      .getByText(`Opened ${run.length} times in a row here:`)
      .locator('xpath=..')
      .getByRole('button')
      .last(),
  ).toHaveAttribute('aria-current', 'true')
})

test('Backup groups a browser’s profiles under one row that folds away', async ({
  page,
}) => {
  await page.goto('/#/backup')
  await unlockIfLocked(page)
  const sources = page
    .getByRole('heading', { name: 'Sources' })
    .locator('xpath=ancestor::section[1]')
  const chrome = sources.getByRole('button', {
    name: /^Hide the profiles of .*Chrome/,
  })
  await expect(chrome).toHaveAttribute('aria-expanded', 'true')
  const { config, browserProfiles } = await backend<{
    config: { selectedProfileIds: string[] }
    browserProfiles: { profileId: string; browserName: string }[]
  }>('app_snapshot')
  const chromeProfiles = browserProfiles.filter((profile) =>
    profile.browserName.includes('Chrome'),
  )
  expect(chromeProfiles).toHaveLength(2)
  const on = chromeProfiles.filter((profile) =>
    config.selectedProfileIds.includes(profile.profileId),
  ).length
  await expect(chrome).toContainText('2 profiles')
  await expect(chrome).toContainText(`${on} of 2 backed up`)
  const profileSwitches = sources.getByRole('switch', { name: /Chrome/ })
  await expect(profileSwitches).toHaveCount(2)

  await chrome.click()
  await expect(
    sources.getByRole('button', { name: /^Show the profiles of .*Chrome/ }),
  ).toHaveAttribute('aria-expanded', 'false')
  await expect(profileSwitches).toHaveCount(0)
  // Firefox has one profile: a row of its own, with its switch.
  await expect(sources.getByRole('switch', { name: /Firefox/ })).toHaveCount(1)
  await sources
    .getByRole('button', { name: /^Show the profiles of .*Chrome/ })
    .click()
  await expect(profileSwitches).toHaveCount(2)
})

test('a list that scrolls in its card fades by a mask on the list, not a layer over it', async ({
  page,
}) => {
  await page.goto('/#/insights')
  await unlockIfLocked(page)
  const lists = page.locator('[style*="mask-image"]')
  await expect(lists.first()).toBeVisible({ timeout: 60_000 })
  const mask = await lists
    .first()
    .evaluate((element) => getComputedStyle(element).maskImage)
  expect(mask).toContain('linear-gradient')
  await expect(page.locator('.from-card.to-transparent')).toHaveCount(0)
})
