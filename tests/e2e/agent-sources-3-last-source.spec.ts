/**
 * The last source of the only kind on (`data-spaces.md` §8): with browsing
 * history paused, Agent conversations are the only kind on, and with one
 * tool switched on, that tool is their last source. Starts from both kinds
 * on and at least two tools switched on with approved folders (Claude Code
 * and Codex, as agent-sources-2 leaves them).
 *
 * Proves:
 * - with every tool but the first switched off and browsing history paused
 *   in Manage data types, the first tool's switch on the backup page is on
 *   and cannot be used, and the card says why (as the switch's
 *   description); the other tools' switches still work;
 * - the backend refuses switching it off anyway (`source-selection-empty`),
 *   and it stays on;
 * - switching another tool on lifts the lock.
 *
 * Browsing history is turned on and the tools switched back on at the end,
 * whatever happens.
 */
import { expect, test, type Page } from '@playwright/test'
import type { AgentTool } from '../../scripts/fixtures/synthetic-agents.mjs'
import {
  dataKinds,
  openBackupPage,
  setAgentTools,
  setKindState,
  sourceRow,
} from './support/agent-backup'
import { TOOL_NAMES, agentSettings } from './support/agent-tools'
import { unlockIfLocked } from './support/app'
import { backend } from './support/fixture'

const toolSwitch = (page: Page, tool: AgentTool) =>
  sourceRow(page, tool).getByRole('switch', {
    name: `Back up ${TOOL_NAMES[tool]}`,
  })

test('the last tool of the only kind on cannot be switched off', async ({
  page,
}, testInfo) => {
  const tools = [...(await agentSettings()).policy.enabledTools]
  expect(tools.length, 'two tools switched on to start').toBeGreaterThan(1)
  const [last, ...others] = tools
  const lastNote = `${TOOL_NAMES[last]} is the only source of Agent conversations, and they are the only kind of data that’s on, so it can’t be switched off. Turn on another tool or browsing history first.`

  try {
    // The others off while browsing history is on; then browsing paused.
    await openBackupPage(page)
    for (const tool of others) {
      await toolSwitch(page, tool).click()
      await expect
        .poll(async () => (await agentSettings()).policy.enabledTools)
        .not.toContain(tool)
    }
    await expect(toolSwitch(page, last)).toBeEnabled()
    await page.goto('/#/settings/kinds')
    await unlockIfLocked(page)
    await page
      .getByRole('radiogroup', { name: 'State of Browsing history' })
      .getByRole('radio', { name: 'Paused' })
      .click()
    await expect
      .poll(async () => (await dataKinds()).kinds.map((row) => row.state))
      .toEqual(['paused', 'on'])

    // The last source is locked on, and says why.
    await openBackupPage(page)
    await expect(page.getByText(lastNote)).toBeVisible()
    await expect(toolSwitch(page, last)).toBeChecked()
    await expect(toolSwitch(page, last)).toBeDisabled()
    await expect(toolSwitch(page, last)).toHaveAccessibleDescription(lastNote)
    for (const tool of others)
      await expect(toolSwitch(page, tool)).toBeEnabled()
    await testInfo.attach('the last source, locked on', {
      body: await page.screenshot({ fullPage: true }),
      contentType: 'image/png',
    })

    // Forced through the backend anyway: refused, and nothing changes.
    const settings = await agentSettings()
    await expect(
      backend('update_agent_sources', {
        request: {
          expectedToken: settings.policyToken,
          toolChanges: { [last]: false },
        },
      }),
    ).rejects.toThrow(/source-selection-empty/)
    expect((await agentSettings()).policy.enabledTools).toEqual([last])

    // Another tool on: no last source any more.
    await toolSwitch(page, others[0]).click()
    await expect
      .poll(async () => (await agentSettings()).policy.enabledTools)
      .toContain(others[0])
    await expect(page.getByText(lastNote)).toHaveCount(0)
    await expect(toolSwitch(page, last)).toBeEnabled()
  } finally {
    await setKindState('browsing', 'on').catch(() => undefined)
    await setAgentTools(tools, true).catch(() => undefined)
  }
})
