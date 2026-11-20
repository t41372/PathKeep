/**
 * First run with Agent conversations chosen (`data-spaces.md` §3.3), on an
 * empty data folder, against the synthetic browsers and Agent tool folders
 * as the projects before this one left them.
 *
 * Proves:
 * - the welcome chart has Claude Code's and Codex's rows;
 * - "What to keep" starts with both kinds ticked, since both have sources
 *   here, and names the tools found; unticking both says one is needed and
 *   holds Continue back;
 * - "Sources" has a pane per kind, ↓ moves to Agent conversations, whose
 *   tools are listed with the conversation files, subagent files and size
 *   the walk counts from the folders (what the fixture wrote); the .git
 *   note is there, and Claude Code's auto-delete note only while it is
 *   ticked; with no Agent tool ticked, the step names the kind and holds
 *   Continue back;
 * - with Pi and DeepSeek Harness unticked, "Storage" has a row per kind and
 *   a total, and the Agent row is what the backend estimates for Claude
 *   Code and Codex alone;
 * - the encryption and schedule steps say they cover every kind, and "Don't
 *   encrypt" warns about what Agent conversations hold;
 * - "Done" sums up the kinds, the browsers and the two tools with the .git
 *   line; the first backup shows a line per kind while it runs, and ends
 *   with each kind's result in its own unit: the browsing run's new visits
 *   and the conversations the first save saved, which are what the two
 *   tools hold;
 * - the first save read Claude Code and Codex and `.git`, never Pi or
 *   DeepSeek Harness; both kinds are on, and the Agent space lists the
 *   conversations.
 *
 * Runs after `empty`, which leaves the data folder empty, and deletes all
 * data at the end the way a user does, so `upgrade` finds it empty again.
 */
import { expect, test, type Page } from '@playwright/test'
import type { AgentTool } from '../../scripts/fixtures/synthetic-agents.mjs'
import { unlockIfLocked } from './support/app'
import { chatList } from './support/agent-chats'
import {
  agentOverview,
  agentRunWhole,
  agentSources,
  count,
  dataKinds,
  heldBy,
  waitForRunAfter,
} from './support/agent-backup'
import {
  TOOL_NAMES,
  expectedFolders,
  expectedTool,
  expectedToolNames,
  formatBytes,
  previewFrames,
} from './support/agent-tools'
import { archivedProfiles, backend, fixture } from './support/fixture'

const CHOSEN: AgentTool[] = ['claude-code', 'codex']
const LEFT_OUT: AgentTool[] = ['pi', 'deepseek']
const WELCOME = 'Keep what gets deleted automatically'

const list = (items: string[]) =>
  new Intl.ListFormat('en-US', { type: 'conjunction' }).format(items)
const plural = (n: number, word: string) =>
  `${n.toLocaleString('en-US')} ${word}${n === 1 ? '' : 's'}`

/** A row of the Done step's summary, by its label. */
const summaryValue = (page: Page, label: string) =>
  page
    .locator('dl > div')
    .filter({ has: page.locator('dt').getByText(label, { exact: true }) })
    .locator('dd')

const toolTick = (page: Page, tool: AgentTool) =>
  page.getByRole('checkbox', { name: TOOL_NAMES[tool], exact: true })

