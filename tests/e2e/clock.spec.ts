/**
 * Clock: times follow the clock chosen in Settings → General, everywhere a
 * time of day is shown, and "Match system" follows the OS (on Linux the OS
 * says nothing, so the language decides: 12-hour for English).
 *
 * Proves: the run list on Backup and the hour axis of Insights' rhythm switch
 * between "6:37 AM" and "06:37" with the setting, and the choice survives a
 * reload.
 */
import { expect, test, type Page } from '@playwright/test'

const latestRunTime = (page: Page) =>
  page
    .getByRole('heading', { name: 'Recent runs' })
    .locator('xpath=ancestor::*[.//li][1]')
    .getByRole('listitem')
    .first()

async function chooseClock(page: Page, option: RegExp) {
  await page.goto('/#/settings/general')
  await page.getByRole('combobox', { name: 'Clock' }).click()
  await page.getByRole('option', { name: option }).click()
}

// The row reads "Today 8:27 AM3 sources · …": the run's summary follows the
// time with no space, so neither pattern can end on a word boundary.
const twelveHour = /\d:\d{2}\s?[AP]M/
const twentyFourHour = /\b\d{2}:\d{2}(?!\s?[AP]M)/

test('times follow the clock chosen in Settings', async ({ page }) => {
  await chooseClock(page, /^12-hour/)
  await page.goto('/#/backup')
  await expect(latestRunTime(page)).toContainText(twelveHour)
  await page.goto('/#/insights?range=d30')
  await expect(page.getByText('12 AM', { exact: true }).first()).toBeVisible({
    timeout: 60_000,
  })

  await chooseClock(page, /^24-hour/)
  await page.goto('/#/backup')
  await expect(latestRunTime(page)).toContainText(twentyFourHour)
  await expect(latestRunTime(page)).not.toContainText(twelveHour)
  await page.reload()
  await expect(latestRunTime(page)).not.toContainText(twelveHour)

  // The Linux OS has no clock setting to read; English then uses 12 hours.
  await chooseClock(page, /^Match system/)
  await page.goto('/#/backup')
  await expect(latestRunTime(page)).toContainText(twelveHour)
})
