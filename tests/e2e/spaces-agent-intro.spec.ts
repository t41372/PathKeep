/**
 * Where someone already using PathKeep meets Agent conversations while they
 * are off (`data-spaces.md` §2, §3.1): the card on browsing's Home, and any
 * address of the Agent space.
 *
 * Proves, against the synthetic tool folders (`CHB_AGENT_TOOLS_HOME`):
 * - the card names exactly the tools whose folder is on this computer, in
 *   the app's order, as the backend's presence check reports them, and
 *   follows the folders: with Pi's folder gone it no longer names Pi;
 * - its `Preview what would be saved` opens the enable dialog over Home;
 * - putting it away is kept by the backend with this computer's settings
 *   (`dismiss_agent_hint`): when the backend refuses (×'s request cut off
 *   on its way), the card comes back and says it couldn't be hidden;
 *   `Not now` then puts it away for good: the settings list it, and
 *   neither a reload nor a new window (no browser storage) shows it or even
 *   looks for the tools' folders;
 * - `#/agent/…` shows that the kind is off, with both ways to turn it on,
 *   while the rail stays on browsing (no Agent hue, no Agent rail);
 *   `Preview and turn on` opens the dialog over that page and closing it
 *   leaves the person there; `Manage data types` opens Settings.
 *
 * Putting the card away cannot be undone, so the test that does it runs
 * last, and no spec after this one uses the card: they open the dialog
 * from the Agent space's off page or by its address.
 */
import { existsSync, renameSync } from 'node:fs'
import { expect, test, type Page } from '@playwright/test'
import { resolveDesktopBridgeEnv } from '../../scripts/pathkeep-dev-desktop-bridge.mjs'
import { unlockIfLocked } from './support/app'
import {
  enableDialog,
  expectedToolNames,
  TOOL_NAMES,
} from './support/agent-tools'
import { backend, expectCount, fixture } from './support/fixture'

const card = (page: Page) =>
  page.getByRole('region', {
    name: 'PathKeep can now keep your Agent conversations too',
  })

const cardTools = (page: Page) =>
  card(page)
    .getByRole('list', { name: 'Folders of these tools are on this computer:' })
    .getByRole('listitem')

interface Presence {
  sources: { kind: string; tool?: keyof typeof TOOL_NAMES; found: boolean }[]
}

/** The tools the backend's presence check finds, in the app's order. */
async function foundByBackend() {
  const presence = await backend<Presence>('detect_data_sources', {
    request: { kinds: ['agent-history'] },
  })
  const found = new Set(
    presence.sources
      .filter((source) => source.kind === 'agent-history' && source.found)
      .map((source) => source.tool!),
  )
  return (Object.keys(TOOL_NAMES) as (keyof typeof TOOL_NAMES)[])
    .filter((tool) => found.has(tool))
    .map((tool) => TOOL_NAMES[tool])
}

/** The hints put away on this computer, as the backend keeps them. */
async function dismissedHints() {
  const settings = await backend<{ dismissedHints?: string[] }>(
    'get_agent_settings',
    { request: {} },
  )
  return settings.dismissedHints ?? []
}

/**
 * Opens Home in `page` (a reload, or a new window) and proves the card
 * stays away for the right reason: the app knows Agent conversations are
 * off (its kinds answer arrived), the backend's settings say the card was
 * put away, and the card did not even look for the tools' folders.
 */
async function expectPutAway(page: Page, open: () => Promise<unknown>) {
  const looked: string[] = []
  const onRequest = (request: { url: () => string }) => {
    if (request.url().endsWith('/commands/detect_data_sources'))
      looked.push(request.url())
  }
  page.on('request', onRequest)
  try {
    const kindsAnswered = page.waitForResponse((response) =>
      response.url().endsWith('/commands/list_data_kinds'),
    )
    const settingsAnswered = page.waitForResponse((response) =>
      response.url().endsWith('/commands/get_agent_settings'),
    )
    await open()
    await unlockIfLocked(page)
    await kindsAnswered
    await settingsAnswered
    await expect(page.getByRole('link', { name: /Saved visits/ })).toBeVisible()
    // Two frames: the answers have been drawn, and a card that was not put
    // away would have asked for the tools' folders by now.
    await page.evaluate(
      () =>
        new Promise((resolve) =>
          requestAnimationFrame(() => requestAnimationFrame(resolve)),
        ),
    )
    await expect(card(page)).toHaveCount(0)
    expect(looked).toEqual([])
  } finally {
    page.off('request', onRequest)
  }
}

