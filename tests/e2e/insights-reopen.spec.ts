/**
 * Insights cleared in Settings → Background work are built again the next
 * time the app opens the archive.
 *
 * Proves: after "Clear Insights data" nothing builds them again by itself
 * (no work runs, they stay "Not built yet"); once the app is killed and
 * started again (`support/app.ts`), opening the archive builds them, and
 * Insights count every visit the archive holds again.
 */
import { expect, test, type Page } from '@playwright/test'
import { killAndRestartApp, unlockIfLocked } from './support/app'
import { backend } from './support/fixture'

const compact = new Intl.NumberFormat('en', {
  notation: 'compact',
  maximumFractionDigits: 1,
})

function insightsRow(page: Page) {
  return page
    .getByText('Insights and page details', { exact: true })
    .locator('xpath=ancestor::div[contains(@class,"rounded-xl")][1]')
}

const visitDetails = (page: Page) =>
  page
    .getByRole('list', { name: 'Insights data' })
    .locator('li[data-module="visit-derived-facts"]')

test('Insights cleared in Background work are built again when the app opens next', async ({
  page,
}) => {
  test.setTimeout(600_000)
  await page.goto('/#/settings/background')
  await unlockIfLocked(page)
  await expect(insightsRow(page).getByText('Nothing running.')).toBeVisible({
    timeout: 120_000,
  })

  await page.getByRole('button', { name: 'Clear…' }).click()
  const dialog = page.getByRole('alertdialog', {
    name: 'Clear Insights data?',
  })
  await dialog.getByRole('button', { name: 'Clear', exact: true }).click()
  await expect(
    page.getByText(
      'Insights data cleared. Insights are empty until they are built again.',
    ),
  ).toBeVisible()
  await expect(visitDetails(page)).toContainText('Not built yet')
  // Left alone, nothing builds them.
  await page.waitForTimeout(5_000)
  await page.reload()
  await unlockIfLocked(page)
  await expect(visitDetails(page)).toContainText('Not built yet')
  await expect(insightsRow(page).getByText('Nothing running.')).toBeVisible()

  await killAndRestartApp()
  await page.reload()
  await unlockIfLocked(page)
  await page.goto('/#/settings/background')
  await expect(visitDetails(page)).toContainText('Up to date', {
    timeout: 300_000,
  })

  // Insights count the whole archive again.
  const { totalVisits } = await backend<{ totalVisits: number }>(
    'load_dashboard_snapshot',
  )
  await page.goto('/#/insights')
  await expect(
    page.getByRole('group', { name: 'Page visits', exact: true }),
  ).toContainText(compact.format(totalVisits), { timeout: 120_000 })
})
