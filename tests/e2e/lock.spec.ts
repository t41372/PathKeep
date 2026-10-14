/**
 * App lock: without a passcode the lock button explains itself; with one,
 * PathKeep locks, refuses a wrong passcode and opens with the right one.
 * "Forgot passcode?" shows the hint saved with the passcode, and changing
 * the passcode keeps that hint.
 *
 * Proves: the lock screen never asks for a passcode that does not exist,
 * the passcode set in Settings is the one the lock screen checks, the hint
 * is readable on a locked window (from the backend, not from a cache the
 * lock cleared), and changing the passcode no longer wipes the hint.
 *
 * Touch ID cannot run in headless Chrome. Its rules (shown only when turned
 * on, never prompting while unavailable, refused when Settings turned it
 * off) are covered by the `app_lock` Rust tests in vault-core.
 *
 * Leaves app lock on with passcode 4826; wipe.spec relies on that.
 */
import { expect, test, type Page } from '@playwright/test'

const PASSCODE = '4826'
const HINT = 'the year we moved, backwards'

const lockHeading = (page: Page) =>
  page.getByRole('heading', { name: 'PathKeep is locked' })

async function lock(page: Page) {
  await page.keyboard.press('ControlOrMeta+l')
  await expect(lockHeading(page)).toBeVisible()
}

async function unlock(page: Page) {
  const field = page.getByPlaceholder('Passcode')
  await field.fill(PASSCODE)
  await field.press('Enter')
  await expect(lockHeading(page)).toBeHidden()
}

/** Opens "Forgot passcode?" and expects the saved hint, hidden until then. */
async function expectHintOnLockScreen(page: Page) {
  await expect(page.getByText(HINT)).toBeHidden()
  await page.getByRole('button', { name: 'Forgot passcode?' }).click()
  await expect(page.getByText(HINT)).toBeVisible()
  await expect(
    page.getByText('App lock only guards this window.', { exact: false }),
  ).toBeVisible()
  await expect(page.getByText(/config\.json$/)).toBeVisible()
}

test('the passcode from Settings locks and unlocks PathKeep', async ({
  page,
}) => {
  await page.goto('/#/')
  await expect(page.getByRole('link', { name: /Saved visits/ })).toBeVisible()

  // No passcode yet: locking sends you to set one instead of locking you out.
  await page.keyboard.press('ControlOrMeta+l')
  await expect(
    page.getByText('Set a passcode first, then PathKeep can lock.'),
  ).toBeVisible()
  await expect(page).toHaveURL(/#\/settings\/security$/)

  await page.getByRole('switch', { name: /App lock/ }).click()
  const dialog = page.getByRole('dialog', { name: 'Set a passcode' })
  await dialog.getByLabel('Passcode', { exact: true }).fill(PASSCODE)
  await dialog.getByLabel('Type it again').fill(PASSCODE)
  await dialog.getByLabel('Hint (optional)').fill(HINT)
  await dialog.getByRole('button', { name: 'Save passcode' }).click()
  await expect(page.getByText('Passcode saved. App lock is on.')).toBeVisible()

  await lock(page)
  const field = page.getByPlaceholder('Passcode')
  await field.fill('0000')
  await field.press('Enter')
  await expect(
    page.getByText('That did not work. Check it and try again.'),
  ).toBeVisible()
  await expect(lockHeading(page)).toBeVisible()

  await unlock(page)
  await expect(page.getByRole('heading', { name: 'Security' })).toBeVisible()
})

test('"Forgot passcode?" shows the hint, and changing the passcode keeps it', async ({
  page,
}) => {
  await page.goto('/#/settings/security')
  // Wait for the app to finish booting; ⌘L does nothing before that.
  const security = page.getByRole('heading', { name: 'Security' })
  await expect(security.or(lockHeading(page))).toBeVisible()
  if (await lockHeading(page).isVisible()) await unlock(page)
  await expect(security).toBeVisible()

  await lock(page)
  await expectHintOnLockScreen(page)
  await unlock(page)

  // The bug this guards: the change dialog sent no hint and the backend
  // stored "no hint" with the new passcode.
  await expect(page.getByText(`Hint: ${HINT}`)).toBeVisible()
  const passcodeRow = page
    .locator('div.rounded-xl')
    .filter({ has: page.getByText('Passcode', { exact: true }) })
  await passcodeRow.getByRole('button', { name: 'Change…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Change the passcode' })
  await expect(dialog.getByLabel('Hint (optional)')).toHaveValue(HINT)
  await dialog.getByLabel('Passcode', { exact: true }).fill(PASSCODE)
  await dialog.getByLabel('Type it again').fill(PASSCODE)
  await dialog.getByRole('button', { name: 'Save passcode' }).click()
  await expect(page.getByText('Passcode changed.')).toBeVisible()
  await expect(page.getByText(`Hint: ${HINT}`)).toBeVisible()

  await lock(page)
  await expectHintOnLockScreen(page)
  await unlock(page)
})
