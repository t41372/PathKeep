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
 *   overview and in the switcher's line;
 * - it adds no new notes: the empty Claude Code file and the cut-off Codex
 *   line the first save noted are listed again but not as new, the run
 *   succeeded with nothing unsaved and nothing asks for a look; its row,
 *   its notice and the source rows say nothing is new.
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
  agentRunWhole,
  agentRuns,
  agentSources,
  agentTasks,
  conversationsText,
  count,
  noteIsAbout,
  notesText,
  openBackupPage,
  newestRunRef,
  sourceNotes,
  sourceRow,
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

test('a second backup counts what changed, keeps what a tool deleted, and adds no new notes', async ({
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
  const detail = await agentRunWhole(run.runRef)
  expect(count(detail.summary.newRoots)).toBe(1)
  expect(count(detail.summary.updatedRoots)).toBe(1)
  await expect(
    page.getByText(/^Agent conversations backed up/).first(),
  ).toBeVisible()

  // The same files in the same condition: listed again, nothing new.
  expect(detail.warnings).toEqual([])
  expect(run.result).toBe('success')
  const { emptyFile, malformed } = agents.scenes
  const notes = count(detail.noteCount)
  expect(notes).toBe(detail.notes.length)
  expect(count(detail.newNoteCount)).toBe(0)
  expect(detail.notes.some((note) => note.isNew)).toBe(false)
  expect(
    detail.notes.find((note) => noteIsAbout(note, emptyFile.file))?.code,
  ).toBe('empty_transcript')
  expect(
    detail.notes
      .filter((note) => noteIsAbout(note, malformed.file))
      .map((note) => note.code),
  ).toContain('malformed_record')
  expect(
    (await agentTasks('attention')).filter((t) => t.runRef === run.runRef),
  ).toEqual([])
  // Its notice says nothing about notes that are not new.
  await expect(page.getByText(/new notes? about reading it/)).toHaveCount(0)
  const sources = await agentSources()
  for (const tool of ['claude-code', 'codex'] as const) {
    expect(sourceNotes(sources, tool).fresh, `${tool}: new notes`).toBe(0)
    await sourceRow(page, tool)
      .getByRole('button', { name: /: see what is kept$/ })
      .click()
    const kinds = sourceRow(page, tool).locator('li[data-note-code]')
    await expect(kinds.first()).toBeVisible()
    for (const kind of await kinds.all())
      await expect(kind).not.toContainText(/\d+ new$/)
  }

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
  const row = page.locator(`li[data-run-ref="${run.runRef}"]`)
  await expect(row).toContainText(notesText(notes))
  await expect(row).not.toContainText(/notes? · \d+ new/)
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
