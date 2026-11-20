/**
 * From the Agent space's off page to a finished first save of Agent
 * conversations (`data-spaces.md` §3, `agent-history.md` §6), against the
 * synthetic tool folders (`CHB_AGENT_TOOLS_HOME`). The capture is slowed
 * per file (`PATHKEEP_TEST_AGENT_CAPTURE_FILE_DELAY_US`) so its progress can
 * be seen. (The Home card was put away for good by spaces-agent-intro.spec;
 * the off page's button opens the same dialog.)
 *
 * Proves:
 * - ticking Claude Code and Codex with `.git` off and starting turns the
 *   kind on at once: the switcher lists Agent conversations while the save
 *   still runs;
 * - the Run step shows each ticked tool's progress in files (found, then
 *   saved with their size), never a percentage, and no row for the others;
 * - Done's numbers are the run's (`get_archive_run`), and those are what
 *   the fixture says the two tools hold: every conversation and subagent;
 * - "View conversations" opens the space at Insights; History lists them;
 *   its backup page's header counts the same conversations and its recent
 *   backups list the run;
 *   its details name only the two tools and say `.git` was not read;
 * - Pi and DeepSeek Harness were not read: their rows say not backed up,
 *   and the archive holds nothing of them;
 * - the fixture's 0-byte Claude Code transcript and its Codex rollout with a
 *   line cut off in the middle are saved, and leave notes, not a problem:
 *   the run succeeded with nothing unsaved, every note is new, nothing asks
 *   for a look; Done says everything was saved with that many notes; the
 *   run's row and the header count them; the run's details list both in
 *   plain words and say the files are saved as they are; each tool's
 *   source row counts its notes and, unfolded, names them.
 */
import { expect, test } from '@playwright/test'
import { unlockIfLocked } from './support/app'
import { chatList } from './support/agent-chats'
import { enableDialog, TOOL_NAMES } from './support/agent-tools'
import {
  agentOverview,
  agentRun,
  agentRunWhole,
  agentSources,
  agentTasks,
  chooseAndStart,
  conversationsText,
  count,
  heldBy,
  noteIsAbout,
  notesText,
  sourceNotes,
  sourceRow,
  subagentsText,
  waitForRunAfter,
} from './support/agent-backup'
import { fixture } from './support/fixture'

const CHOSEN = ['claude-code', 'codex'] as const

