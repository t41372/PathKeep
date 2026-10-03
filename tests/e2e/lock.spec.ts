/**
 * App lock: without a passcode the lock button explains itself; with one,
 * PathKeep locks, refuses a wrong passcode and opens with the right one.
 *
 * Proves: the lock screen never asks for a passcode that does not exist,
 * and the passcode set in Settings is the one the lock screen checks.
 */
import { expect, test } from '@playwright/test'

const PASSCODE = '4826'

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
  await dialog.getByRole('button', { name: 'Save passcode' }).click()
  await expect(page.getByText('Passcode saved. App lock is on.')).toBeVisible()

  await page.keyboard.press('ControlOrMeta+l')
  await expect(
    page.getByRole('heading', { name: 'PathKeep is locked' }),
  ).toBeVisible()
  const field = page.getByPlaceholder('Passcode')
  await field.fill('0000')
  await field.press('Enter')
  await expect(
    page.getByText('That did not work. Check it and try again.'),
  ).toBeVisible()
  await expect(
    page.getByRole('heading', { name: 'PathKeep is locked' }),
  ).toBeVisible()

  await field.fill(PASSCODE)
  await field.press('Enter')
  await expect(
    page.getByRole('heading', { name: 'PathKeep is locked' }),
  ).toBeHidden()
  await expect(page.getByRole('navigation')).toBeVisible()
})
