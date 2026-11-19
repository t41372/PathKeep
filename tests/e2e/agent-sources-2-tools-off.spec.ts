/**
 * Agent conversations on with every tool switched off (`data-spaces.md`
 * §8, `agent-history.md` §6): allowed while browsing history is on, and
 * then nothing of them is backed up (`sourceReviewRequired`). Starts from
 * what the specs before leave: both kinds on, some tools switched on with
 * approved folders (Claude Code and Codex).
 *
 * Proves:
 * - switching each tool off on the backup page leaves the kind on with
 *   nothing to back up: the backend says so; the page says every tool is
 *   switched off, "Back up now" is gone and the switches still work;
 * - Manage data types says it and offers "Choose sources", which opens the
 *   backup page; browsing history can be neither paused nor turned off,
 *   says why, and the backend refuses the pause (`source-selection-empty`);
 * - "Back up everything now" backs up browsing history and says Agent
 *   conversations had nothing chosen, with "Choose sources", which opens
 *   their backup page; no conversation backup is made;
 * - switching the tools back on brings "Back up now" back, and a backup
 *   reads them.
 *
 * The tools are switched back on at the end whatever happens, so the specs
 * after this one find them as they were.
 */
import { expect, test, type Page } from '@playwright/test'
import type { AgentTool } from '../../scripts/fixtures/synthetic-agents.mjs'
import {
  agentKindState,
  agentRuns,
  dataKinds,
  lastBrowsingRun,
  newestRunRef,
  openBackupPage,
  openTaskList,
  setAgentTools,
  sourceRow,
  waitForBrowsingRunAfter,
  waitForRunAfter,
} from './support/agent-backup'
import { TOOL_NAMES, agentSettings } from './support/agent-tools'
import { unlockIfLocked } from './support/app'
import { backend } from './support/fixture'

const SWITCHED_OFF =
  'Agent conversations are on, but no tool under Sources is switched on, so nothing is backed up. Switch one on, or pause Agent conversations in Manage data types.'
const NOTHING_CHOSEN =
  '“Agent conversations” has nothing chosen to back up, so it wasn’t backed up.'
const HELD =
  '“Browsing history” has to stay on: “Agent conversations” is on but has nothing to back up until its sources are chosen.'

const card = (page: Page, name: string) =>
  page.getByRole('region', { name, exact: true })
const states = (page: Page, name: string) =>
  card(page, name).getByRole('radiogroup', { name: `State of ${name}` })
const toolSwitch = (page: Page, tool: AgentTool) =>
  sourceRow(page, tool).getByRole('switch', {
    name: `Back up ${TOOL_NAMES[tool]}`,
  })

test('with every tool switched off, the kind stays on, says so, and holds browsing on', async ({
  page,
}, testInfo) => {
  const tools = [...(await agentSettings()).policy.enabledTools]
  expect(
    tools.length,
    'the specs before leave tools switched on',
  ).toBeGreaterThan(0)
  expect((await dataKinds()).kinds.map((row) => row.state)).toEqual([
    'on',
    'on',
  ])

  try {
    // Each tool off, from the backup page.
    await openBackupPage(page)
    for (const tool of tools) {
      await toolSwitch(page, tool).click()
      await expect
        .poll(async () => (await agentSettings()).policy.enabledTools)
        .not.toContain(tool)
    }
    expect(await agentKindState()).toMatchObject({
      state: 'on',
      sourceReviewRequired: true,
    })
    await expect(
      page.getByText('Every tool is switched off', { exact: true }),
    ).toBeVisible()
    await expect(page.getByText(SWITCHED_OFF)).toBeVisible()
    await expect(page.getByRole('button', { name: 'Back up now' })).toHaveCount(
      0,
    )
    for (const tool of tools) {
      await expect(toolSwitch(page, tool)).toBeEnabled()
      await expect(toolSwitch(page, tool)).not.toBeChecked()
    }
    await testInfo.attach('every tool switched off', {
      body: await page.screenshot({ fullPage: true }),
      contentType: 'image/png',
    })

    // Manage data types: said on the card; browsing history is held on.
    await page.goto('/#/settings/kinds')
    await unlockIfLocked(page)
    await expect(card(page, 'Agent conversations')).toContainText(
      'On, but nothing on this computer is chosen to back up, so nothing is backed up. Everything saved is still here.',
    )
    for (const state of ['Paused', 'Off'])
      await expect(
        states(page, 'Browsing history').getByRole('radio', { name: state }),
      ).toBeDisabled()
    await expect(states(page, 'Browsing history')).toHaveAccessibleDescription(
      HELD,
    )
    const { policyToken } = await dataKinds()
    await expect(
      backend('set_data_kind_state', {
        request: {
          kind: 'browsing',
          state: 'paused',
          expectedPolicyToken: policyToken,
        },
      }),
    ).rejects.toThrow(/source-selection-empty/)
    expect((await dataKinds()).kinds.map((row) => row.state)).toEqual([
      'on',
      'on',
    ])
    await testInfo.attach('Manage data types', {
      body: await page.screenshot({ fullPage: true }),
      contentType: 'image/png',
    })
    await card(page, 'Agent conversations')
      .getByRole('button', { name: 'Choose sources' })
      .click()
    await expect(page).toHaveURL(/#\/agent\/backup$/)

    // "Back up everything now": browsing history only, and it says why.
    await page.goto('/#/settings/kinds')
    await unlockIfLocked(page)
    const runsBefore = (await agentRuns()).length
    const browsingBefore = await lastBrowsingRun()
    const list = await openTaskList(page)
    await list.getByRole('button', { name: 'Back up everything now' }).click()
    const notice = page
      .locator('[data-sonner-toast]')
      .filter({ hasText: NOTHING_CHOSEN })
    await expect(notice).toBeVisible()
    await testInfo.attach('what Back up everything now said', {
      body: await page.screenshot(),
      contentType: 'image/png',
    })
    await notice.getByRole('button', { name: 'Choose sources' }).click()
    await expect(page).toHaveURL(/#\/agent\/backup$/)
    await waitForBrowsingRunAfter(browsingBefore?.id)
    expect(await agentRuns()).toHaveLength(runsBefore)

    // The tools on again, from the page: it backs them up.
    await openBackupPage(page)
    for (const tool of tools) {
      await toolSwitch(page, tool).click()
      await expect
        .poll(async () => (await agentSettings()).policy.enabledTools)
        .toContain(tool)
    }
    expect(await agentKindState()).not.toHaveProperty('sourceReviewRequired')
    await expect(page.getByText('Every tool is switched off')).toHaveCount(0)
    const previous = await newestRunRef()
    await page.getByRole('button', { name: 'Back up now' }).click()
    const run = await waitForRunAfter(previous)
    expect(run.result).not.toBe('failed')
  } finally {
    await setAgentTools(tools, true).catch(() => undefined)
  }
})
