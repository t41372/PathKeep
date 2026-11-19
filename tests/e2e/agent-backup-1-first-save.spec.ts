/**
 * From the Home card to a finished first save of Agent conversations
 * (`data-spaces.md` §3, `agent-history.md` §6), against the synthetic tool
 * folders (`CHB_AGENT_TOOLS_HOME`). The capture is slowed per file
 * (`PATHKEEP_TEST_AGENT_CAPTURE_FILE_DELAY_US`) so its progress can be seen.
 *
 * Proves:
 * - ticking Claude Code and Codex with `.git` off and starting turns the
 *   kind on at once: the switcher lists Agent conversations while the save
 *   still runs;
 * - the Run step shows each ticked tool's progress in files (found, then
 *   saved with their size), never a percentage, and no row for the others;
 * - Done's numbers are the run's (`get_archive_run`), and those are what
 *   the fixture says the two tools hold: every conversation and subagent;
 * - "View conversations" opens the space at its backup page, whose header
 *   counts the same conversations and whose recent backups list the run;
 *   its details name only the two tools and say `.git` was not read;
 * - Pi and DeepSeek Harness were not read: their rows say not backed up,
 *   and the archive holds nothing of them.
 */
import { expect, test } from '@playwright/test'
import { unlockIfLocked } from './support/app'
import { enableDialog, TOOL_NAMES } from './support/agent-tools'
import {
  agentOverview,
  agentRun,
  chooseAndStart,
  conversationsText,
  count,
  heldBy,
  sourceRow,
  subagentsText,
  waitForRunAfter,
} from './support/agent-backup'
import { fixture } from './support/fixture'

const CHOSEN = ['claude-code', 'codex'] as const

test('a first save from the Home card, with two tools and no .git', async ({
  page,
}, testInfo) => {
  await page.goto('/#/browsing/home')
  await unlockIfLocked(page)
  await page
    .getByRole('region', {
      name: 'PathKeep can now keep your Agent conversations too',
    })
    .getByRole('button', { name: 'Preview what would be saved' })
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
  expect(['success', 'partial']).toContain(run.result)
  const detail = await agentRun(run.runRef)
  const held = heldBy(fixture().agents, CHOSEN)
  const conversations =
    count(detail.summary.newRoots) + count(detail.summary.updatedRoots)
  expect(conversations).toBe(held.conversations)
  expect(count(detail.summary.newChildren)).toBe(held.subagents)
  await expect(dialog).toContainText(
    `Saved ${conversationsText(held.conversations)} and ${subagentsText(held.subagents)}`,
  )
  if (run.result === 'partial')
    await expect(
      dialog.getByText(
        'Not everything could be read. What could be read is saved:',
      ),
    ).toBeVisible()
  // Only the two tools were read, and `.git` was not.
  expect(detail.readGit).toBe(false)
  expect((detail.perTool ?? []).map((tool) => tool.tool).sort()).toEqual(
    [...CHOSEN].sort(),
  )
  await testInfo.attach('Done', {
    body: await dialog.screenshot(),
    contentType: 'image/png',
  })

  // The space opens at its backup page, which shows the same run.
  await dialog.getByRole('button', { name: 'View conversations' }).click()
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
  await row.getByRole('button', { name: /^Details of the backup at / }).click()
  const sheet = page.getByRole('dialog')
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
