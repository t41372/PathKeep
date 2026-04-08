import { expect, test, type Page } from '@playwright/test'

async function completePreviewOnboarding(page: Page) {
  await page.goto('/')

  await expect(page.getByRole('heading', { name: 'Dashboard' })).toBeVisible()
  await expect(
    page.getByText('The first archive run still needs review'),
  ).toBeVisible()

  await page.getByRole('link', { name: 'Open onboarding flow' }).click()

  await expect(page.getByTestId('onboarding-page')).toBeVisible()
  await page.getByRole('button', { name: /Begin Setup/ }).click()
  await page.getByRole('button', { name: /Continue/ }).click()
  await page.getByRole('button', { name: /Continue/ }).click()
  await page.getByPlaceholder('Enter master password').fill('vault-passphrase')
  await page.getByPlaceholder('Confirm password').fill('vault-passphrase')
  await page.getByRole('button', { name: /Continue/ }).click()
  await page.getByRole('button', { name: /Continue/ }).click()
  await page
    .getByRole('button', { name: 'Initialize + First Backup →' })
    .click()

  await expect(page.getByText('RECENT RUNS')).toBeVisible()
}

test('walks through onboarding, first backup, explorer, and audit in browser preview', async ({
  page,
}) => {
  await completePreviewOnboarding(page)

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

  const auditPage = page.getByTestId('audit-page')
  await expect(auditPage).toBeVisible()
  await expect(auditPage.getByText('MANIFEST CHAIN')).toBeVisible()
  await expect(auditPage.getByText(/ARTIFACTS ·/)).toBeVisible()
})

test('keeps schedule and security review surfaces inspectable in browser preview', async ({
  page,
}) => {
  await completePreviewOnboarding(page)

  await page.getByRole('link', { name: 'Schedule', exact: true }).click()
  await expect(page.getByTestId('schedule-page')).toBeVisible()
  await expect(
    page.getByTestId('schedule-page').getByText('BACKUP SCHEDULE'),
  ).toBeVisible()
  await page.getByRole('button', { name: 'Execute' }).click()
  await expect(
    page.getByText(
      'launchctl bootstrap gui/501 dev.codex.pathkeep.backup.plist',
    ),
  ).toBeVisible()

  await page.getByRole('link', { name: 'Security', exact: true }).click()
  await expect(page.getByText('ENCRYPTION STATUS')).toBeVisible()
  await expect(page.getByText('Archive is Encrypted')).toBeVisible()
})

test('surfaces intelligence routes and degraded states after the first backup', async ({
  page,
}) => {
  await completePreviewOnboarding(page)

  await page.getByRole('link', { name: 'Explorer', exact: true }).click()
  await page.getByRole('button', { name: 'hybrid' }).click()
  await expect(page.getByText('SEMANTIC RECALL')).toBeVisible()
  await expect(page.getByText('AI disabled')).toBeVisible()

  await page.getByRole('link', { name: 'AI Assistant', exact: true }).click()
  await expect(page.getByText('Assistant is currently disabled')).toBeVisible()

  await page.getByRole('link', { name: 'Insights', exact: true }).click()
  await expect(page.getByText('INSIGHT CARDS')).toBeVisible()
  await expect(page.getByText('AI disabled')).toBeVisible()
})
