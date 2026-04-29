import { expect, test, type Page } from '@playwright/test'

test.describe.configure({ mode: 'parallel' })

async function completePreviewOnboarding(page: Page) {
  await page.goto('/')

  await expect(page.getByRole('heading', { name: 'Dashboard' })).toBeVisible()
  await expect(
    page.getByRole('heading', {
      name: 'Ready to back up your browsing history',
    }),
  ).toBeVisible()

  await page.getByRole('link', { name: 'Start setup' }).click()

  await expect(page.getByTestId('onboarding-page')).toBeVisible()
  await expect(page.getByText(/Open-source .* GPL v3/i)).toBeVisible()
  await page.getByRole('button', { name: /Get Started/ }).click()
  await page.getByRole('button', { name: /Continue/ }).click()
  await page.getByRole('button', { name: /Continue/ }).click()
  await page.getByPlaceholder(/Enter a password/i).fill('vault-passphrase')
  await page
    .getByPlaceholder(/Enter the same password again/i)
    .fill('vault-passphrase')
  await page.getByRole('button', { name: /Continue/ }).click()
  await page.getByRole('button', { name: 'Skip for now' }).click()
  await page.getByRole('button', { name: 'Create Archive & Back Up →' }).click()

  await expect(page.getByText('RECENT RUNS')).toBeVisible()
  await expect(page.getByText('ON THIS DAY')).toBeVisible()
  await expect(
    page.getByRole('link', { name: 'Open Intelligence' }),
  ).toHaveCount(2)
  await expect(page.getByText('AI features off')).toBeVisible()
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

  await page.getByRole('button', { name: 'jsonl' }).evaluate((element) => {
    ;(element as HTMLButtonElement).click()
  })
  await expect(page.getByText(/pathkeep-export-/)).toBeVisible()

  await Promise.all([
    page.waitForURL(/#\/audit/),
    page.getByRole('link', { name: 'Audit Ledger' }).click(),
  ])

  const auditPage = page.getByTestId('audit-page')
  await expect(auditPage).toBeVisible({ timeout: 10_000 })
  await expect(auditPage.getByText('RUN TIMELINE')).toBeVisible()
  await auditPage.getByRole('button', { name: 'Artifacts' }).click()
  await expect(auditPage.getByText(/ARTIFACTS · \d+ files/)).toBeVisible()
})

test('keeps schedule and security review surfaces inspectable in browser preview', async ({
  page,
}) => {
  await completePreviewOnboarding(page)

  await page
    .getByRole('link', { name: 'Scheduled Backup Settings', exact: true })
    .click()
  const schedulePage = page.getByTestId('schedule-page')
  await expect(schedulePage).toBeVisible()
  await expect(schedulePage.getByText('CURRENT STATE')).toBeVisible()
  await expect(schedulePage.getByText('Backup trigger')).toBeVisible()
  await expect(schedulePage.getByText('Profiles')).toBeVisible()
  await expect(schedulePage.getByText('RECOVERY ACTIONS')).toBeVisible()
  await schedulePage.getByText('Manual Install', { exact: true }).click()
  await expect(
    schedulePage.getByRole('button', { name: 'I Completed These Steps' }),
  ).toBeVisible()
  await expect(schedulePage.getByText('Follow the platform step')).toBeVisible()
  await expect(
    schedulePage.getByRole('button', { name: 'Run This Step Automatically' }),
  ).toBeVisible()
  await expect(
    schedulePage.getByRole('button', { name: 'Verify This Step' }),
  ).toBeVisible()
  await expect(
    page.getByText(
      'launchctl bootstrap gui/501 com.yi-ting.pathkeep.backup.plist',
    ),
  ).toBeVisible()

  await page.getByRole('link', { name: 'Security', exact: true }).click()
  await expect(page.getByTestId('security-page')).toBeVisible()
  await expect(page.getByText('ENCRYPTION', { exact: true })).toBeVisible()
  await expect(page.getByText('Archive is Encrypted')).toBeVisible()
})

