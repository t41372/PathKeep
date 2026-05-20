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

  // The v0.3 paper dashboard replaces the v0.2 "RECENT RUNS" panel +
  // disabled "Coming in v0.3" badges with a card-based composition. The
  // post-onboarding contract is now: the paper dashboard mounts and renders
  // its canonical testid-keyed cards (Archive / On This Day / This Week /
  // Active Threads).
  await expect(page.getByTestId('dashboard-page')).toBeVisible()
  await expect(page.getByTestId('dashboard-archive-card')).toBeVisible()
  await expect(page.getByTestId('dashboard-on-this-day')).toBeVisible()
  await expect(page.getByText('common.disabled')).toHaveCount(0)
}

test('walks through onboarding, first backup, explorer, and audit in browser preview', async ({
  page,
}) => {
  await completePreviewOnboarding(page)

  await page.getByRole('link', { name: 'Explorer', exact: true }).click()

  // The paper redesign retired the v0.2 ExplorerQueryFiltersPanel chrome:
  // the "SQLite inspection" trust callout, the AdvancedSearchHelp
  // hover-card with `site:github.com -pathkeep` / `manual OR youtube`
  // examples, and the inline `jsonl` export button are all gone. Phase 4
  // (`?layout=legacy` retirement) deleted those panels — paper Browse is
  // anchored on the contact-sheet + day navigator + paper Search palette
  // instead, with export moved into the Maintenance route's PME flow.
  // The remaining audit ledger journey still exists and is covered below.
  await expect(page.getByTestId('explorer-page')).toBeVisible()

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

test('surfaces intelligence and assistant routes after the first backup', async ({
  page,
}) => {
  await completePreviewOnboarding(page)

  // The paper redesign retired the v0.2 ExplorerQueryFiltersPanel's three-mode
  // toggle (Keyword / Hybrid / Semantic) along with the "Smart search is
  // coming in v0.3" / "Assistant is coming in v0.3" deferred copy. The paper
  // Search panel composes mode selection differently and the assistant route
  // now mounts a real composer. What is left to assert at the e2e level is
  // simply that the routes still mount their canonical paper testids.
  await page.goto('/#/assistant')
  await expect(page).toHaveURL(/#\/assistant/)

  await page.goto('/#/intelligence')
  await expect(page.getByTestId('intelligence-page')).toBeVisible({
    timeout: 10_000,
  })
  await expect(page.getByTestId('intelligence-runtime-digest')).toBeVisible()
})

// The "keeps shared profile scope, regex recall, and export guardrails
// aligned" test exercised the v0.2 inline ExplorerQueryFiltersPanel —
// the "Explorer profile" combobox in the page body, the "Toggle regex
// mode" button, the keyword `<input>` with debounced commit, the
// inline-alert "Invalid regex" copy, and the `jsonl` export button. Paper
// retired all five surfaces:
//
// - Profile scope lives in the top-bar source picker (covered by the
//   shell's status-bar tests).
// - Regex toggle lives inside the PaperSearchPanel mode strip with
//   different copy.
// - Keyword commit is immediate (no debounce), so the alert is gone too.
// - Export moved into the Maintenance route's PME flow.
//
// No equivalent paper surface exists to drive the same workflow end-to-end;
// the underlying behavior is covered by hook + intelligence-surface unit
// tests.

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
  const remoteSection = page.getByTestId('settings-remote')

  await expect(settingsPage).toBeVisible()
  await expect(remoteSection).toBeVisible()

  await remoteSection.getByLabel('Bucket').fill('example-bucket')
  await remoteSection.getByLabel('Region').fill('us-east-1')
  await remoteSection.getByRole('button', { name: 'Save', exact: true }).click()

  await remoteSection.getByLabel('Access key ID').fill('preview-key')
  await remoteSection.getByLabel('Secret access key').fill('preview-secret')
  await remoteSection.getByRole('button', { name: 'Save credentials' }).click()

  // The "Open Maintenance" link lives inside the remote-backup preferences
  // card; both Settings + Maintenance reuse the same RemoteBackupSection
  // component so its testid is stable across routes.
  await page
    .getByRole('link', { name: /Open Maintenance/i })
    .first()
    .click()
  const maintenancePage = page.getByTestId('maintenance-page')
  const maintenanceRemoteSection =
    maintenancePage.getByTestId('settings-remote')

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
