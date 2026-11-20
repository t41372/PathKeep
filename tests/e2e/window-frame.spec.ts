/**
 * The window frame once the archive is open: the main window, a dialog over
 * it, and App Lock. Runs after lock.spec, so App Lock has passcode 4826.
 *
 * Proves, at the smallest window size:
 * - Windows and Linux: the main window has exactly one set of window
 *   buttons, the strip of window background beside them drags the window
 *   (and a double click there maximizes, by the same mark), and the buttons
 *   still take a click while the command palette is open, without closing
 *   it, as the system's title bar would. A sheet at the window's edge (a
 *   backup run's details) starts below the buttons, so its own close button
 *   can be clicked and closes the sheet, not the window. The empty part of
 *   the nav rail drags the window. App Lock keeps the buttons and drags from
 *   its background.
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
  SMALLEST_WINDOW,
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

/**
 * A point in the rail's empty part: the middle of the gap between the last
 * page link and the first control below it (the tallest gap between two
 * neighbours in the rail).
 */
async function railGap(page: Page) {
  const rail = page.getByRole('navigation', { name: 'Main navigation' })
  const railBox = (await rail.boundingBox())!
  const boxes = (
    await Promise.all(
      (await rail.locator('a, button').all()).map((item) => item.boundingBox()),
    )
  )
    .filter((box) => box !== null)
    .sort((a, b) => a.y - b.y)
  let gap = { top: 0, height: 0 }
  for (let i = 1; i < boxes.length; i += 1) {
    const top = boxes[i - 1].y + boxes[i - 1].height
    const height = boxes[i].y - top
    if (height > gap.height) gap = { top, height }
  }
  expect(gap.height, 'the rail has an empty part').toBeGreaterThan(40)
  return {
    x: Math.round(railBox.x + railBox.width / 2),
    y: Math.round(gap.top + gap.height / 2),
    where: 'empty part of the nav rail',
  }
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
  // The rail names its shortcuts by the keys this keyboard has.
  await expect(
    page.getByRole('button', { name: 'Command palette (Ctrl+K)' }),
  ).toBeVisible()
  await expect(
    page.getByRole('button', { name: 'Lock PathKeep (Ctrl+L)' }),
  ).toBeVisible()
  await page.screenshot({ path: test.info().outputPath('home-linux.png') })

  // The command palette is a modal dialog: the rest of the window stops
  // taking clicks, but the window buttons stand in for the system's, so a
  // click on one reaches it and leaves the palette open, its field focused.
  await page.keyboard.press('Control+k')
  const palette = page.getByPlaceholder(
    'Search conversations, projects, pages…',
  )
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

  // The empty part of the rail, between the space's pages and the controls at
  // its foot, is window background too.
  await expectDragsAt(page, [await railGap(page)])

  // A run's details open as a sheet down the right edge, where the window
  // buttons are. Nothing of it may lie under them, and its own close button
  // closes it.
  await page.goto('/#/browsing/backup')
  await page
    .getByRole('heading', { name: 'Recent runs' })
    .locator('xpath=ancestor::*[.//li][1]')
    .getByRole('listitem')
    .first()
    .getByRole('button')
    .click()
  const sheet = page.getByRole('dialog')
  await expect(sheet).toBeVisible()
  // The sheet slides in; its place is checked once it has stopped.
  await expect
    .poll(async () => {
      const box = await sheet.boundingBox()
      return box ? Math.round(box.x + box.width) : 0
    })
    .toBe(SMALLEST_WINDOW.width)
  await expectWindowButtons(page)
  await page.screenshot({ path: test.info().outputPath('sheet-linux.png') })
  const closeSheet = sheet.getByRole('button', { name: 'Close', exact: true })
  const closeBox = await closeSheet.boundingBox()
  expect(closeBox, 'the sheet has a close button').not.toBeNull()
  await page.mouse.click(
    closeBox!.x + closeBox!.width / 2,
    closeBox!.y + closeBox!.height / 2,
  )
  await expect(sheet).toBeHidden()
  await expectWindowButtons(page)

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
  await expect(
    page.getByRole('button', { name: 'Command palette (⌘K)' }),
  ).toBeVisible()
  await expect(
    page.getByRole('button', { name: 'Lock PathKeep (⌘L)' }),
  ).toBeVisible()
  await expectDragsAt(page, [
    { x: 540, y: 4, where: 'margin above the panel' },
    { x: 1076, y: 400, where: 'margin right of the panel' },
    await railGap(page),
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
