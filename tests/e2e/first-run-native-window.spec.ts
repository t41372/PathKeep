/**
 * PathKeep's real window on a Linux desktop, worked with real input
 * (`support/native-window.ts`): the window buttons, the strip beside them, and
 * Close. Runs first, while the app opens on onboarding, so a fault here shows
 * before the rest of the suite has run.
 *
 * Proves, on the real WebKitGTK window under GNOME's window manager and then
 * Xfce's: Maximize maximizes and Restore restores, Minimize minimizes,
 * dragging the strip moves the window, a double click on it maximizes and
 * restores; under Xfce's, a tap on the window background does not leave the
 * window following the pointer (`WORK-DRAG-ON-MOVE`; GNOME's never did). Then
 * Close quits the app (no menu bar icon, no work running). That is the whole
 * way from the page through Tauri's IPC and the window's permissions, which
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
  appWindowIsHere,
  closeButton,
  expectTapsDoNotDrag,
  expectWorkingWindowFrame,
  nativeClick,
  nativeWindowMissing,
  switchToXfwm4,
  useMutter,
  windowManagerAvailable,
} from './support/native-window'

const missing = nativeWindowMissing()
// Set by the environment that installs the tools (CI's gate): there a missing
// piece fails the run instead of skipping the check.
const required = process.env.PATHKEEP_E2E_NATIVE_WINDOW === 'required'
test.skip(missing !== null && !required, `The real window ${missing}.`)

test('linux: the real window: buttons, drag, double click, a tap, and Close quits', async () => {
  test.setTimeout(900_000)
  if (missing)
    throw new Error(`This run requires the real window: it ${missing}.`)
  const pid = await runningPid()
  if (pid === null) throw new Error('The app is not answering.')
  if (!required) {
    test.skip(
      !windowManagerAvailable(),
      'The real window needs a window manager, and mutter is not installed.',
    )
    test.skip(
      !(await appWindowIsHere(pid)),
      'The app’s window is not on this X display.',
    )
  }
  await useMutter()
  const id = await appWindow(pid)
  await expectWorkingWindowFrame(id)

  const noXfwm4 = await switchToXfwm4()
  if (noXfwm4 && required)
    throw new Error(`This run requires Xfce’s window manager: ${noXfwm4}.`)
  if (noXfwm4)
    test.info().annotations.push({
      type: 'skipped',
      description: `Xfce’s window manager: ${noXfwm4}.`,
    })
  else {
    await expectWorkingWindowFrame(id)
    await expectTapsDoNotDrag(id)
  }

  restartAppWhenItExits()
  const close = closeButton(id)
  await nativeClick(close.x, close.y)
  await waitForAppRestart(pid)
})