test('surfaces intelligence routes and degraded states after the first backup', async ({
  page,
}) => {
  await completePreviewOnboarding(page)

  await page.getByRole('link', { name: 'Explorer', exact: true }).click()
  await page.getByRole('button', { name: 'hybrid' }).click()
  await expect(page.getByText('SEMANTIC RECALL')).toBeVisible()
  await expect(page.getByText('AI features off')).toBeVisible()

  await Promise.all([
    page.waitForURL(/#\/assistant/),
    page.getByRole('link', { name: 'AI Assistant', exact: true }).click(),
  ])
  await expect(page.getByText('Assistant is turned off')).toBeVisible()

  await Promise.all([
    page.waitForURL(/#\/intelligence/),
    page.getByRole('link', { name: 'Intelligence', exact: true }).click(),
  ])
  await expect(page.getByTestId('intelligence-page')).toBeVisible({
    timeout: 10_000,
  })
  await expect(page.getByTestId('intelligence-runtime-digest')).toBeVisible()
  await expect(
    page.getByText('Manual output review moved to Settings'),
  ).toHaveCount(0)
  await expect(
    page.getByRole('link', { name: 'Review in Settings' }),
  ).toHaveCount(0)
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
      'Showing results for the selected profile. Change it in the top bar.',
    ),
  ).toBeVisible()

  await page.getByRole('button', { name: 'Toggle regex mode' }).click()
  await page.getByLabel('Explorer keyword').fill('sqlite(')

  await expect(page.getByRole('alert')).toHaveText('Invalid regex')
  await expect(
    page.getByText('Fix the regex pattern before searching.'),
  ).toBeVisible()
  await expect(page.getByRole('button', { name: 'jsonl' })).toHaveCount(0)

  await page.getByLabel('Explorer keyword').fill('sqlite')
  await expect(page.getByText('Valid regex')).toBeVisible()
  await expect(page.getByRole('button', { name: 'jsonl' })).toBeEnabled()

  await page.getByRole('button', { name: 'jsonl' }).evaluate((element) => {
    ;(element as HTMLButtonElement).click()
  })
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
  await expect(page.getByRole('button', { name: 'Undo import' })).toBeEnabled()

  page.on('dialog', (dialog) => dialog.accept())
  await page
    .getByRole('button', { name: 'Undo import' })
    .evaluate((element) => {
      ;(element as HTMLButtonElement).click()
    })
  await expect(
    page.getByRole('button', { name: 'Restore import' }),
  ).toBeEnabled()

  await page
    .getByRole('button', { name: 'Restore import' })
    .evaluate((element) => {
      ;(element as HTMLButtonElement).click()
    })
  await expect(page.getByRole('button', { name: 'Undo import' })).toBeEnabled()

  await page.getByRole('button', { name: 'Show history' }).click()
  await page.getByRole('button', { name: 'Run health check' }).click()
  await expect(
    page.getByText('Import batch audit artifacts are present and reviewable.'),
  ).toBeVisible()
})

test('walks remote backup settings and Maintenance PME in browser preview', async ({
  page,
}) => {
  await completePreviewOnboarding(page)

  await page.getByRole('link', { name: 'Settings', exact: true }).click()
  const settingsPage = page.getByTestId('settings-page')
  const remoteSection = settingsPage
    .locator('.panel')
    .filter({ hasText: 'CLOUD BACKUP' })

  await expect(settingsPage).toBeVisible()
  await expect(
    remoteSection.getByText('CLOUD BACKUP', { exact: true }),
  ).toBeVisible()

  await remoteSection.getByLabel('Bucket').fill('example-bucket')
  await remoteSection.getByLabel('Region').fill('us-east-1')
  await remoteSection.getByRole('button', { name: 'Save', exact: true }).click()

  await remoteSection.getByLabel('Access key ID').fill('preview-key')
  await remoteSection.getByLabel('Secret access key').fill('preview-secret')
  await remoteSection.getByRole('button', { name: 'Save credentials' }).click()

  await remoteSection.getByRole('link', { name: 'Open Maintenance' }).click()
  const maintenancePage = page.getByTestId('maintenance-page')
  const maintenanceRemoteSection = maintenancePage
    .locator('.panel')
    .filter({ hasText: 'CLOUD BACKUP' })

  await expect(maintenancePage).toBeVisible()
  await maintenanceRemoteSection
    .getByRole('button', { name: 'Preview upload' })
    .click()
  await expect(
    maintenanceRemoteSection.getByText('File path', { exact: true }),
  ).toBeVisible()
  await expect(
    maintenanceRemoteSection.getByText(/pathkeep-remote-.*\.zip/).first(),
  ).toBeVisible()

  await maintenanceRemoteSection.getByRole('button', { name: 'Manual' }).click()
  await expect(
    maintenanceRemoteSection.getByText('Upload command', { exact: true }),
  ).toBeVisible()

  await maintenanceRemoteSection
    .getByRole('button', { name: 'Upload now' })
    .click()
  await expect(
    maintenanceRemoteSection.getByText(
      'Browser preview mode simulated the upload and produced a local bundle for verification.',
    ),
  ).toBeVisible()

  await maintenanceRemoteSection
    .getByRole('button', { name: 'Verify backup' })
    .click()
  await expect(
    maintenanceRemoteSection.getByText('pathkeep.remote-backup.v1'),
  ).toBeVisible()
})
