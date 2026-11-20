/**
 * Switching between the two spaces, now that Agent conversations are on
 * (`data-spaces.md` §1), on the archive the agent projects left. The
 * one-space shell is in `spaces-shell.spec.ts`; this is what it could not
 * test without a second space.
 *
 * Proves:
 * - the switcher's menu lists both spaces, each with its status line
 *   (Agent conversations: the backend's count of conversations and of
 *   those only in PathKeep), its shortcut and a check on the current one;
 *   with every kind on there is nothing more to keep;
 * - choosing Agent conversations there opens its first page and the rail
 *   lists its pages; ⌘1 / ⌘2 (Ctrl off macOS) and ⌘K's "Switch to" switch
 *   too; `#/` reopens the space used last;
 * - the rail's items slide sideways as the space changes, and with "reduce
 *   motion" they only fade;
 * - a paused kind's space stays in the switcher and in ⌘K.
 *
 * Ends on browsing history with Agent conversations on, as it found them.
 */
import { expect, test, type Page } from '@playwright/test'
import { agentKindState, setKindState } from './support/agent-backup'
import { unlockIfLocked } from './support/app'
import { backend } from './support/fixture'

const mac = process.platform === 'darwin'
const key = (n: number) => `${mac ? 'Meta' : 'Control'}+${n}`

const rail = (page: Page) =>
  page.getByRole('navigation', { name: 'Main navigation' })
const switcher = (page: Page) =>
  page.getByRole('button', { name: /^Switch data type\. Now: / })

async function visit(page: Page, path: string) {
  await page.goto(path)
  await unlockIfLocked(page)
}

/** Watches the rail's sliding items each frame while `act` switches. */
async function sampleSlide(page: Page, act: () => Promise<void>) {
  await page.evaluate(() => {
    const samples: { x: number; opacity: number }[] = []
    ;(window as unknown as { slideSamples: typeof samples }).slideSamples =
      samples
    const until = performance.now() + 700
    const frame = () => {
      for (const element of document.querySelectorAll(
        '[data-space-slide] > div',
      )) {
        const style = getComputedStyle(element)
        const x =
          style.transform === 'none' ? 0 : new DOMMatrix(style.transform).m41
        samples.push({ x, opacity: Number(style.opacity) })
      }
      if (performance.now() < until) requestAnimationFrame(frame)
    }
    requestAnimationFrame(frame)
  })
  await act()
  await page.waitForTimeout(800)
  return page.evaluate(
    () =>
      (
        window as unknown as {
          slideSamples: { x: number; opacity: number }[]
        }
      ).slideSamples,
  )
}

