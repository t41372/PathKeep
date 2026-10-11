/**
 * Home's alerts (archive.md: a failing automatic backup and an archive
 * password at risk must not stay hidden in Backup or Security).
 *
 * Proves:
 * - With the archive as first-run left it (encrypted, its password in the
 *   keychain, no automatic-backup trouble) Home shows no alerts.
 * - A failed automatic backup, recorded the way the scheduled worker records
 *   it, shows on Home with what it means and leads to Backup; once a later
 *   run succeeds, the alert is gone.
 * - Taking the password out of the keychain in Settings → Security shows on
 *   Home that only the user has it. "Save it in the keychain" leads back to
 *   Security, and saving it there clears the alert. Taken out again, "I keep
 *   it safe" clears it for good, also after a reload.
 *
 * Leaves the password in the keychain: the restart project reopens the
 * archive from it.
 */
import { mkdirSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { expect, test, type Page } from '@playwright/test'
import { fixture } from './support/fixture'

/** first-run's; security.spec changes it later. */
const ARCHIVE_PASSWORD = 'correct horse battery'

const alerts = (page: Page) =>
  page.getByRole('region', { name: 'Needs your attention' })

/** Home, read afresh: the scheduler's status is otherwise kept for a minute. */
async function openHome(page: Page) {
  await page.goto('/#/')
  await page.reload()
  // The page has loaded once its stat cards are there.
  await expect(page.getByText('Saved visits', { exact: true })).toBeVisible()
}

/** One run of the scheduled worker, as it records it in the attempt ledger. */
function recordScheduledRun(
  id: string,
  at: Date,
  outcome: 'failed' | 'success',
) {
  const directory = path.join(
    fixture().dirs.projectRoot,
    'schedule',
    'attempts',
  )
  mkdirSync(directory, { recursive: true })
  writeFileSync(
    path.join(directory, `attempt-${id}.json`),
    JSON.stringify({
      id,
      startedAt: at.toISOString(),
      finishedAt: new Date(at.getTime() + 2_000).toISOString(),
      outcome,
      phase: outcome === 'failed' ? 'backup' : 'finished',
      reasonCode: outcome === 'failed' ? 'backup-failed' : 'backup-succeeded',
      detail:
        outcome === 'failed' ? 'Chrome history could not be copied' : null,
      runId: null,
    }),
  )
}

async function keychainSwitch(page: Page) {
  await page.goto('/#/settings/security')
  return page.getByRole('switch', { name: 'Save password in the keychain' })
}

async function takeOutOfKeychain(page: Page) {
  const toggle = await keychainSwitch(page)
  await expect(toggle).toBeChecked()
  await toggle.click()
  // The key is gone and the setting saved; the switch turns off last.
  await expect(toggle).not.toBeChecked()
}

async function saveInKeychain(page: Page) {
  const toggle = page.getByRole('switch', {
    name: 'Save password in the keychain',
  })
  await toggle.click()
  const dialog = page.getByRole('dialog', {
    name: 'Save password in the keychain',
  })
  await dialog.getByLabel('Archive password').fill(ARCHIVE_PASSWORD)
  await dialog.getByRole('button', { name: 'Save' }).click()
  await expect(dialog).toBeHidden()
  await expect(toggle).toBeChecked()
}

test('Home shows what needs fixing and where, and nothing once it is fixed', async ({
  page,
}) => {
  await openHome(page)
  await expect(alerts(page)).toHaveCount(0)

  // A scheduled backup that failed an hour ago.
  recordScheduledRun('e2e-1', new Date(Date.now() - 3_600_000), 'failed')
  await openHome(page)
  const failed = alerts(page).getByRole('listitem').filter({
    hasText: 'The last automatic backup failed',
  })
  await expect(failed).toContainText(
    'New history is not saved until this is fixed.',
  )
  await failed.getByRole('link', { name: 'Open Backup' }).click()
  await expect(page).toHaveURL(/#\/backup$/)

  // The next run went through.
  recordScheduledRun('e2e-2', new Date(Date.now() - 60_000), 'success')
  await openHome(page)
  await expect(alerts(page)).toHaveCount(0)

  try {
    // The password leaves the keychain: only the user has it now.
    await takeOutOfKeychain(page)
    await openHome(page)
    const risk = alerts(page).getByRole('listitem').filter({
      hasText: 'Only you have the archive password',
    })
    await expect(risk).toBeVisible()
    await risk.getByRole('link', { name: 'Save it in the keychain' }).click()
    await expect(page).toHaveURL(/#\/settings\/security$/)
    await saveInKeychain(page)
    await openHome(page)
    await expect(alerts(page)).toHaveCount(0)

    // Out again, and the user says they keep it safe.
    await takeOutOfKeychain(page)
    await openHome(page)
    await alerts(page)
      .getByRole('listitem')
      .filter({ hasText: 'Only you have the archive password' })
      .getByRole('button', { name: 'I keep it safe' })
      .click()
    await expect(alerts(page)).toHaveCount(0)
    await openHome(page)
    await expect(alerts(page)).toHaveCount(0)
  } finally {
    // The rest of the suite reopens the archive from the keychain.
    const toggle = await keychainSwitch(page)
    await expect(toggle).toBeVisible()
    if (!(await toggle.isChecked())) await saveInKeychain(page)
  }
})
