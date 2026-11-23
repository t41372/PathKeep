/**
 * "Delete all data" that didn't finish and was put back: the person is told
 * once, in the place they will look.
 *
 * Proves:
 * - a delete that fails after moving the archive out is put back in the same
 *   attempt: its settings file and no unfinished-delete record; the archive
 *   opens again with every visit; the one-time notice was taken by the
 *   dialog, which says nothing was deleted, so the app doesn't say it a
 *   second time later;
 * - a delete cut short partway through putting things back leaves its record
 *   and the emptied settings file; the next start puts everything back
 *   before the archive opens, and once it is open the app says so in a
 *   notice that stays until it is closed; the archive has every visit, and
 *   the notice is not said again.
 *
 * Failures come from the debug build's fault points (`injectFault`), and the
 * cut is a restart of the app (`killAndRestartApp`).
 *
 * Runs after import-keeps-conversations.spec: encrypted with move.spec's
 * file password, App Lock passcode 4826. Leaves the archive as it found it.
 */
import { existsSync } from 'node:fs'
import path from 'node:path'
import { expect, test, type Page } from '@playwright/test'
import { killAndRestartApp } from './support/app'
import {
  accessScreen,
  dataRoot,
  enterApp,
  loadFresh,
  MOVE_FILE_PASSWORD,
  readControl,
} from './support/archive-recovery'
import { backend, injectFault, openArchiveVisitTotal } from './support/fixture'

test.describe.configure({ mode: 'serial' })

const WIPE_RECORD = '.pk-wipe-v2.json'
const NOTICE = 'The last “Delete all data” didn’t finish'

const noticeToast = (page: Page) =>
  page.locator('[data-sonner-toast]').filter({ hasText: NOTICE })

/** Storage, loaded from scratch: the archive may have closed since. */
async function openStorage(page: Page) {
  await loadFresh(page, '/#/settings/storage')
  await enterApp(
    page,
    MOVE_FILE_PASSWORD,
    page.getByRole('heading', { name: 'Storage' }),
  )
}

/**
 * Where the window can be once a failed delete returns: its dialog saying
 * nothing was deleted, or, when the archive closed with it, a lock screen,
 * the recovery screen or setup.
 */
const afterFailedDelete = (page: Page) =>
  page
    .getByRole('dialog', { name: 'Delete all data?' })
    .getByText(/^Nothing was deleted\./)
    .or(page.getByRole('heading', { name: 'PathKeep is locked' }))
    .or(page.getByRole('heading', { name: 'Your archive is encrypted' }))
    .or(
      page.getByRole('heading', {
        name: 'Keep what gets deleted automatically',
      }),
    )
    .or(accessScreen(page))

async function deleteEverything(page: Page) {
  await page.getByRole('button', { name: 'Delete…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Delete all data?' })
  await dialog.getByRole('textbox').fill('DELETE')
  await dialog.getByRole('button', { name: 'Delete forever' }).click()
  return dialog
}

test('a delete put back in the same attempt says so in its dialog, once', async ({
  page,
}) => {
  test.setTimeout(300_000)
  await openStorage(page)
  const visits = await openArchiveVisitTotal()
  expect(visits).toBeGreaterThan(0)

  const heal = injectFault('wipe-moved')
  const dialog = page.getByRole('dialog', { name: 'Delete all data?' })
  try {
    await deleteEverything(page)
    await expect(afterFailedDelete(page).first()).toBeVisible({
      timeout: 120_000,
    })
  } finally {
    heal()
  }

  // Put back in the same attempt: the settings file, no record.
  expect(existsSync(path.join(dataRoot(), WIPE_RECORD))).toBe(false)
  expect(readControl().initialized).toBe(true)

  // Open again, with every visit. The rollback left the archive closed, so
  // the window loads again and unlocks it if asked.
  if (await dialog.isVisible())
    await dialog.getByRole('button', { name: 'Cancel' }).click()
  await openStorage(page)
  expect(await openArchiveVisitTotal()).toBe(visits)

  // The dialog took the notice: a fresh window doesn't say it again.
  await page.reload()
  await enterApp(
    page,
    MOVE_FILE_PASSWORD,
    page.getByRole('heading', { name: 'Storage' }),
  )
  expect(await backend<unknown[]>('take_wipe_notice', {})).toEqual([])
  await expect(noticeToast(page)).toHaveCount(0)
})

test('a delete cut short is put back at the next start, and the app says so once', async ({
  page,
}, testInfo) => {
  test.setTimeout(600_000)
  await openStorage(page)
  const visits = await openArchiveVisitTotal()

  // Fails after moving the archive out, then again while putting it back.
  const healMoved = injectFault('wipe-moved')
  const healPutBack = injectFault('wipe-entry-put-back')
  try {
    await deleteEverything(page)
    await expect(afterFailedDelete(page).first()).toBeVisible({
      timeout: 120_000,
    })
  } finally {
    healMoved()
    healPutBack()
  }
  // Half put back: the record stays, with the emptied settings file.
  expect(existsSync(path.join(dataRoot(), WIPE_RECORD))).toBe(true)
  expect(readControl().initialized).toBeFalsy()

  // The Playwright page must be the first window to take the one-shot notice.
  await killAndRestartApp({ PATHKEEP_TEST_NO_MAIN_WINDOW: '1' })
  await openStorage(page)
  // Put back before the archive opened; said once it is open.
  const toast = noticeToast(page)
  await expect(toast).toBeVisible({ timeout: 60_000 })
  await expect(toast).toContainText(
    'PathKeep put everything back, so your archive is as it was. To delete everything, run Delete all data again.',
  )
  await testInfo.attach('the notice after the start', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
  expect(existsSync(path.join(dataRoot(), WIPE_RECORD))).toBe(false)
  expect(readControl().initialized).toBe(true)
  expect(await openArchiveVisitTotal()).toBe(visits)

  // It stays until it is closed, and isn't said again.
  await toast.getByRole('button', { name: 'Close' }).click()
  await expect(toast).toHaveCount(0)
  await page.reload()
  await enterApp(
    page,
    MOVE_FILE_PASSWORD,
    page.getByRole('heading', { name: 'Storage' }),
  )
  expect(await backend<unknown[]>('take_wipe_notice', {})).toEqual([])
  await expect(noticeToast(page)).toHaveCount(0)
})
