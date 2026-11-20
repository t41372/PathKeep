/**
 * A first run that is quit half way and started again (`data-spaces.md`
 * §3.3): the kinds and Agent tools chosen are kept on this computer on
 * each Continue, before any archive exists, and the archive is made with
 * the last ones kept.
 *
 * Proves:
 * - Continue on "What to keep" and on "Sources" keeps what was chosen
 *   (`app_snapshot.config.onboardingChoices`): Agent conversations only,
 *   with Claude Code and Codex, `.git` read;
 * - the app killed on the next step (nothing gets to save on the way out)
 *   and opened again starts at the welcome step, as setup always does, but
 *   "What to keep" has browsing history unticked and Agent conversations
 *   ticked, and the Agent pane of "Sources" has Claude Code and Codex
 *   ticked and the other tools found not, though they start ticked on a
 *   setup with nothing kept;
 * - a choice changed after that (browsing history ticked again) is the one
 *   kept, and the archive is made with it: both kinds are on, the first save
 *   read Claude Code and Codex only, with `.git`, and the kept choices are
 *   gone from the config.
 *
 * Runs in the `onboarding-agent` project on the empty data folder the spec
 * before it leaves, and deletes all data at the end the way a user does.
 */
import { expect, test, type Page } from '@playwright/test'
import type { AgentTool } from '../../scripts/fixtures/synthetic-agents.mjs'
import { killAndRestartApp, unlockIfLocked } from './support/app'
import {
  agentRunWhole,
  dataKinds,
  waitForRunAfter,
} from './support/agent-backup'
import { TOOL_NAMES, expectedToolNames } from './support/agent-tools'
import { backend } from './support/fixture'

const WELCOME = 'Keep what gets deleted automatically'
const CHOSEN: AgentTool[] = ['claude-code', 'codex']

interface Kept {
  kinds: string[]
  agentTools: string[]
  readGit: boolean
}

const kept = async () =>
  (
    await backend<{
      config: { initialized: boolean; onboardingChoices?: Kept }
    }>('app_snapshot')
  ).config

const kindTick = (page: Page, name: string) =>
  page.getByRole('checkbox', { name, exact: true })
const toolTick = (page: Page, tool: AgentTool) =>
  page.getByRole('checkbox', { name: TOOL_NAMES[tool], exact: true })

async function startSetup(page: Page) {
  await expect(page.getByRole('heading', { name: WELCOME })).toBeVisible({
    timeout: 60_000,
  })
  await page.getByRole('button', { name: 'Get started' }).click()
  await expect(
    page.getByRole('heading', { name: 'What should PathKeep keep?' }),
  ).toBeVisible()
}

/** The Agent pane of "Sources", once the walk has listed every tool found. */
async function agentPane(page: Page, found: string[], twoKinds: boolean) {
  await expect(
    page.getByRole('heading', { name: 'Here’s what PathKeep found' }),
  ).toBeVisible()
  if (twoKinds)
    await page
      .getByRole('tablist', { name: 'Kinds of data' })
      .getByRole('tab', { name: /^Agent conversations/ })
      .click()
  await expect(
    page
      .getByRole('list', { name: 'Agent tools found on this computer' })
      .getByRole('listitem'),
  ).toHaveCount(found.length, { timeout: 60_000 })
}