test('onboarding with Agent conversations: two tools of four, then a first backup per kind', async ({
  page,
}, testInfo) => {
  test.setTimeout(300_000)
  const { agents } = fixture()
  const found = expectedToolNames()
  const continueButton = page.getByRole('button', { name: 'Continue' })

  await page.goto('/')
  await unlockIfLocked(page)
  await expect(page.getByRole('heading', { name: WELCOME })).toBeVisible()
  const chart = page.getByRole('listitem')
  await expect(chart.filter({ hasText: 'Claude Code' })).toContainText(
    '30 days by default',
  )
  await expect(chart.filter({ hasText: 'Codex' })).toContainText(
    'Never by itself, but often cleared by hand',
  )
  await page.getByRole('button', { name: 'Get started' }).click()

  // What to keep: both kinds have sources here, so both start ticked.
  await expect(
    page.getByRole('heading', { name: 'What should PathKeep keep?' }),
  ).toBeVisible()
  const browsingKind = page.getByRole('checkbox', {
    name: 'Browsing history',
    exact: true,
  })
  const agentKind = page.getByRole('checkbox', {
    name: 'Agent conversations',
    exact: true,
  })
  await expect(browsingKind).toBeChecked()
  await expect(agentKind).toBeChecked()
  await expect(agentKind).toContainText(`Found here: ${list(found)}`)
  await browsingKind.click()
  await agentKind.click()
  await expect(page.getByText('Choose at least one.')).toBeVisible()
  await expect(continueButton).toBeDisabled()
  await browsingKind.click()
  await agentKind.click()
  await expect(agentKind).toBeChecked()
  await testInfo.attach('What to keep', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
  await continueButton.click()

  // Sources: a pane per kind; browsing's is open first.
  await expect(
    page.getByRole('heading', { name: 'Here’s what PathKeep found' }),
  ).toBeVisible()
  const kinds = page.getByRole('tablist', { name: 'Kinds of data' })
  const browsingTab = kinds.getByRole('tab', { name: /^Browsing history/ })
  const agentTab = kinds.getByRole('tab', { name: /^Agent conversations/ })
  await expect(browsingTab).toHaveAttribute('aria-selected', 'true')
  await expect(browsingTab).toContainText(
    `${archivedProfiles.length} profiles, all backed up`,
  )
  await expect(agentTab).toContainText(
    'Also reads the .git of each project folder',
  )
  await browsingTab.focus()
  await page.keyboard.press('ArrowDown')
  await expect(agentTab).toHaveAttribute('aria-selected', 'true')
  await expect(agentTab).toBeFocused()

  // Each tool found, with what the walk counted, all ticked to begin with.
  const tools = page.getByRole('list', {
    name: 'Agent tools found on this computer',
  })
  await expect(tools.getByRole('listitem')).toHaveCount(found.length, {
    timeout: 60_000,
  })
  await expect(agentTab).toContainText(`${found.length} tools, all backed up`)
  for (const tool of CHOSEN) {
    const facts = expectedTool(tool, agents.unreadable)
    const row = tools.locator(`li[data-tool="${tool}"]`)
    await expect(toolTick(page, tool)).toBeChecked()
    await expect(row).toContainText(
      plural(facts.conversationFiles, 'conversation file'),
    )
    await expect(row).toContainText(formatBytes(facts.bytes))
    for (const folder of expectedFolders().filter((f) => f.tool === tool))
      await expect(row).toContainText(folder.path)
  }
  await expect(
    page.getByText(
      /^Backups also read the \.git of each project folder, but only the repo’s root folder/,
    ),
  ).toBeVisible()
  const claude = expectedTool('claude-code', agents.unreadable)
  const autoDelete = page.getByText(
    `Claude Code deletes conversations after 30 days by default (you can change that in its own settings). The first backup saves the ${plural(claude.conversationFiles, 'conversation file')} still there now.`,
  )
  await expect(autoDelete).toBeVisible()

  // No tool ticked: the kind is named and Continue waits.
  for (const tool of [...CHOSEN, ...LEFT_OUT].filter((tool) =>
    found.includes(TOOL_NAMES[tool]),
  ))
    await toolTick(page, tool).setChecked(false)
  await expect(autoDelete).toHaveCount(0)
  await expect(agentTab).toContainText(`${found.length} tools, none chosen`)
  const noSource = page.getByText(
    '“Agent conversations” has no source ticked that PathKeep can read. Tick one, or go back and untick this kind.',
  )
  await expect(noSource).toBeVisible()
  await expect(continueButton).toBeDisabled()
  for (const tool of CHOSEN) await toolTick(page, tool).setChecked(true)
  await expect(noSource).toHaveCount(0)
  await expect(autoDelete).toBeVisible()
  await expect(agentTab).toContainText(`${found.length} tools, 2 chosen`)
  await testInfo.attach('Sources, Agent conversations', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
  await continueButton.click()

  // Storage: a row per kind and a total; Agent's is the backend's estimate
  // for the two ticked tools alone.
  await expect(
    page.getByRole('heading', { name: 'Where the archive lives' }),
  ).toBeVisible()
  const frames = await previewFrames({ purpose: 'onboarding', tools: CHOSEN })
  const report = frames.find((frame) => frame.type === 'finished')
    ?.payload as unknown as {
    totals: { sourceBytes: string }
    estimatedArchiveBytes: string
  }
  expect(report, 'the walk of the two tools finished').toBeTruthy()
  await testInfo.attach('the backend’s estimate for the two tools', {
    body: JSON.stringify(report, null, 2),
    contentType: 'application/json',
  })
  const table = page.getByRole('table', { name: 'Size by kind of data' })
  const agentRow = table
    .locator('tbody tr')
    .filter({ hasText: 'Agent conversations' })
    .getByRole('cell')
  await expect(agentRow).toHaveText(
    [
      formatBytes(Number(report.totals.sourceBytes)),
      `≈ ${formatBytes(Number(report.estimatedArchiveBytes))}`,
      String(CHOSEN.length),
    ],
    { timeout: 60_000 },
  )
  await expect(
    table
      .locator('tbody tr')
      .filter({ hasText: 'Browsing history' })
      .getByRole('cell')
      .last(),
  ).toHaveText(String(archivedProfiles.length))
  await expect(
    table
      .locator('tbody tr')
      .filter({ hasText: 'Total' })
      .getByRole('cell')
      .last(),
  ).toHaveText(String(archivedProfiles.length + CHOSEN.length))
  await testInfo.attach('Storage', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
  await continueButton.click()

  await expect(
    page.getByRole('heading', { name: 'Encrypt the archive?' }),
  ).toBeVisible()
  await expect(
    page.getByText(
      'Every kind of data is kept in the same archive, so this choice applies to all of them.',
    ),
  ).toBeVisible()
  const plaintext = page.getByRole('radio', { name: /^Don’t encrypt/ })
  await expect(plaintext).toContainText(
    'Agent conversations often hold code, keys and private paths, so think it over.',
  )
  await plaintext.click()
  await continueButton.click()

  // No schedule: installing one would talk to the OS scheduler.
  await expect(
    page.getByRole('heading', { name: 'How often should we back up?' }),
  ).toBeVisible()
  await expect(
    page.getByText('One schedule backs up every kind of data together.'),
  ).toBeVisible()
  await page.getByRole('radio', { name: /^Manual/ }).click()
  await continueButton.click()
  await expect(
    page.getByRole('heading', { name: 'Turn on AI features?' }),
  ).toBeVisible()
  await page.getByRole('radio', { name: /^Not now/ }).click()
  await continueButton.click()

  // Done: the summary names the kinds, the browsers and the two tools.
  await expect(
    page.getByRole('heading', { name: 'You’re all set' }),
  ).toBeVisible()
  await expect(summaryValue(page, 'Keep')).toHaveText(
    'Browsing history and Agent conversations',
  )
  await expect(summaryValue(page, 'Browsers')).toHaveText(
    new RegExp(`^${archivedProfiles.length} profiles: `),
  )
  await expect(summaryValue(page, 'Agent tools')).toHaveText(
    `2 tools: ${list(CHOSEN.map((tool) => TOOL_NAMES[tool]))}; also reads the .git of each project folder`,
  )
  await expect(summaryValue(page, 'Online')).toBeVisible()

  // The first backup: a line per kind while it runs (the save is slowed per
  // file, so it is seen running).
  await page.getByRole('button', { name: 'Start first backup' }).click()
  const agentLine = page.locator('li[data-kind="agent-history"]')
  await expect(page.locator('li[data-kind="browsing"]')).toBeVisible({
    timeout: 60_000,
  })
  await expect(agentLine).toContainText('Agent conversations ·')
  await expect(
    agentLine.getByRole('progressbar', {
      name: 'Backup progress: Agent conversations',
    }),
  ).toBeVisible()
  await testInfo.attach('the first backup of each kind running', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  // Each kind's result in its own unit; nothing is added together.
  const result = page.getByRole('status').filter({ hasText: 'Done.' })
  await expect(result).toBeVisible({ timeout: 180_000 })
  const run = await waitForRunAfter(undefined)
  const detail = await agentRunWhole(run.runRef)
  const held = heldBy(agents, CHOSEN)
  const saved =
    count(detail.summary.newRoots) + count(detail.summary.updatedRoots)
  expect(detail.warnings).toEqual([])
  expect(run.result).toBe('success')
  expect(run.trigger).toBe('enable')
  expect(saved).toBe(held.conversations)
  const snapshot = await backend<{ recentRuns: { newVisits: number }[] }>(
    'app_snapshot',
  )
  const visits = snapshot.recentRuns[0]?.newVisits ?? 0
  expect(visits).toBeGreaterThan(0)
  await expect(result.locator('li[data-kind="browsing"]')).toHaveText(
    `Browsing history: ${plural(visits, 'new visit')}`,
  )
  await expect(result.locator('li[data-kind="agent-history"]')).toHaveText(
    `Agent conversations: ${plural(held.conversations, 'conversation')} saved`,
  )
  await testInfo.attach('the result of each kind', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  // Only the two ticked tools were read, with `.git`; Pi and DeepSeek
  // Harness were never read and hold nothing in the archive.
  expect(detail.readGit).toBe(true)
  expect((detail.perTool ?? []).map((tool) => tool.tool).sort()).toEqual(
    [...CHOSEN].sort(),
  )
  const overview = await agentOverview()
  for (const tool of LEFT_OUT) {
    const holds = overview.perTool.find((row) => row.tool === tool)
    expect(count(holds?.roots), `${tool} holds nothing`).toBe(0)
    for (const source of (await agentSources()).filter(
      (row) => row.tool === tool,
    ))
      expect(source.enabled, `${tool} is not backed up`).toBe(false)
  }
  expect(count(overview.roots)).toBe(held.conversations)
  const states = await dataKinds()
  expect(
    Object.fromEntries(states.kinds.map((row) => [row.kind, row.state])),
  ).toEqual({ browsing: 'on', 'agent-history': 'on' })

  // The app opens; the Agent space lists the conversations.
  await page.getByRole('button', { name: 'Open PathKeep' }).click()
  await expect(
    page.getByRole('heading', { name: 'You’re all set' }),
  ).toBeHidden()
  await page.goto('/#/agent/chats')
  await unlockIfLocked(page)
  await expect(
    page.getByRole('heading', { name: 'History', level: 1 }),
  ).toBeVisible()
  await expect(chatList(page).locator('[data-chat-id]').first()).toBeVisible({
    timeout: 30_000,
  })

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
