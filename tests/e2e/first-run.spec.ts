/**
 * First run: a new user goes through onboarding with an encrypted archive,
 * three of the profiles PathKeep finds, the password in the keychain and no
 * schedule, then lands on Home.
 *
 * Proves: the first backup archives exactly the visits in the chosen
 * profiles (no profile counted twice, none dropped), the archive really is
 * encrypted with the key stored, and the browsers' own files are untouched.
 */
import { expect, test, type Locator } from '@playwright/test'
import {
  archivedVisits,
  backend,
  browserFileHashes,
  expectCount,
  parseCount,
} from './support/fixture'

const PASSWORD = 'correct horse battery'

test('onboarding sets up an encrypted archive and backs up the chosen browsers', async ({
  page,
}, testInfo) => {
  const before = browserFileHashes()
  const expected = archivedVisits().length

  await page.goto('/')
  await expect(
    page.getByRole('heading', {
      name: 'Your browser is deleting your history',
    }),
  ).toBeVisible()
  await page.getByRole('button', { name: 'Get started' }).click()

  // Browsers: keep the two Chrome profiles and Firefox. The Firefox forks
  // point at the same synthetic profile, so they must stay unchecked.
  await expect(
    page.getByRole('heading', { name: 'Which browsers should we back up?' }),
  ).toBeVisible()
  const want = [
    /Google Chrome\s*Personal/,
    /Google Chrome\s*Work/,
    /^Firefox\s*default-release/,
  ]
  for (const checkbox of await page.getByRole('checkbox').all()) {
    const name =
      (await checkbox.getAttribute('aria-label')) ?? (await labelOf(checkbox))
    const shouldCheck = want.some((pattern) => pattern.test(name))
    if ((await checkbox.isChecked()) !== shouldCheck) await checkbox.click()
  }
  await page.getByRole('button', { name: 'Continue' }).click()

  await expect(
    page.getByRole('heading', { name: 'Where the archive lives' }),
  ).toBeVisible()
  await page.getByRole('button', { name: 'Continue' }).click()

  await expect(
    page.getByRole('heading', { name: 'Encrypt the archive?' }),
  ).toBeVisible()
  await page.getByRole('radio', { name: /^Encrypt/ }).click()
  await page.getByLabel('Password', { exact: true }).fill(PASSWORD)
  await page.getByLabel('Confirm password').fill(PASSWORD)
  const keychain = page.getByRole('checkbox', { name: /keychain/i })
  if (!(await keychain.isChecked())) await keychain.click()
  await page.getByRole('button', { name: 'Continue' }).click()

  // No schedule: installing one would talk to the real launchd.
  await expect(
    page.getByRole('heading', { name: 'How often should we back up?' }),
  ).toBeVisible()
  await page.getByRole('radio', { name: /^Manual/ }).click()
  await page.getByRole('button', { name: 'Continue' }).click()

  await expect(
    page.getByRole('heading', { name: 'Turn on AI features?' }),
  ).toBeVisible()
  await page.getByRole('radio', { name: /^Not now/ }).click()
  await page.getByRole('button', { name: 'Continue' }).click()

  await expect(
    page.getByRole('heading', { name: 'You’re all set' }),
  ).toBeVisible()
  await page.getByRole('button', { name: 'Start first backup' }).click()
  const done = page.getByText(/^Done\. [\d,]+ visits? saved\.$/)
  await expect(done).toBeVisible({ timeout: 90_000 })
  await expectCount(
    testInfo,
    'visits saved by the first backup',
    parseCount(await done.innerText()),
    expected,
  )

  const snapshot = await backend<{
    config: {
      rememberDatabaseKeyInKeyring: boolean
      selectedProfileIds: string[]
    }
    archiveStatus: { encryptionMode: string; unlocked: boolean }
  }>('app_snapshot')
  expect(snapshot.archiveStatus.encryptionMode.toLowerCase()).toContain(
    'encrypt',
  )
  expect(snapshot.config.rememberDatabaseKeyInKeyring).toBe(true)
  expect([...snapshot.config.selectedProfileIds].sort()).toEqual([
    'chrome:Default',
    'chrome:Profile 1',
    'firefox:k3x9.default-release',
  ])
  expect(await backend<string | null>('keyring_get_database_key')).toBe(
    PASSWORD,
  )

  await page.getByRole('button', { name: 'Open PathKeep' }).click()
  const saved = page.getByRole('link', { name: /Saved visits/ })
  await expect(saved).toContainText(/\d/)
  const savedText =
    (await saved.innerText()).match(/Saved visits\s+([\d,]+)/)?.[1] ?? ''
  await expectCount(
    testInfo,
    'saved visits on Home',
    parseCount(savedText),
    expected,
  )

  expect(browserFileHashes(), 'browser files must not change').toEqual(before)
})

async function labelOf(checkbox: Locator) {
  const id = await checkbox.getAttribute('id')
  const label = checkbox.page().locator(`label[for="${id}"]`)
  return (await label.innerText()).replace(/\s+/g, ' ')
}
