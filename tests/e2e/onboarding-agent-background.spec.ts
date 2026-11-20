/**
 * First run that keeps Agent conversations only, where the answer to
 * turning them on is lost and the first save is left to go on in the
 * background (`data-spaces.md` §3.3).
 *
 * Proves:
 * - "What to keep" with browsing history unticked has one pane of sources;
 * - when `enable_agent_history` starts the save but its answer never
 *   reaches the window, setup takes the save that is running instead of
 *   turning the kind on again: the Agent line shows the save going, no
 *   failure, and the archive has one `enable` run, the one the lost answer
 *   named;
 * - "Keep going in the background" stops setup waiting: PathKeep opens by
 *   itself in the Agent space, the toast says the save goes on, browsing
 *   history is off, and the save then ends on its own as a success.
 *
 * Runs in the `onboarding-agent` project on an empty data folder, and
 * deletes all data at the end the way a user does, so the next spec finds
 * the folder empty again.
 */
import { expect, test, type Route } from '@playwright/test'
import { resolveDesktopBridgeEnv } from '../../scripts/pathkeep-dev-desktop-bridge.mjs'
import { unlockIfLocked } from './support/app'
import { agentRun, agentRuns, dataKinds } from './support/agent-backup'
import { backend } from './support/fixture'

const WELCOME = 'Keep what gets deleted automatically'

test('a first save whose start lost its answer is taken over, and can go on in the background', async ({
  page,
}, testInfo) => {
  test.setTimeout(300_000)
  await page.goto('/')
  await expect(page.getByRole('heading', { name: WELCOME })).toBeVisible({
    timeout: 60_000,
  })
  await page.getByRole('button', { name: 'Get started' }).click()

  // Agent conversations only.
  const browsing = page.getByRole('checkbox', {
    name: 'Browsing history',
    exact: true,
  })
  await expect(browsing).toBeChecked()
  await browsing.click()
  await expect(
    page.getByRole('checkbox', { name: 'Agent conversations', exact: true }),
  ).toBeChecked()
  await page.getByRole('button', { name: 'Continue' }).click()
  await expect(
    page.getByRole('heading', { name: 'Here’s what PathKeep found' }),
  ).toBeVisible()
  // One kind, one pane: no list of kinds to move between.
  await expect(
    page.getByRole('tablist', { name: 'Kinds of data' }),
  ).toHaveCount(0)
  await page.getByRole('button', { name: 'Continue' }).click()
  await expect(
    page.getByRole('heading', { name: 'Where the archive lives' }),
  ).toBeVisible()
  await page.getByRole('button', { name: 'Continue' }).click()
  await page.getByRole('radio', { name: /^Don’t encrypt/ }).click()
  await page.getByRole('button', { name: 'Continue' }).click()
  await page.getByRole('radio', { name: /^Manual/ }).click()
  await page.getByRole('button', { name: 'Continue' }).click()
  await page.getByRole('radio', { name: /^Not now/ }).click()
  await page.getByRole('button', { name: 'Continue' }).click()

  // The backend starts the save; the window never hears back.
  const { devIpcUrl } = resolveDesktopBridgeEnv(process.env)
  const enable = `${devIpcUrl}/commands/enable_agent_history`
  let lostRunRef: string | undefined
  await page.route(enable, async (route: Route) => {
    if (route.request().method() !== 'POST' || lostRunRef)
      return route.continue()
    const response = await route.fetch()
    const body = (await response.json()) as { firstSave?: { runRef: string } }
    lostRunRef = body.firstSave?.runRef
    await route.abort('connectionreset')
  })
  await page.getByRole('button', { name: 'Start first backup' }).click()

  const agentLine = page.locator('li[data-kind="agent-history"]')
  const keepGoing = agentLine.getByRole('button', {
    name: 'Keep going in the background',
  })
  await expect(keepGoing).toBeVisible({ timeout: 120_000 })
  await page.unroute(enable)
  expect(lostRunRef, 'the save started before the answer was lost').toBeTruthy()
  await expect(agentLine).not.toContainText('failed')
  await expect(
    agentLine.getByText(
      'The first save reads every conversation, so it can take a while. It doesn’t need this window.',
    ),
  ).toBeVisible()
  const enableRuns = async () =>
    (await agentRuns(20)).filter((run) => run.trigger === 'enable')
  expect((await enableRuns()).map((run) => run.runRef)).toEqual([lostRunRef])
  await testInfo.attach('the save setup took over', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  // Left to go on: setup finishes and PathKeep opens in the Agent space.
  await keepGoing.click()
  await expect(
    page
      .locator('[data-sonner-toast]')
      .filter({ hasText: 'Saving goes on in the background' }),
  ).toBeVisible({ timeout: 60_000 })
  await expect(page).toHaveURL(/#\/agent\//)
  await unlockIfLocked(page)
  const kinds = await dataKinds()
  expect(
    Object.fromEntries(kinds.kinds.map((row) => [row.kind, row.state])),
  ).toEqual({ browsing: 'off', 'agent-history': 'on' })

  // The save ends by itself, still the only one `enable` started.
  await expect
    .poll(async () => (await agentRun(lostRunRef!)).result, {
      timeout: 180_000,
      intervals: [1_000],
    })
    .toBe('success')
  expect((await enableRuns()).map((run) => run.runRef)).toEqual([lostRunRef])

  // Leave the data folder empty, the way a user deletes everything.
  await page.goto('/#/settings/storage')
  await unlockIfLocked(page)
  await page.getByRole('button', { name: 'Delete…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Delete all data?' })
  await dialog.getByRole('textbox').fill('DELETE')
  await dialog.getByRole('button', { name: 'Delete forever' }).click()
  await expect(page.getByRole('heading', { name: WELCOME })).toBeVisible({
    timeout: 60_000,
  })
  const after = await backend<{ config: { initialized: boolean } }>(
    'app_snapshot',
  )
  expect(after.config.initialized).toBe(false)
})
