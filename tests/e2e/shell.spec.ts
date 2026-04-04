import { expect, test } from '@playwright/test'

test('loads the setup workspace in browser preview mode', async ({ page }) => {
  await page.goto('/')

  await expect(
    page.getByRole('heading', { name: 'browser history backup' }),
  ).toBeVisible()
  await expect(page.getByRole('heading', { name: 'Setup' })).toBeVisible()
  await expect(
    page.getByText(
      'Work through the source, storage, schedule, and review steps. Every system action still exposes Preview, Manual, and Apply paths.',
    ),
  ).toBeVisible()
  await expect(
    page.getByRole('button', { name: 'Preview native schedule' }),
  ).toBeVisible()
})
