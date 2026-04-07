import { expect, test } from '@playwright/test'

test('loads the rewritten shell, onboarding, and dashboard preview', async ({
  page,
}) => {
  await page.goto('/')

  await expect(page.getByRole('heading', { name: 'Dashboard' })).toBeVisible()
  await expect(page.getByText('RECENT RUNS')).toBeVisible()
  await expect(page.getByText('PATHKEEP')).toBeVisible()

  await page.getByRole('link', { name: 'Review onboarding' }).click()

  await expect(
    page.getByRole('heading', { name: 'Onboarding / Setup' }),
  ).toBeVisible()
  await expect(
    page.getByRole('button', { name: 'Preview native schedule' }),
  ).toBeVisible()

  await page.getByRole('link', { name: 'Open dashboard preview' }).click()

  await expect(page.getByRole('button', { name: 'Backup Now' })).toBeVisible()
})
