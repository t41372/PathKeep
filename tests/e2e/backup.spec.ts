/**
 * Backup: new browsing shows up after the next backup, and a paused source
 * is left alone until it is turned back on.
 *
 * Proves: a backup appends exactly the visits added since the last one (no
 * re-import, no loss), a paused profile contributes nothing even when it
 * has new visits, and turning it back on catches up exactly those visits.
 */
import { expect, test, type Page } from '@playwright/test'
import {
  appendChromeVisits,
  appendFirefoxVisits,
} from '../../scripts/fixtures/synthetic-browsers.mjs'
import { fixture } from './support/fixture'

const NEW_URL = 'https://e2e.example/backup-check'
const NEW_TITLE = 'Backup check page'

function visitsNow(count: number, offsetMs = 0) {
  const now = Date.now() + offsetMs
  return Array.from({ length: count }, (_, index) => ({
    url: NEW_URL,
    title: NEW_TITLE,
    at: now - (count - index) * 60_000,
    typed: false,
    duration: 30_000,
  }))
}

async function backUp(page: Page, expected: RegExp) {
  await page.goto('/#/backup')
  await page.getByRole('button', { name: 'Back up now' }).first().click()
  const latest = page
    .locator('section, div')
    .filter({ has: page.getByRole('heading', { name: 'Recent runs' }) })
    .last()
    .getByRole('listitem')
    .first()
  await expect(latest).toContainText(expected, { timeout: 60_000 })
}

async function searchCount(page: Page) {
  await page.goto('/#/history')
  await page.getByRole('searchbox', { name: 'Search history' }).fill(NEW_TITLE)
  const count = page.getByText(/^[\d,]+ results? · Full text$|^Nothing found$/)
  await expect(count).toBeVisible()
  const text = await count.innerText()
  return text === 'Nothing found' ? 0 : Number(text.replace(/[^\d]/g, ''))
}

test('new visits arrive with the next backup; a paused source waits', async ({
  page,
}) => {
  const { files } = fixture()
  expect(await searchCount(page)).toBe(0)

  appendChromeVisits(files['chrome:Default'], visitsNow(3))
  appendFirefoxVisits(files['firefox:k3x9.default-release'], visitsNow(2))
  await backUp(page, /\b5 new visits\b/)
  expect(await searchCount(page)).toBe(5)

  // Pause Firefox, browse some more in it, back up: nothing new.
  await page.goto('/#/backup')
  const firefox = page.getByRole('switch', {
    name: 'Back up Firefox default-release',
  })
  await firefox.click()
  await expect(firefox).not.toBeChecked()
  appendFirefoxVisits(
    files['firefox:k3x9.default-release'],
    visitsNow(4, 1_000),
  )
  await backUp(page, /\b0 new visits\b/)
  expect(await searchCount(page)).toBe(5)

  // Turn it back on: exactly the four waiting visits come in.
  await page.goto('/#/backup')
  await firefox.click()
  await expect(firefox).toBeChecked()
  await backUp(page, /\b4 new visits\b/)
  expect(await searchCount(page)).toBe(9)
})
