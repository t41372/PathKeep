/**
 * History search at its limits, made small: the E2E backend runs with a
 * keyword window of 40 URLs (`PATHKEEP_DEBUG_SEARCH_WINDOW` in
 * playwright.config.ts) instead of the release build's 25,000.
 *
 * Proves: a word that matches more URLs than one search ranks shows an "N+"
 * count, never an exact one, with one plain line saying not every match was
 * ranked and that another word narrows it.
 */
import { expect, test } from '@playwright/test'
import { archivedProfiles, fixture, parseCount } from './support/fixture'

const WINDOW = Number(process.env.PATHKEEP_DEBUG_SEARCH_WINDOW)

test('a word on more pages than one search ranks says so', async ({ page }) => {
  // Every URL has "https" in it; each browser profile keeps its own copy.
  const copies = new Set(
    archivedProfiles.flatMap((id) =>
      fixture().visitsByProfile[id].map((visit) => `${id} ${visit.url}`),
    ),
  )
  expect(WINDOW, 'the E2E config sets a small window').toBeGreaterThan(0)
  expect(
    copies.size,
    'the word must match more than the window',
  ).toBeGreaterThan(WINDOW)

  await page.goto('/#/history')
  await page.getByRole('searchbox', { name: 'Search history' }).fill('https')

  const header = page.getByText(/^[\d,]+\+ pages · Full text$/)
  await expect(header).toBeVisible()
  const shown = parseCount((await header.innerText()).split('+')[0])
  expect(shown).toBeGreaterThan(0)
  expect(shown).toBeLessThanOrEqual(WINDOW)
  await expect(
    page.getByText(
      'Not every match was ranked, only the most recently saved ones. Add a word to narrow the search.',
    ),
  ).toBeVisible()
  // The count stays a lower bound once the separate count query lands too.
  await expect(
    page.getByText(/^[\d,]+ pages? · [\d,]+ visits? · Full text$/),
  ).toHaveCount(0)
})
