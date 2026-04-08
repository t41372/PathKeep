import { expect, test, type Page } from '@playwright/test'

async function completePreviewOnboarding(page: Page) {
  await page.goto('/')

  await expect(page.getByRole('heading', { name: 'Dashboard' })).toBeVisible()
  await expect(
    page.getByText('The first archive run still needs review'),
  ).toBeVisible()

  await page.getByRole('link', { name: 'Open onboarding flow' }).click()

  await expect(page.getByTestId('onboarding-page')).toBeVisible()
  await expect(
    page.getByText('Open-source — GPL v3 licensed, audit the code'),
  ).toBeVisible()
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
  await expect(page.getByText('ON THIS DAY')).toBeVisible()
  await expect(page.getByText('PERIODIC SUMMARY')).toBeVisible()
  await expect(page.getByText('Disabled', { exact: true })).toBeVisible()
  await expect(page.getByText('common.disabled')).toHaveCount(0)
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
  await expect(
    page.getByText(
      'Browser preview mode keeps schedule verification read-only. Use the desktop app for the real platform status.',
    ),
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

test('keeps shared profile scope, regex recall, and export guardrails aligned', async ({
  page,
}) => {
  await completePreviewOnboarding(page)

  await page
    .getByRole('button', { name: 'Switch profile scope' })
    .evaluate((element) => {
      ;(element as HTMLButtonElement).click()
    })
  await page.getByRole('option', { name: 'Primary' }).evaluate((element) => {
    ;(element as HTMLButtonElement).click()
  })

  await expect(page.getByText('Profile scope: Primary')).toBeVisible()

  await page.getByRole('link', { name: 'Explorer', exact: true }).click()
  await expect(page.getByLabel('Explorer profile')).toHaveValue(
    'chrome:Default',
  )
  await expect(
    page.getByText(
      'Using the shared profile scope until you choose a page-specific filter.',
    ),
  ).toBeVisible()

  await page
    .getByRole('button', { name: 'Toggle regular expression mode' })
    .click()
  await page.getByLabel('Explorer keyword').fill('sqlite(')

  await expect(page.getByRole('alert')).toHaveText('Invalid regular expression')
  await expect(
    page.getByText(
      'Fix the regular expression before PathKeep can run a scoped regex search.',
    ),
  ).toBeVisible()
  await expect(page.getByRole('button', { name: 'jsonl' })).toHaveCount(0)

  await page.getByLabel('Explorer keyword').fill('sqlite')
  await expect(page.getByText('Valid regular expression')).toBeVisible()
  await expect(page.getByRole('button', { name: 'jsonl' })).toBeEnabled()

  await page.getByRole('button', { name: 'jsonl' }).click()
  await expect(page.getByText(/pathkeep-export-/)).toBeVisible()
})

test('walks import preview, revert, restore, and doctor review in browser preview', async ({
  page,
}) => {
  await completePreviewOnboarding(page)

  await page.getByRole('link', { name: 'Import', exact: true }).click()
  await expect(page.getByTestId('import-page')).toBeVisible()

  await page.getByPlaceholder('/path/to/takeout.zip').fill('/tmp/takeout')
  await page.getByRole('button', { name: 'Scan source →' }).click()

  await expect(page.getByText('Step 3: Preview Import')).toBeVisible()
  await expect(page.getByText('PathKeep trust UX notes')).toBeVisible()

  await page.getByRole('button', { name: 'Confirm import →' }).click()
  await expect(page.getByText('Step 5: Import Complete')).toBeVisible()
  await expect(page.getByRole('button', { name: 'Revert batch' })).toBeEnabled()

  page.on('dialog', (dialog) => dialog.accept())
  await page.getByRole('button', { name: 'Revert batch' }).click()
  await expect(
    page.getByRole('button', { name: 'Restore batch' }),
  ).toBeEnabled()

  await page.getByRole('button', { name: 'Restore batch' }).click()
  await expect(page.getByRole('button', { name: 'Revert batch' })).toBeEnabled()

  await page.getByRole('button', { name: 'Run doctor' }).click()
  await expect(
    page.getByText('Import batch audit artifacts are present and reviewable.'),
  ).toBeVisible()
})

test('walks the remote backup PME from settings in browser preview', async ({
  page,
}) => {
  await completePreviewOnboarding(page)

  await page.getByRole('link', { name: 'Settings', exact: true }).click()
  const settingsPage = page.getByTestId('settings-page')

  await expect(settingsPage).toBeVisible()
  await expect(
    settingsPage.getByText('REMOTE BACKUP', { exact: true }),
  ).toBeVisible()
  await expect(
    settingsPage.getByText('ENRICHMENT + DERIVED STATE'),
  ).toBeVisible()

  await settingsPage.getByLabel('Bucket').fill('example-bucket')
  await settingsPage
    .getByRole('button', { name: 'Save remote settings' })
    .click()

  await settingsPage.getByLabel('Access key ID').fill('preview-key')
  await settingsPage.getByLabel('Secret access key').fill('preview-secret')
  await settingsPage.getByRole('button', { name: 'Store credentials' }).click()

  await expect(settingsPage.getByText('Credentials saved')).toBeVisible()

  await settingsPage.getByRole('button', { name: 'Preview bundle' }).click()
  await expect(
    settingsPage.getByText('Bundle path', { exact: true }),
  ).toBeVisible()
  await expect(
    settingsPage.getByText(/pathkeep-remote-.*\.zip/).first(),
  ).toBeVisible()

  await settingsPage.getByRole('button', { name: 'Manual' }).click()
  await expect(
    settingsPage.getByText('Preview command', { exact: true }),
  ).toBeVisible()

  await settingsPage.getByRole('button', { name: 'Execute upload' }).click()
  await expect(
    settingsPage.getByText(
      'Browser preview mode simulated the upload and produced a local bundle for verification.',
    ),
  ).toBeVisible()

  await settingsPage.getByRole('button', { name: 'Verify bundle' }).click()
  await expect(
    settingsPage.getByText('pathkeep.remote-backup.v1'),
  ).toBeVisible()
})
