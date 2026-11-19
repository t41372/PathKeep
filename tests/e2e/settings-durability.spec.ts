/**
 * A settings save the disk did not confirm. The backend installs the new
 * settings, then cannot flush the folder that holds them (the debug fault
 * `config-save-parent-sync`), so it answers with the settings in effect and a
 * diagnostic.
 *
 * Proves:
 * - The person sees a warning, not an error: the settings are saved and in
 *   effect, a power loss soon could bring the previous ones back, and the
 *   backend's diagnostic.
 * - The settings are in effect: the control shows the new value, the backend
 *   answers it, the settings file on disk holds it, and a reloaded window
 *   shows it.
 * - The next save, which the disk confirms, says nothing.
 *
 * Runs in `change` after lock.spec, so App Lock is on. Puts "Tasks at a time"
 * back as it was.
 */
import { readFileSync } from 'node:fs'
import { expect, test, type Page } from '@playwright/test'
import type { AppConfig, AppSnapshot } from '../../src/lib/types'
import { unlockIfLocked } from './support/app'
import { backend, injectFault } from './support/fixture'

const TITLE = 'Settings saved, but the disk didn’t confirm it'

const snapshot = () => backend<AppSnapshot>('app_snapshot')

const tasksAtATime = (page: Page) =>
  page.getByRole('combobox', { name: 'Tasks at a time' })
const durabilityToast = (page: Page) =>
  page.locator('[data-sonner-toast]').filter({ hasText: TITLE })

/** "Tasks at a time" as the settings file on disk has it. */
const onDisk = (configPath: string) =>
  (JSON.parse(readFileSync(configPath, 'utf8')) as AppConfig).ai
    .jobQueueConcurrency

async function openBackgroundSettings(page: Page) {
  await page.goto('/#/settings/background')
  await unlockIfLocked(page)
  await expect(tasksAtATime(page)).toBeVisible({ timeout: 60_000 })
}

/** Chooses `count` in "Tasks at a time", as a person does, and returns the save's answer. */
async function chooseTasks(page: Page, count: number) {
  const saved = page.waitForResponse((response) =>
    response.url().endsWith('/commands/save_config'),
  )
  await tasksAtATime(page).click()
  await page.getByRole('option', { name: String(count), exact: true }).click()
  const response = await saved
  expect(response.ok(), 'the save went through').toBe(true)
  return (await response.json()) as AppSnapshot
}

test('a save the disk did not confirm is in effect and says so; the next save says nothing', async ({
  page,
}, info) => {
  await openBackgroundSettings(page)
  const { config, directories } = await snapshot()
  const original = config.ai.jobQueueConcurrency
  const shown = Math.max(1, original)
  const [unconfirmed, confirmed] = [1, 2, 3, 4].filter(
    (count) => count !== shown,
  )
  await expect(tasksAtATime(page)).toHaveText(String(shown))

  let clear: (() => void) | null = injectFault('config-save-parent-sync')
  try {
    const answer = await chooseTasks(page, unconfirmed)
    clear()
    clear = null
    expect(answer.configDurabilityWarning).toContain('config-save-parent-sync')
    expect(answer.config.ai.jobQueueConcurrency).toBe(unconfirmed)

    // What the person sees: a warning with the diagnostic, and no error.
    const toast = durabilityToast(page)
    await expect(toast).toBeVisible()
    // Held while it is read: a toast pauses under the pointer.
    await toast.hover()
    await expect(toast).toHaveAttribute('data-type', 'warning')
    await expect(toast).toContainText(
      'They are in effect now. If this computer loses power soon, the previous settings may come back.',
    )
    await expect(toast).toContainText(directories.configPath)
    await expect(toast).toContainText('config-save-parent-sync')
    await expect(page.getByText('Could not save this setting')).toHaveCount(0)
    await info.attach('settings not confirmed by the disk', {
      body: await toast.screenshot(),
      contentType: 'image/png',
    })
    // It stays until closed: still up well past a default toast's life.
    await page.mouse.move(0, 0)
    await page.waitForTimeout(6_000)
    await expect(toast).toBeVisible()
    await toast.getByRole('button', { name: /close/i }).click()
    await expect(toast).toHaveCount(0)

    // In effect: on screen, in the backend, in the file, and after a reload.
    await expect(tasksAtATime(page)).toHaveText(String(unconfirmed))
    expect((await snapshot()).config.ai.jobQueueConcurrency).toBe(unconfirmed)
    expect(onDisk(directories.configPath)).toBe(unconfirmed)

    await page.reload()
    await unlockIfLocked(page)
    await expect(tasksAtATime(page)).toHaveText(String(unconfirmed), {
      timeout: 60_000,
    })
    await expect(durabilityToast(page)).toHaveCount(0)

    // The disk confirms the next save: nothing to say.
    const next = await chooseTasks(page, confirmed)
    expect(next.configDurabilityWarning).toBeUndefined()
    await expect(tasksAtATime(page)).toHaveText(String(confirmed))
    expect((await snapshot()).config.ai.jobQueueConcurrency).toBe(confirmed)
    expect(onDisk(directories.configPath)).toBe(confirmed)
    // The save has answered and the screen shows it; a warning it raised
    // would be up now (a toast stays for seconds).
    await expect(durabilityToast(page)).toHaveCount(0)
    await expect(page.getByText('Could not save this setting')).toHaveCount(0)
  } finally {
    clear?.()
    const current = (await snapshot()).config
    await backend('save_config', {
      config: {
        ...current,
        ai: { ...current.ai, jobQueueConcurrency: original },
      } satisfies AppConfig,
    })
  }
})
