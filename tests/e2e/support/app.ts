/**
 * The running app as a whole: its lock screen, and stopping it the way a
 * crash would. The E2E launcher (`scripts/pathkeep-dev-desktop-bridge.mjs`,
 * `PATHKEEP_E2E_RESTART_FILE`) starts it again on the same ports and data.
 */
import { writeFileSync } from 'node:fs'
import { expect, type Page } from '@playwright/test'
import { resolveDesktopBridgeEnv } from '../../../scripts/pathkeep-dev-desktop-bridge.mjs'

/** The passcode lock.spec sets for App Lock. */
const PASSCODE = '4826'

/**
 * Unlocks App Lock if the app shows it, once the app has finished starting:
 * right after a launch it shows neither the lock screen nor its frame yet.
 */
export async function unlockIfLocked(page: Page) {
  const lock = page.getByRole('heading', { name: 'PathKeep is locked' })
  await expect(lock.or(page.getByRole('main')).first()).toBeVisible({
    timeout: 60_000,
  })
  if (await lock.isVisible()) {
    await page.getByPlaceholder('Passcode').fill(PASSCODE)
    await page.getByPlaceholder('Passcode').press('Enter')
    await expect(lock).toBeHidden()
  }
}

async function runningPid(): Promise<number | null> {
  const { devIpcUrl } = resolveDesktopBridgeEnv(process.env)
  try {
    const health = (await (await fetch(`${devIpcUrl}/health`)).json()) as {
      pid?: number
    }
    return health.pid ?? null
  } catch {
    return null
  }
}

/** Kills the app (SIGKILL: nothing gets to finish or save) and waits for the new one to answer. */
export async function killAndRestartApp() {
  const restartFile = process.env.PATHKEEP_E2E_RESTART_FILE
  if (!restartFile)
    throw new Error(
      'PATHKEEP_E2E_RESTART_FILE is not set; run via the E2E config.',
    )
  const pid = await runningPid()
  if (pid === null) throw new Error('The app is not answering.')
  writeFileSync(restartFile, '')
  process.kill(pid, 'SIGKILL')
  await expect
    .poll(
      async () => {
        const now = await runningPid()
        return now !== null && now !== pid
      },
      { timeout: 180_000, intervals: [1_000] },
    )
    .toBe(true)
}