test('a first save from the off page, with two tools and no .git, leaves notes and no problem', async ({
  page,
}, testInfo) => {
  await page.goto('/#/agent/backup')
  await unlockIfLocked(page)
  await page
    .getByRole('button', { name: 'Preview and turn on Agent conversations' })
    .click()
  await expect(enableDialog(page)).toBeVisible()
  await chooseAndStart(page, CHOSEN, false)

  // On at once: the switcher has the space while the save runs.
  const switcher = page.getByRole('button', {
    name: /^Switch data type\. Now: /,
  })
  await switcher.click()
  const menu = page.getByRole('menu', { name: /^Switch data type/ })
  await expect(
    menu.getByRole('menuitemradio', { name: /Agent conversations/ }),
  ).toBeVisible()
  await page.keyboard.press('Escape')

  // Progress in files for each ticked tool, and no percentage.
  const dialog = enableDialog(page)
  const tools = dialog.getByRole('list', { name: 'Saving, by tool' })
  await expect(tools.getByRole('listitem')).toHaveCount(CHOSEN.length)
  for (const tool of CHOSEN)
    await expect(tools.locator(`li[data-tool="${tool}"]`)).toContainText(
      TOOL_NAMES[tool],
    )
  await expect(
    tools.getByText(/\d+ files? (found so far|saved, )/).first(),
  ).toBeVisible()
  expect(await tools.innerText()).not.toMatch(/\d+\s*%/)
  await testInfo.attach('the first save running', {
    body: await dialog.screenshot(),
    contentType: 'image/png',
  })

  // Done: the run's numbers, which are what the two tools hold.
  await expect(dialog.getByRole('heading', { name: 'Saved' })).toBeVisible({
    timeout: 120_000,
  })
  const run = await waitForRunAfter(undefined)
  expect(run.trigger).toBe('enable')
  const detail = await agentRunWhole(run.runRef)
  // Everything chosen was saved: no warning, so nothing makes it partial.
  // (Listed first, so a red run says which codes it left.)
  expect(detail.warnings).toEqual([])
  expect(run.result).toBe('success')
  expect(count(run.warningCount)).toBe(0)
  const held = heldBy(fixture().agents, CHOSEN)
  const conversations =
    count(detail.summary.newRoots) + count(detail.summary.updatedRoots)
  expect(conversations).toBe(held.conversations)
  expect(count(detail.summary.newChildren)).toBe(held.subagents)
  await expect(dialog).toContainText(
    `Saved ${conversationsText(held.conversations)} and ${subagentsText(held.subagents)}`,
  )
  // The empty file and the cut-off line are notes on saved files, all new.
  const { emptyFile, malformed } = fixture().agents.scenes
  const notes = count(detail.noteCount)
  expect(detail.notes.length).toBeGreaterThanOrEqual(2)
  expect(notes).toBe(detail.notes.length)
  expect(count(detail.newNoteCount)).toBe(notes)
  expect(detail.notes.every((note) => note.isNew)).toBe(true)
  const empty = detail.notes.find((note) => noteIsAbout(note, emptyFile.file))
  expect(empty?.code).toBe('empty_transcript')
  expect(empty?.tool).toBe('claude-code')
  const cutOff = detail.notes.filter((note) =>
    noteIsAbout(note, malformed.file),
  )
  expect(cutOff.map((note) => note.code)).toContain('malformed_record')
  expect(cutOff.every((note) => note.tool === 'codex')).toBe(true)
  expect(detail.formatNotices ?? []).toEqual([])
  // Nothing asks for a look.
  expect(
    (await agentTasks('attention')).filter((t) => t.runRef === run.runRef),
  ).toEqual([])
  const task = (await agentTasks('today')).find((t) => t.runRef === run.runRef)
  expect(task?.state).toBe('succeeded')
  expect(task?.attention).toBe(false)
  await expect(
    dialog.getByText(
      `Everything was saved, with ${notesText(notes)} about reading it; the backup page lists ${notes === 1 ? 'it' : 'them'}.`,
    ),
  ).toBeVisible()
  await expect(
    dialog.getByText(/Not everything was saved|Until it is saved in full/),
  ).toHaveCount(0)
  // Only the two tools were read, and `.git` was not.
  expect(detail.readGit).toBe(false)
  expect((detail.perTool ?? []).map((tool) => tool.tool).sort()).toEqual(
    [...CHOSEN].sort(),
  )
  await testInfo.attach('Done', {
    body: await dialog.screenshot(),
    contentType: 'image/png',
  })

  // The space opens at Insights (`data-spaces.md` §3.2); History lists what
  // the first save saved; its backup page shows the same run.
  await dialog.getByRole('button', { name: 'View conversations' }).click()
  await expect(page).toHaveURL(/#\/agent\/insights$/)
  await page.getByRole('link', { name: 'History', exact: true }).click()
  await expect(page).toHaveURL(/#\/agent\/chats$/)
  await expect(
    page.getByRole('heading', { name: 'History', level: 1 }),
  ).toBeVisible()
  await expect(chatList(page).locator('[data-chat-id]').first()).toBeVisible()
  await page.getByRole('link', { name: 'Backup', exact: true }).click()
  await expect(page).toHaveURL(/#\/agent\/backup$/)
  await expect(
    page.getByRole('heading', { name: 'Backup', level: 1 }),
  ).toBeVisible()
  await expect(
    page.getByText(
      `Agent conversations · ${conversationsText(held.conversations)}.`,
    ),
  ).toBeVisible()
  const runs = page.getByRole('button', { name: /^Recent backups/ })
  await runs.click()
  // Health checks and clean-ups of the whole archive are listed too.
  const row = page.locator('li[data-run-ref^="agent-history:"]')
  await expect(row).toHaveCount(1)
  await expect(row).toHaveAttribute('data-run-ref', run.runRef)
  await expect(row).toContainText(`${notesText(notes)} · ${notes} new`)
  await expect(
    page.getByRole('button', {
      name: new RegExp(`^Last backup: .* · ${notesText(notes)}$`),
    }),
  ).toBeVisible()
  await row.getByRole('button', { name: /^Details of the backup at / }).click()
  const sheet = page.getByRole('dialog')
  await expect(sheet.getByText(`Done · ${notesText(notes)}`)).toBeVisible()
  const notSaved = sheet.getByRole('region', { name: 'Not saved' })
  await expect(notSaved).toContainText('Everything it read was saved.')
  const listed = sheet.getByRole('region', { name: 'Notes' })
  await expect(listed).toContainText(
    'These files are saved as they are. The notes say what PathKeep couldn’t make sense of while reading them; nothing is missing because of them.',
  )
  // The sheet shows the first page of notes until "Show more".
  const firstPage = (await agentRun(run.runRef)).notes ?? []
  await expect(listed.getByRole('listitem')).toHaveCount(firstPage.length)
  await expect(
    listed.locator('li[data-code="empty_transcript"]').first(),
  ).toContainText('Claude Code: An empty file: it holds no conversation yet.')
  await expect(
    listed.locator('li[data-code="malformed_record"]').first(),
  ).toContainText(
    'Codex: Part of it couldn’t be read, such as a damaged or cut-off line; the rest was read.',
  )
  await expect(sheet).toContainText(
    'First save, when Agent conversations were turned on',
  )
  await expect(sheet.locator('li[data-tool]')).toHaveCount(CHOSEN.length)
  await expect(sheet).toContainText(/Read \.git\s*No/)
  await expect(page).toHaveURL(/\?run=agent-history/)
  await testInfo.attach('the run’s details', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
  await page.keyboard.press('Escape')
  await expect(page).not.toHaveURL(/\?run=/)

  // Each tool's row counts its notes, and unfolded names them by kind.
  const sources = await agentSources()
  const rowNotes = {
    'claude-code': sourceNotes(sources, 'claude-code'),
    codex: sourceNotes(sources, 'codex'),
  }
  expect(rowNotes['claude-code'].codes).toContain('empty_transcript')
  expect(rowNotes.codex.codes).toContain('malformed_record')
  expect(rowNotes['claude-code'].count + rowNotes.codex.count).toBe(notes)
  for (const tool of CHOSEN) {
    const mine = rowNotes[tool]
    await expect(sourceRow(page, tool)).toContainText(notesText(mine.count))
    await sourceRow(page, tool)
      .getByRole('button', { name: `${TOOL_NAMES[tool]}: see what is kept` })
      .click()
    const kept = sourceRow(page, tool).getByRole('region', {
      name: 'Notes from its last backup',
    })
    await expect(kept).toContainText('These files are saved as they are.')
    await expect(kept.locator('li[data-note-code]')).toHaveCount(
      mine.codes.length,
    )
  }
  await expect(
    sourceRow(page, 'claude-code').locator(
      'li[data-note-code="empty_transcript"]',
    ),
  ).toContainText('An empty file: it holds no conversation yet.')
  await expect(
    sourceRow(page, 'codex').locator('li[data-note-code="malformed_record"]'),
  ).toContainText('Part of it couldn’t be read')
  // Notes are no problem: neither row asks for a look.
  for (const tool of CHOSEN)
    await expect(sourceRow(page, tool)).not.toContainText('Needs a look')
  await testInfo.attach('notes on the source rows', {
    body: await page.getByRole('region', { name: 'Sources' }).screenshot(),
    contentType: 'image/png',
  })

  // The tools left unticked were not read.
  for (const tool of ['pi', 'deepseek'] as const)
    await expect(sourceRow(page, tool)).toContainText('Not backed up')
  for (const tool of CHOSEN)
    await expect(sourceRow(page, tool)).not.toContainText('Not backed up')
  const overview = await agentOverview()
  for (const tool of ['pi', 'deepseek'] as const) {
    const totals = overview.perTool.find((item) => item.tool === tool)
    expect(count(totals?.roots), `${tool} holds nothing`).toBe(0)
  }
  expect(count(overview.roots)).toBe(held.conversations)
  await testInfo.attach('the backup page after the first save', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })
})
