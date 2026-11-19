/**
 * A second backup after the tools moved on (`agent-history.md` §6): one
 * conversation got a new turn, one is new, one was deleted from its tool.
 * The fixture's mutators do what the tools would; their result is kept for
 * the specs after this one (`replaceAgents`).
 *
 * Proves:
 * - "Back up now" on the backup page backs up only this kind, says it
 *   started, shows the running state in the header, and ends with a notice;
 * - the run counts one new conversation and one with new content
 *   (`get_archive_run`), and its row in Recent backups says the same;
 * - the deleted conversation is still in the archive: the count went up by
 *   one, not down, and it is counted as only in PathKeep, in the backend's
 *   overview and in the switcher's line.
 */
import { expect, test } from '@playwright/test'
import {
  addAgentConversation,
  appendAgentTurns,
  deleteAgentConversation,
  type SyntheticAgents,
} from '../../scripts/fixtures/synthetic-agents.mjs'
import {
  agentOverview,
  agentRun,
  agentRuns,
  conversationsText,
  count,
  openBackupPage,
  newestRunRef,
  waitForRunAfter,
} from './support/agent-backup'
import { fixture, replaceAgents } from './support/fixture'

/** Conversations a scene points at; the mutators leave those alone here. */
function sceneConversations(agents: SyntheticAgents) {
  const keys = new Set<string>()
  const walk = (value: unknown) => {
    if (!value || typeof value !== 'object') return
    for (const [name, inner] of Object.entries(value)) {
      if (name === 'conversation' && typeof inner === 'string') keys.add(inner)
      else walk(inner)
    }
  }
  walk(agents.scenes)
  return keys
}

test('a second backup counts what changed and keeps what a tool deleted', async ({
  page,
}, testInfo) => {
  let agents = fixture().agents
  const scenes = sceneConversations(agents)
  const plain = (tool: string) =>
    agents.conversations.find(
      (c) =>
        c.tool === tool &&
        !scenes.has(c.key) &&
        c.files.every((file) => !file.endsWith('.zst')),
    )
  const appended = plain('claude-code')
  const deleted = plain('codex')
  expect(appended, 'a plain Claude Code conversation').toBeDefined()
  expect(deleted, 'a plain Codex conversation').toBeDefined()

  const before = await agentOverview()
  const runsBefore = (await agentRuns()).length

  agents = appendAgentTurns(agents, appended!.key)
  agents = addAgentConversation(agents, 'codex')
  agents = deleteAgentConversation(agents, deleted!.key)
  replaceAgents(agents)

  await openBackupPage(page)
  const previous = await newestRunRef()
  await page.getByRole('button', { name: 'Back up now' }).click()
  await expect(page.getByText('Backing up “Agent conversations”')).toBeVisible()
  await expect(page.getByText(/^Backing up · /)).toBeVisible()
  await testInfo.attach('the header while it backs up', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  const run = await waitForRunAfter(previous)
  expect(run.trigger).toBe('manual')
  expect(await agentRuns()).toHaveLength(runsBefore + 1)
  const detail = await agentRun(run.runRef)
  expect(count(detail.summary.newRoots)).toBe(1)
  expect(count(detail.summary.updatedRoots)).toBe(1)
  await expect(
    page.getByText(/^Agent conversations backed up/).first(),
  ).toBeVisible()

  // The deleted one is kept, and counted as only in PathKeep.
  const after = await agentOverview()
  expect(count(after.roots)).toBe(count(before.roots) + 1)
  expect(count(after.onlyInPathkeep)).toBe(count(before.onlyInPathkeep) + 1)
  const codex = (o: typeof after) =>
    count(o.perTool.find((tool) => tool.tool === 'codex')?.onlyInPathkeep)
  expect(codex(after)).toBe(codex(before) + 1)

  // The page says the same.
  await expect(
    page.getByText(
      `Agent conversations · ${conversationsText(count(after.roots))}.`,
    ),
  ).toBeVisible()
  await page.getByRole('button', { name: /^Recent backups/ }).click()
  await expect(
    page.getByText('1 new conversation, 1 with new content.').first(),
  ).toBeVisible()
  const switcher = page.getByRole('button', {
    name: /^Switch data type\. Now: /,
  })
  await switcher.click()
  await expect(
    page
      .getByRole('menu', { name: /^Switch data type/ })
      .getByRole('menuitemradio', { name: /Agent conversations/ }),
  ).toContainText(
    `${conversationsText(count(after.roots))} · ${count(after.onlyInPathkeep)} only in PathKeep`,
  )
  await testInfo.attach('the switcher after the second backup', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
  await page.keyboard.press('Escape')
  await testInfo.attach('the backup page after the second backup', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })
})
