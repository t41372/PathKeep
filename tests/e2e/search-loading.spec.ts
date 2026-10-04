/**
 * History while a search is still on its way.
 *
 * Proves: the timeline's visit rows are never shown as search results while
 * the first page of a search loads. They used to be: the list kept the
 * previous query's rows on screen, so a page visited twice in the newest
 * hundred visits showed up as two "results" until the search answered.
 *
 * The search request is held at the transport (`page.route` on the dev IPC
 * bridge), so the in-between state lasts as long as the test needs.
 */
import { expect, test } from '@playwright/test'

test('a loading search never shows the timeline as its results', async ({
  page,
}) => {
  await page.goto('/#/history')
  const timeline = page
    .getByRole('listbox', { name: 'History timeline' })
    .getByRole('option')
  await expect(timeline.first()).toBeVisible()

  let release = () => {}
  const held = new Promise<void>((resolve) => (release = resolve))
  let searched = () => {}
  const searchSent = new Promise<void>((resolve) => (searched = resolve))
  await page.route('**/commands/query_history', async (route) => {
    const body = route.request().postDataJSON() as {
      query?: { groupByUrl?: boolean }
    }
    if (body.query?.groupByUrl) {
      searched()
      await held
    }
    await route.continue()
  })

  await page
    .getByRole('searchbox', { name: 'Search history' })
    .fill('why I left tokio')
  await searchSent
  const results = page
    .getByRole('listbox', { name: 'History results' })
    .getByRole('option')
  await expect(results).toHaveCount(0)

  release()
  await expect(
    results.filter({ hasText: 'Hacker News · Why I left tokio' }),
  ).toHaveCount(1)
})
