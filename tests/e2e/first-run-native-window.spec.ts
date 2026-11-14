/**
 * PathKeep's real window on a Linux desktop, worked with real input
 * (`support/native-window.ts`): the window buttons, the strip beside them, and
 * Close. Runs first, while the app opens on onboarding, so a fault here shows
 * before the rest of the suite has run.
 *
 * Proves, on the real WebKitGTK window under a real window manager: Maximize
 * maximizes and Restore restores, Minimize minimizes, dragging the strip moves
 * the window, a double click on it maximizes and restores, and Close quits the
 * app (no menu bar icon, no work running). That is the whole way from the page
 * through Tauri's IPC and the window's permissions, which
 * `first-run-window.spec.ts` (the browser standing in for the window) cannot
 * reach. The buttons are one component on every screen; which screens show
 * them is that spec's job.
 *
 * The launcher starts the app again after Close, on the same data, so
 * first-run.spec still finds a new archive.
 *
 * Linux only. CI's gate requires it (`PATHKEEP_E2E_NATIVE_WINDOW=required`);
 * anywhere without the tools it is skipped.
 */
import { test } from '@playwright/test'
import {
  restartAppWhenItExits,
  runningPid,
  waitForAppRestart,
} from './support/app'
import {
  appWindow,
  closeButton,
  ensureWindowManager,
  expectWorkingWindowFrame,
  nativeClick,
  nativeWindowMissing,
} from './support/native-window'

const missing = nativeWindowMissing()
// Set by the environment that installs the tools (CI's gate): there a missing
// piece fails the run instead of skipping the check.
const required = process.env.PATHKEEP_E2E_NATIVE_WINDOW === 'required'
test.skip(missing !== null && !required, `The real window ${missing}.`)

test('linux: the real window: buttons, drag, double click, and Close quits', async () => {
  test.setTimeout(600_000)
  if (missing)
    throw new Error(`This run requires the real window: it ${missing}.`)
  await ensureWindowManager()
  const id = await appWindow()
  await expectWorkingWindowFrame(id)

  const pid = await runningPid()
  if (pid === null) throw new Error('The app is not answering.')
  restartAppWhenItExits()
  const close = closeButton(id)
  await nativeClick(close.x, close.y)
  await waitForAppRestart(pid)
})
