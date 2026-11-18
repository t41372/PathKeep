/**
 * The window frame on the screens before the archive opens: onboarding,
 * starting, and failing to start. Runs before first-run.spec, while the
 * archive is still new and the app opens on onboarding.
 *
 * Proves, at the smallest window size:
 * - Windows and Linux (the window has no system title bar there): each of
 *   these screens has one set of window buttons at the top right that a click
 *   reaches, with nothing of the screen under them, and the window background
 *   along the top drags the window.
 * - macOS: no buttons of ours, and onboarding drags from the window background
 *   around its panel, as the main window does.
 *
 * Starting and failing to start are reached by holding back, then refusing,
 * the app's first question to the backend (`app_lock_status`); the screens
 * themselves are the real ones.
 *
 * What the buttons and the drag do to a real window is not here: see
 * `support/window-frame.ts`.
 */
import { expect, test } from '@playwright/test'
import {
  desktopWindow,
  expectDragsAt,
  expectNoWindowButtons,
  expectWindowButtons,
  titleStrip,
  type FramePlatform,
} from './support/window-frame'

const welcome = 'Your browser is deleting your history'

for (const platform of ['linux', 'windows'] as FramePlatform[]) {
  test(`${platform}: onboarding has the window buttons and drags from the top`, async ({
    browser,
  }) => {
    const page = await desktopWindow(browser, platform)
    await page.goto('/')
    await expect(page.getByRole('heading', { name: welcome })).toBeVisible()

    await expectWindowButtons(page)
    await expectDragsAt(page, [
      ...titleStrip(page),
      { x: 130, y: 12, where: 'top of the step list' },
    ])
    await page.screenshot({
      path: test.info().outputPath(`onboarding-${platform}.png`),
    })
    await page.context().close()
  })
}

test('linux: starting and failing to start keep the window buttons', async ({
  browser,
}) => {
  const page = await desktopWindow(browser, 'linux')
  let release = () => {}
  const held = new Promise<void>((resolve) => (release = resolve))
  await page.route('**/commands/app_lock_status', async (route) => {
    await held
    await route.continue()
  })
  await page.goto('/')
  await expect(page.getByText('Opening your archive…')).toBeVisible()
  await expectWindowButtons(page)
  await expectDragsAt(page, [
    ...titleStrip(page),
    { x: 40, y: 400, where: 'window background left of the message' },
  ])

  await page.screenshot({ path: test.info().outputPath('starting-linux.png') })

  // Let it start, then start it again with the backend refusing.
  release()
  await expect(page.getByRole('heading', { name: welcome })).toBeVisible()
  await page.unroute('**/commands/app_lock_status')
  await page.route('**/commands/app_lock_status', (route) =>
    route.abort('connectionrefused'),
  )
  await page.reload()
  await expect(
    page.getByRole('heading', { name: 'PathKeep could not start' }),
  ).toBeVisible()
  await expectWindowButtons(page)
  await expectDragsAt(page, titleStrip(page))
  await page.screenshot({
    path: test.info().outputPath('start-failed-linux.png'),
  })
  await page.context().close()
})

test('macos: onboarding drags from the window background around its panel', async ({
  browser,
}) => {
  const page = await desktopWindow(browser, 'macos')
  await page.goto('/')
  await expect(page.getByRole('heading', { name: welcome })).toBeVisible()
  await expectNoWindowButtons(page)
  await expectDragsAt(page, [
    { x: 540, y: 4, where: 'margin above the panel' },
    { x: 1076, y: 400, where: 'margin right of the panel' },
    { x: 130, y: 30, where: 'top of the step list, under the traffic lights' },
  ])
  await page.context().close()
})
