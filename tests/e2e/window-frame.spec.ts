/**
 * The window frame once the archive is open: the main window, a dialog over
 * it, and App Lock. Runs after lock.spec, so App Lock has passcode 4826.
 *
 * Proves, at the smallest window size:
 * - Windows and Linux: the main window has exactly one set of window
 *   buttons, the strip of window background beside them drags the window
 *   (and a double click there maximizes, by the same mark), and the buttons
 *   still take a click while the command palette is open, without closing
 *   it, as the system's title bar would. App Lock keeps the buttons and drags
 *   from its background.
 * - macOS: no buttons of ours, and the window background drags on the main
 *   window and on App Lock.
 *
 * What the buttons and the drag do to a real window is not here: see
 * `support/window-frame.ts`.
 */
import { expect, test, type Page } from '@playwright/test'
import { unlockIfLocked } from './support/app'
import {
  desktopWindow,
  expectDragsAt,
  expectNoWindowButtons,
  expectWindowButtons,
  titleStrip,
  windowButtonCenter,
} from './support/window-frame'

const lockHeading = (page: Page) =>
  page.getByRole('heading', { name: 'PathKeep is locked' })

async function openHome(page: Page) {
  await page.goto('/#/')
  await unlockIfLocked(page)
  await expect(
    page.getByRole('navigation', { name: 'Main navigation' }),
  ).toBeVisible()
}

async function lockAndUnlock(page: Page, check: () => Promise<void>) {
  await page.keyboard.press('ControlOrMeta+l')
  await expect(lockHeading(page)).toBeVisible()
  await check()
  await page.getByPlaceholder('Passcode').fill('4826')
  await page.getByPlaceholder('Passcode').press('Enter')
  await expect(lockHeading(page)).toBeHidden()
}

test('linux: the main window, a dialog over it and App Lock keep one set of window buttons', async ({
  browser,
}) => {
  const page = await desktopWindow(browser, 'linux')
  await openHome(page)
  await expectWindowButtons(page)
  await expectDragsAt(page, titleStrip(page))
  await page.screenshot({ path: test.info().outputPath('home-linux.png') })

  // The command palette is a modal dialog: the rest of the window stops
  // taking clicks, but the window buttons stand in for the system's, so a
  // click on one reaches it and leaves the palette open, its field focused.
  await page.keyboard.press('Control+k')
  const palette = page.getByPlaceholder('Search pages, visits or commands…')
  await expect(palette).toBeVisible()
  await expectWindowButtons(page)
  const minimize = await windowButtonCenter(page, 'Minimize')
  if (minimize) await page.mouse.click(minimize.x, minimize.y)
  await expect.soft(palette, 'palette open after Minimize').toBeVisible()
  await expect.soft(palette, 'palette field keeps the focus').toBeFocused()
  await page.screenshot({
    path: test.info().outputPath('palette-linux.png'),
  })
  if (await palette.isVisible()) await page.keyboard.press('Escape')
  await expect(palette).toBeHidden()

  await lockAndUnlock(page, async () => {
    await expectWindowButtons(page)
    await expectDragsAt(page, [
      ...titleStrip(page),
      { x: 40, y: 400, where: 'lock screen background' },
    ])
    await page.screenshot({ path: test.info().outputPath('lock-linux.png') })
  })
  await expectWindowButtons(page)
  await page.context().close()
})

test('macos: the main window and App Lock drag from the window background', async ({
  browser,
}) => {
  const page = await desktopWindow(browser, 'macos')
  await openHome(page)
  await expectNoWindowButtons(page)
  await expectDragsAt(page, [
    { x: 540, y: 4, where: 'margin above the panel' },
    { x: 1076, y: 400, where: 'margin right of the panel' },
  ])
  await lockAndUnlock(page, async () => {
    await expectNoWindowButtons(page)
    await expectDragsAt(page, [
      { x: 540, y: 16, where: 'lock screen background, top' },
      { x: 40, y: 400, where: 'lock screen background, left' },
    ])
  })
  await page.context().close()
})