test('the Home card names exactly the tools found, follows them, and opens the preview', async ({
  page,
}, testInfo) => {
  const expected = expectedToolNames()
  expect(expected.length, 'the fixture writes every tool').toBe(4)
  expect(await foundByBackend()).toEqual(expected)
  expect(await dismissedHints()).not.toContain('intro')

  await page.goto('/#/browsing/home')
  await unlockIfLocked(page)
  await expect(card(page)).toBeVisible()
  await expect(cardTools(page)).toHaveText(expected)
  await expect(
    card(page).getByText(
      'PathKeep only checked that the folders are there. Nothing in them has been read.',
    ),
  ).toBeVisible()
  await expectCount(
    testInfo,
    'tools on the card',
    await cardTools(page).count(),
    expected.length,
  )
  await testInfo.attach('intro card', {
    body: await card(page).screenshot(),
    contentType: 'image/png',
  })

  // Pi's folder goes away: the next look names the other three only.
  const piRoot = fixture().agents.roots.pi
  renameSync(piRoot, `${piRoot}.away`)
  try {
    const withoutPi = expected.filter((name) => name !== 'Pi')
    expect(await foundByBackend()).toEqual(withoutPi)
    await page.reload()
    await unlockIfLocked(page)
    await expect(cardTools(page)).toHaveText(withoutPi)
  } finally {
    if (existsSync(`${piRoot}.away`)) renameSync(`${piRoot}.away`, piRoot)
  }
  await page.reload()
  await unlockIfLocked(page)
  await expect(cardTools(page)).toHaveText(expected)

  // Its preview opens the dialog over Home.
  await card(page)
    .getByRole('button', { name: 'Preview what would be saved' })
    .click()
  await expect(page).toHaveURL(/#\/browsing\/home\?enable=agent$/)
  await expect(enableDialog(page)).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(enableDialog(page)).toBeHidden()
  await expect(page).toHaveURL(/#\/browsing\/home\??$/)
  await expect(card(page)).toBeVisible()
})

test('an Agent address says the kind is off, and both ways to turn it on work', async ({
  page,
}, testInfo) => {
  await page.goto('/#/agent/insights')
  await unlockIfLocked(page)
  const heading = page.getByRole('heading', {
    name: '“Agent conversations” is off',
  })
  await expect(heading).toBeVisible()
  await expect(
    page.getByText(
      'PathKeep isn’t keeping this kind of data now. What it saved before is still in the archive, and you’ll see it again once you turn it on.',
    ),
  ).toBeVisible()
  // The rail stays on browsing: the kind that is off is no space.
  const rail = page.getByRole('navigation', { name: 'Main navigation' })
  await expect(
    rail.getByRole('button', {
      name: 'Switch data type. Now: Browsing history',
    }),
  ).toBeVisible()
  await expect(
    rail.getByRole('link', { name: 'History', exact: true }),
  ).toBeVisible()
  await expect(page.locator('html')).toHaveAttribute('data-space', 'browsing')
  await expect(page.locator('html')).not.toHaveAttribute('data-space-hue')
  await testInfo.attach('Agent address while off', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  // Any address in the space, however deep.
  await page.goto('/#/agent/chats/some/conversation')
  await unlockIfLocked(page)
  await expect(heading).toBeVisible()

  // The preview opens over this page, and closing it stays here.
  await page
    .getByRole('button', { name: 'Preview and turn on Agent conversations' })
    .click()
  await expect(page).toHaveURL(
    /#\/agent\/chats\/some\/conversation\?enable=agent$/,
  )
  await expect(enableDialog(page)).toBeVisible()
  await enableDialog(page).getByRole('button', { name: 'No thanks' }).click()
  await expect(enableDialog(page)).toBeHidden()
  await expect(page).toHaveURL(/#\/agent\/chats\/some\/conversation\??$/)
  await expect(heading).toBeVisible()

  await page.getByRole('link', { name: 'Manage data types' }).click()
  await expect(page).toHaveURL(/#\/settings\/kinds$/)
  await expect(
    page.getByRole('heading', { name: 'Manage data types', level: 2 }),
  ).toBeVisible()
})

// Last: putting the card away cannot be undone.
test('the card comes back when the backend refuses to put it away, and “Not now” puts it away for good', async ({
  page,
  browser,
}, testInfo) => {
  await page.goto('/#/browsing/home')
  await unlockIfLocked(page)
  await expect(card(page)).toBeVisible()

  // × with its request cut off on the way: the card hides, comes back, and
  // says it couldn't be hidden.
  const { devIpcUrl } = resolveDesktopBridgeEnv(process.env)
  const command = `${devIpcUrl}/commands/dismiss_agent_hint`
  let refused = 0
  await page.route(command, async (route) => {
    refused += 1
    await route.abort()
  })
  await card(page).getByRole('button', { name: 'Hide this card' }).click()
  await expect(page.getByText('Couldn’t hide this card')).toBeVisible()
  await expect(card(page)).toBeVisible()
  // (A CORS preflight may be what was cut off; either way nothing arrived.)
  expect(refused).toBeGreaterThan(0)
  expect(await dismissedHints()).not.toContain('intro')
  await testInfo.attach('the card after a refused ×', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
  await page.unroute(command)

  // "Not now" reaches the backend, which keeps it with this computer's
  // settings.
  await card(page).getByRole('button', { name: 'Not now' }).click()
  await expect(card(page)).toBeHidden()
  await expect.poll(dismissedHints).toContain('intro')
  await expectPutAway(page, () => page.reload())
  await testInfo.attach('Home after the card was put away and reloaded', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  // A new window has none of this one's browser storage: only the backend
  // can keep the card away there.
  const context = await browser.newContext({
    baseURL: testInfo.project.use.baseURL,
    viewport: { width: 1440, height: 900 },
    locale: 'en-US',
  })
  try {
    const other = await context.newPage()
    await expectPutAway(other, () => other.goto('/#/browsing/home'))
    await testInfo.attach('Home in a new window', {
      body: await other.screenshot(),
      contentType: 'image/png',
    })
  } finally {
    await context.close()
  }
})
