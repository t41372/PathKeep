import { expect, test } from '@playwright/test'

test('walks through onboarding, first backup, explorer, and audit in browser preview', async ({
  page,
}) => {
  await page.goto('/')

  await expect(page.getByRole('heading', { name: 'Dashboard' })).toBeVisible()
  await expect(
    page.getByText('The first archive run still needs review'),
  ).toBeVisible()

  await page.getByRole('link', { name: 'Review onboarding' }).click()

  await expect(
    page.getByRole('heading', { name: 'Onboarding / Setup' }),
  ).toBeVisible()
  await expect(page.getByText('Profiles are the backup boundary')).toBeVisible()

  await page
    .getByRole('textbox', { name: 'MASTER PASSWORD', exact: true })
    .fill('vault-passphrase')
  await page
    .getByRole('textbox', { name: 'CONFIRM PASSWORD', exact: true })
    .fill('vault-passphrase')
  await page
    .getByRole('button', { name: 'Initialize + run first backup' })
    .click()

  await expect(page.getByText('RECENT RUNS')).toBeVisible()
  await expect(page.getByRole('link', { name: 'Detail' })).toBeVisible()

  await page.getByRole('link', { name: 'Explorer', exact: true }).click()

  await expect(
    page.getByRole('heading', { name: 'History Explorer' }),
  ).toBeVisible()
  await expect(
    page.getByRole('button', {
      name: /SQLite inspection in browser developer tools/i,
    }),
  ).toBeVisible()

  await page.getByRole('button', { name: 'jsonl' }).click()
  await expect(page.getByText(/pathkeep-export-/)).toBeVisible()

  await page.getByRole('link', { name: 'Audit Ledger' }).click()

  await expect(page.getByText('RUN LEDGER')).toBeVisible()
  await expect(page.getByText('ARTIFACTS')).toBeVisible()
})
