/**
 * Search ranking, as recall.md states it: by score, then by the newest
 * matching visit.
 *
 * Proves: a word in one page's title lists that page above a page with the
 * word only in its address, although the other page was visited more often
 * and more recently; two pages with the same title list the one visited last
 * first.
 *
 * The pages (`support/ranking-pages.ts`) go into Chrome's history and are
 * backed up like any other browsing.
 */
import { expect, test, type Page } from '@playwright/test'
import { appendChromeVisits } from '../../scripts/fixtures/synthetic-browsers.mjs'
import { fixture } from './support/fixture'
import {
  IN_ADDRESS,
  IN_TITLE,
  NEWER_TWIN,
  OLDER_TWIN,
  RANKING_PAGES,
  rankingVisitCount,
} from './support/ranking-pages'

/** Searches History and waits for exactly these pages, in this order. */
async function expectOrder(page: Page, query: string, titles: string[]) {
  await page.goto('/#/history')
  await page.getByRole('searchbox', { name: 'Search history' }).fill(query)
  await expect(
    page.getByRole('listbox', { name: 'History results' }).getByRole('option'),
  ).toHaveText(
    titles.map((title) => new RegExp(title)),
    { timeout: 30_000 },
  )
}

test('a title match ranks above an address match; the same title, newest first', async ({
  page,
}) => {
  const now = Date.now()
  const visits = RANKING_PAGES.flatMap((ranked) =>
    ranked.ago.map((ago) => ({
      url: ranked.url,
      title: ranked.title,
      at: now - ago,
      typed: false,
      duration: 30_000,
    })),
  ).sort((a, b) => a.at - b.at)
  appendChromeVisits(fixture().files['chrome:Default'], visits)
  await page.goto('/#/backup')
  await page.getByRole('button', { name: 'Back up now' }).first().click()
  await expect(
    page
      .getByRole('heading', { name: 'Recent runs' })
      .locator('xpath=ancestor::*[.//li][1]')
      .getByRole('listitem')
      .first(),
  ).toContainText(new RegExp(`\\b${rankingVisitCount()} new visits`), {
    timeout: 60_000,
  })

  await expectOrder(page, 'lighthouse', [IN_TITLE.title, IN_ADDRESS.title])
  await expectOrder(page, 'ferry timetable', [
    NEWER_TWIN.title,
    OLDER_TWIN.title,
  ])
})