test('two spaces: the menu, its status lines, the shortcuts, ⌘K, #/ and the slide', async ({
  page,
}, testInfo) => {
  await visit(page, '/#/browsing/history')
  await expect(switcher(page)).toHaveAccessibleName(
    'Switch data type. Now: Browsing history',
  )

  // The menu: both spaces, each with its line, shortcut and check.
  const overview = await backend<{ roots: string; onlyInPathkeep: string }>(
    'get_agent_overview',
    { request: { purpose: 'space' } },
  )
  const roots = Number(overview.roots)
  const only = Number(overview.onlyInPathkeep)
  await switcher(page).click()
  const menu = page.getByRole('menu')
  const items = menu.getByRole('menuitemradio')
  await expect(items).toHaveCount(2)
  await expect(items.nth(0)).toContainText('Browsing history')
  await expect(items.nth(0)).toHaveAttribute('aria-checked', 'true')
  await expect(items.nth(0)).toContainText(mac ? '⌘1' : 'Ctrl+1')
  const agent = items.nth(1)
  await expect(agent).toContainText('Agent conversations')
  await expect(agent).toHaveAttribute('aria-checked', 'false')
  await expect(agent).toContainText(mac ? '⌘2' : 'Ctrl+2')
  await expect(agent).toContainText(
    `${roots.toLocaleString('en-US')} conversation${roots === 1 ? '' : 's'} · ${only.toLocaleString('en-US')} only in PathKeep`,
  )
  await expect(
    menu.getByRole('menuitem', { name: /Keep another kind of data/ }),
  ).toHaveCount(0)
  await testInfo.attach('the switcher with two spaces', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  // Choosing Agent conversations: its first page, its pages in the rail;
  // the items slide sideways on the way.
  const slide = await sampleSlide(page, () => agent.click())
  await expect(page).toHaveURL(/#\/agent\/insights$/)
  await expect(switcher(page)).toHaveAccessibleName(
    'Switch data type. Now: Agent conversations',
  )
  for (const name of ['Insights', 'History', 'Projects', 'Backup'])
    await expect(rail(page).getByRole('link', { name })).toBeVisible()
  await expect(rail(page).getByRole('link', { name: 'Home' })).toHaveCount(0)
  expect(
    slide.some((sample) => Math.abs(sample.x) > 2),
    'the items moved sideways',
  ).toBe(true)

  // `#/` reopens the space used last.
  await visit(page, '/#/')
  await expect(page).toHaveURL(/#\/agent\/insights$/)

  // ⌘1 and ⌘2.
  await page.keyboard.press(key(1))
  await expect(page).toHaveURL(/#\/browsing\/home$/)
  await page.keyboard.press(key(2))
  await expect(page).toHaveURL(/#\/agent\/insights$/)

  // ⌘K's "Switch to" lists the other space only.
  await page.keyboard.press('ControlOrMeta+k')
  const palette = page.getByRole('dialog', { name: 'Search and commands' })
  const switchTo = palette.getByRole('group', { name: 'Switch to' })
  await expect(switchTo.getByRole('option')).toHaveCount(1)
  await switchTo.getByRole('option', { name: /Browsing history/ }).click()
  await expect(page).toHaveURL(/#\/browsing\/home$/)

  // Reduce motion: the items fade, they do not move. (The rail reads the
  // setting as it mounts, so the page loads again.)
  await page.emulateMedia({ reducedMotion: 'reduce' })
  await page.reload()
  await unlockIfLocked(page)
  await expect(page).toHaveURL(/#\/browsing\/home$/)
  const still = await sampleSlide(page, () => page.keyboard.press(key(2)))
  await expect(page).toHaveURL(/#\/agent\/insights$/)
  expect(still.length).toBeGreaterThan(0)
  expect(
    still.every((sample) => Math.abs(sample.x) < 0.5),
    'no sideways move',
  ).toBe(true)
  expect(
    still.some((sample) => sample.opacity < 0.95),
    'a cross-fade',
  ).toBe(true)
  await page.emulateMedia({ reducedMotion: null })
  await page.keyboard.press(key(1))
  await expect(page).toHaveURL(/#\/browsing\/home$/)
})

test('a paused kind’s space stays in the switcher and in ⌘K', async ({
  page,
}) => {
  expect((await agentKindState())?.state).toBe('on')
  await setKindState('agent-history', 'paused')
  try {
    await visit(page, '/#/browsing/home')
    await switcher(page).click()
    const agent = page
      .getByRole('menu')
      .getByRole('menuitemradio', { name: /Agent conversations/ })
    await expect(agent).toBeVisible()
    await page.keyboard.press('Escape')

    await page.keyboard.press('ControlOrMeta+k')
    const palette = page.getByRole('dialog', { name: 'Search and commands' })
    await expect(
      palette
        .getByRole('group', { name: 'Switch to' })
        .getByRole('option', { name: /Agent conversations/ }),
    ).toBeVisible()
    await expect(
      palette
        .getByRole('group', { name: 'Go to' })
        .getByRole('option', { name: 'Agent · History' }),
    ).toBeVisible()
    await page.keyboard.press('Escape')
  } finally {
    await setKindState('agent-history', 'on')
  }
  await expect.poll(async () => (await agentKindState())?.state).toBe('on')
})