test('choices kept on Continue survive a quit, and the archive is made with the last ones kept', async ({
  page,
}, testInfo) => {
  test.setTimeout(360_000)
  const foundNames = expectedToolNames()
  const found = (Object.keys(TOOL_NAMES) as AgentTool[]).filter((tool) =>
    foundNames.includes(TOOL_NAMES[tool]),
  )
  const leftOut = found.filter((tool) => !CHOSEN.includes(tool))
  expect(
    leftOut.length,
    'some tool found here is left out, so a restored tick is not just the default',
  ).toBeGreaterThan(0)
  const continueButton = page.getByRole('button', { name: 'Continue' })

  await page.goto('/')
  expect((await kept()).initialized).toBe(false)
  await startSetup(page)

  // Agent conversations only; kept on Continue.
  await kindTick(page, 'Browsing history').click()
  await expect(kindTick(page, 'Agent conversations')).toBeChecked()
  await continueButton.click()
  await agentPane(page, foundNames, false)
  expect((await kept()).onboardingChoices?.kinds).toEqual(['agent-history'])

  // Two tools of those found, all ticked at first; kept on Continue.
  for (const tool of found) await expect(toolTick(page, tool)).toBeChecked()
  for (const tool of leftOut) await toolTick(page, tool).setChecked(false)
  for (const tool of CHOSEN) await expect(toolTick(page, tool)).toBeChecked()
  await continueButton.click()
  await expect(
    page.getByRole('heading', { name: 'Where the archive lives' }),
  ).toBeVisible()
  expect((await kept()).onboardingChoices).toEqual({
    kinds: ['agent-history'],
    agentTools: CHOSEN,
    readGit: true,
  })

  // Quit half way: killed, nothing gets to save on the way out.
  await killAndRestartApp()
  await page.reload()
  await startSetup(page)
  await expect(kindTick(page, 'Browsing history')).not.toBeChecked()
  await expect(kindTick(page, 'Agent conversations')).toBeChecked()
  await testInfo.attach('What to keep, after the quit', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  // Browsing history again: the latest choice is the one kept.
  await kindTick(page, 'Browsing history').click()
  await continueButton.click()
  await agentPane(page, foundNames, true)
  for (const tool of CHOSEN) await expect(toolTick(page, tool)).toBeChecked()
  for (const tool of leftOut)
    await expect(toolTick(page, tool)).not.toBeChecked()
  await testInfo.attach('Sources, after the quit', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
  await continueButton.click()
  await expect(
    page.getByRole('heading', { name: 'Where the archive lives' }),
  ).toBeVisible()
  expect((await kept()).onboardingChoices).toEqual({
    kinds: ['browsing', 'agent-history'],
    agentTools: CHOSEN,
    readGit: true,
  })

  // The rest, the quick way, and the first backup.
  await continueButton.click()
  await page.getByRole('radio', { name: /^Don’t encrypt/ }).click()
  await continueButton.click()
  await page.getByRole('radio', { name: /^Manual/ }).click()
  await continueButton.click()
  await page.getByRole('radio', { name: /^Not now/ }).click()
  await continueButton.click()
  await page.getByRole('button', { name: 'Start first backup' }).click()
  await expect(
    page.getByRole('status').filter({ hasText: 'Done.' }),
  ).toBeVisible({ timeout: 180_000 })

  // Made with the last choices kept, which are then gone.
  const config = await kept()
  expect(config.initialized).toBe(true)
  expect(config.onboardingChoices).toBeUndefined()
  const states = await dataKinds()
  expect(
    Object.fromEntries(states.kinds.map((row) => [row.kind, row.state])),
  ).toEqual({ browsing: 'on', 'agent-history': 'on' })
  const run = await waitForRunAfter(undefined)
  expect(run.trigger).toBe('enable')
  const detail = await agentRunWhole(run.runRef)
  expect(detail.readGit).toBe(true)
  expect((detail.perTool ?? []).map((tool) => tool.tool).sort()).toEqual(
    [...CHOSEN].sort(),
  )
  await testInfo.attach('the first backup, made with the kept choices', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  // Leave the data folder empty, the way a user deletes everything.
  await page.getByRole('button', { name: 'Open PathKeep' }).click()
  await page.goto('/#/settings/storage')
  await unlockIfLocked(page)
  await page.getByRole('button', { name: 'Delete…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Delete all data?' })
  await dialog.getByRole('textbox').fill('DELETE')
  await dialog.getByRole('button', { name: 'Delete forever' }).click()
  await expect(page.getByRole('heading', { name: WELCOME })).toBeVisible({
    timeout: 60_000,
  })
  expect((await kept()).initialized).toBe(false)
})
