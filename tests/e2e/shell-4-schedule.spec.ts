/**
 * The automatic-backup row on the Agent backup page (`data-spaces.md` §4):
 * one schedule for every kind, set in Settings › Schedule and background
 * work; the row says when Agent conversations back up next, on their own
 * clock (an interval after their last finished backup).
 *
 * Proves, where PathKeep installs the scheduler (macOS, Windows; the E2E
 * backend keeps it in a sandbox folder):
 * - with no schedule the row says automatic backup is off, and "Set a
 *   schedule" opens Settings › Schedule and background work;
 * - Hourly installed there: back on the Agent page the row says
 *   "Hourly · next …" at the time the backend gives for Agent conversations
 *   (`agentNextScheduledBackupAt`), and "Change schedule" leads back;
 * - turned off again, the row says so.
 * On Linux, where the schedule is set up by hand, the row says so and "See
 * steps" opens the card with the steps.
 *
 * Leaves the schedule off and the saved interval as it found them.
 */
import { expect, test, type Page } from '@playwright/test'
import { unlockIfLocked } from './support/app'
import { backend } from './support/fixture'

interface WireStatus {
  installState: string
  agentNextScheduledBackupAt?: string | null
  wakeIntervalSeconds?: number | null
}

const status = () => backend<WireStatus>('schedule_status', { platform: null })

const scheduleRow = (page: Page) =>
  page.locator('[data-schedule-row="agent-history"]')

const card = (page: Page) =>
  page
    .getByRole('heading', { name: 'Automatic backup' })
    .locator('xpath=ancestor::section[1]')

async function visit(page: Page, path: string) {
  await page.goto(path)
  await unlockIfLocked(page)
}

/**
 * How the row writes `iso` (moved on by whole wake intervals once it has
 * passed, as the app does): its day in this browser's zone, and its hour
 * and minute on either clock, since the clock follows the system's.
 */
async function written(page: Page, iso: string, wakeSeconds: number) {
  return page.evaluate(
    ([iso, step]) => {
      let next = Date.parse(iso)
      const now = Date.now()
      if (next <= now) next += (Math.floor((now - next) / step) + 1) * step
      const date = new Date(next)
      const day =
        date.toDateString() === new Date().toDateString()
          ? 'Today'
          : new Intl.DateTimeFormat('en-US', {
              month: 'short',
              day: 'numeric',
            }).format(date)
      const hours = date.getHours()
      const minutes = String(date.getMinutes()).padStart(2, '0')
      return { day, h12: ((hours + 11) % 12) + 1, h23: hours, minutes }
    },
    [iso, wakeSeconds * 1_000] as const,
  )
}

test('the Agent page’s automatic-backup row follows the one schedule set in Settings', async ({
  page,
}, testInfo) => {
  await visit(page, '/#/agent/backup')
  const row = scheduleRow(page)

  if (process.platform === 'linux') {
    await expect(row).toContainText('Set this up yourself')
    await row.getByRole('link', { name: 'See steps' }).click()
    await expect(page).toHaveURL(/#\/settings\/background$/)
    await expect(card(page)).toContainText('Set this up yourself')
    return
  }

  const { config } = await backend<{
    config: Record<string, unknown> & { dueAfterHours: number }
  }>('app_snapshot')
  expect((await status()).installState).toBe('not-installed')
  await expect(row).toContainText('Automatic backup is off')
  await row.getByRole('link', { name: 'Set a schedule' }).click()
  await expect(page).toHaveURL(/#\/settings\/background$/)
  await expect(
    page.getByText('One schedule backs up every kind of data that is on.'),
  ).toBeVisible()

  try {
    await card(page).getByRole('radio', { name: 'Hourly' }).click()
    const dialog = page.getByRole('dialog', {
      name: 'Turn on automatic backup',
    })
    await dialog.getByRole('button', { name: 'Install' }).click()
    await expect(dialog).toBeHidden({ timeout: 30_000 })
    await expect
      .poll(async () => (await status()).installState)
      .toBe('installed')

    // Its own clock: the time the backend gives for Agent conversations.
    const installed = await status()
    expect(installed.agentNextScheduledBackupAt).toBeTruthy()
    await visit(page, '/#/agent/backup')
    const when = await written(
      page,
      installed.agentNextScheduledBackupAt!,
      installed.wakeIntervalSeconds ?? 3_600,
    )
    await expect(row).toContainText(
      new RegExp(
        `Hourly · next ${when.day} (${when.h12}:${when.minutes}\\s?[AP]M|0?${when.h23}:${when.minutes})`,
      ),
    )
    await testInfo.attach('the row with a schedule', {
      body: await row.screenshot(),
      contentType: 'image/png',
    })
    await row.getByRole('link', { name: 'Change schedule' }).click()
    await expect(page).toHaveURL(/#\/settings\/background$/)

    // Off again.
    await card(page).getByRole('radio', { name: 'Off' }).click()
    const off = page.getByRole('dialog', { name: 'Turn off automatic backup?' })
    await off.getByRole('button', { name: 'Turn off' }).click()
    await expect(off).toBeHidden({ timeout: 30_000 })
    await expect
      .poll(async () => (await status()).installState)
      .toBe('not-installed')
    await visit(page, '/#/agent/backup')
    await expect(row).toContainText('Automatic backup is off')
  } finally {
    if ((await status()).installState === 'installed') {
      const plan = await backend('preview_schedule', {
        platform: null,
        dueAfterHours: 1,
      })
      await backend('remove_schedule', { plan })
    }
    const now = await backend<{ config: Record<string, unknown> }>(
      'app_snapshot',
    )
    await backend('save_config', {
      config: { ...now.config, dueAfterHours: config.dueAfterHours },
    })
  }
})
