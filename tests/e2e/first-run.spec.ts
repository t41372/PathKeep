/**
 * First run: a new user goes through onboarding with an encrypted archive,
 * three of the profiles PathKeep finds, the password in the keychain and no
 * schedule, then lands on Home.
 *
 * Proves: the first backup archives exactly the visits in the chosen
 * profiles (no profile counted twice, none dropped), the archive really is
 * encrypted with the key stored, and the browsers' own files are untouched.
 * Home, opened while the first Insights build waits, says it is working them
 * out instead of showing empty cards, and fills in once the build has run.
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
  // Background work paused, as Settings can, before the app loads its settings: onboarding keeps
  // them, so the first Insights build waits and Home can be seen before it is done, whatever the
  // machine's speed.
  await setBackgroundWork(false)

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
  const keychain = page.getByRole('switch', { name: /keychain/i })
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
      ai: { jobQueuePaused: boolean }
    }
    archiveStatus: { encrypted: boolean; unlocked: boolean }
  }>('app_snapshot')
  expect(snapshot.archiveStatus).toMatchObject({
    encrypted: true,
    unlocked: true,
  })
  expect(snapshot.config.rememberDatabaseKeyInKeyring).toBe(true)
  expect(snapshot.config.ai.jobQueuePaused, 'onboarding kept the pause').toBe(
    true,
  )
  expect([...snapshot.config.selectedProfileIds].sort()).toEqual([
    'chrome:Default',
    'chrome:Profile 1',
    'firefox:k3x9.default-release',
  ])
  expect(await backend<boolean>('keyring_get_database_key')).toBe(true)

  await page.getByRole('button', { name: 'Open PathKeep' }).click()

  // The first backup queued the first Insights build. Until it is done, Home's
  // Insights cards keep their skeletons and say so; none says it has nothing.
  const firstBuild = page.getByText(
    'Working out your Insights for the first time…',
  )
  await expect(firstBuild.first()).toBeVisible({ timeout: 30_000 })
  await expect(
    page.getByText(
      /Topics you return to over several days|Nothing from this date in earlier years/,
    ),
  ).toHaveCount(0)
  await testInfo.attach('Home before the first Insights build', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
  // Resumed, the build runs and the cards fill in by themselves.
  await setBackgroundWork(true)
  await expect(firstBuild).toHaveCount(0, { timeout: 120_000 })

  const saved = page.getByRole('link', { name: /Saved visits/ })
  // The value is a skeleton until the dashboard loads; wait for the number.
  await expect(saved).toHaveText(
    /Saved visits\s*[\d,]+\s*[\d,]+ new this week/,
    { timeout: 30_000 },
  )
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

/** Pauses or resumes background work the way the Settings switch does. */
async function setBackgroundWork(running: boolean) {
  const { config } = await backend<{
    config: { ai: { jobQueuePaused: boolean } }
  }>('app_snapshot')
  config.ai.jobQueuePaused = !running
  await backend('save_config', { config })
  // Settings reloads the queues after the switch, which starts their workers.
  if (running) await backend('load_intelligence_runtime')
}

async function labelOf(checkbox: Locator) {
  const id = await checkbox.getAttribute('id')
  const label = checkbox.page().locator(`label[for="${id}"]`)
  return (await label.innerText()).replace(/\s+/g, ' ')
}
