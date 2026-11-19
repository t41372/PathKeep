/**
 * Backing up Agent conversations, for the agent-backup specs: the backup
 * page, the rail's task list, Settings › Background work, and what the
 * backend says about the same runs (`list_archive_runs`, `get_archive_run`,
 * `get_agent_overview`), to check the screens against.
 *
 * Expected numbers come from the fixture's facts (`fixture().agents`):
 * what a tool holds is its conversations and their subagents as the
 * generator wrote them, after whatever the mutators did.
 */
import { expect, type Page } from '@playwright/test'
import type {
  AgentTool,
  SyntheticAgents,
} from '../../../scripts/fixtures/synthetic-agents.mjs'
import { unlockIfLocked } from './app'
import { enableDialog, toolTick, waitForPreview } from './agent-tools'
import { backend } from './fixture'

export interface WireRun {
  runRef: string
  scope: string
  operation: string
  trigger: string
  result: string
  startedAt: number | null
  finishedAt?: number
  summary: Record<string, string | null>
  warningCount: string
}

export interface WireRunDetail extends WireRun {
  readGit?: boolean
  perTool?: { tool: AgentTool; availability: string }[]
  roots?: {
    rootId: string
    tool: AgentTool
    path: string
    availability: string
  }[]
  warnings?: { code: string; tool?: AgentTool; path?: string }[]
}

interface WireOverview {
  roots: string
  children: string
  onlyInPathkeep: string
  perTool: {
    tool: AgentTool
    roots: string
    children: string
    onlyInPathkeep: string
  }[]
}

interface WireTask {
  taskId: string
  runRef: string
  trigger: string
  state: string
  attention: boolean
  reason?: string
}

/** The newest conversation runs, newest first (this space only). */
export async function agentRuns(limit = 50) {
  const page = await backend<{ items: WireRun[] }>('list_archive_runs', {
    request: { space: 'agent-history', includeArchiveWide: false, limit },
  })
  return page.items
}

export function agentRun(runRef: string) {
  return backend<WireRunDetail>('get_archive_run', { request: { runRef } })
}

export function agentOverview() {
  return backend<WireOverview>('get_agent_overview', {
    request: { purpose: 'backup' },
  })
}

export async function agentTasks(group: 'running' | 'attention' | 'today') {
  const page = await backend<{ items: WireTask[] }>('list_background_tasks', {
    request: { group, limit: 50 },
  })
  return page.items
}

export const count = (value: string | null | undefined) => Number(value ?? 0)

/** The newest conversation run's reference, or undefined before the first. */
export async function newestRunRef() {
  const [newest] = await agentRuns(1)
  return newest?.runRef
}

/**
 * Waits for a run newer than `previous` to end, and returns it. (Waiting
 * for "nothing running" alone could end before the backup started.)
 */
export async function waitForRunAfter(
  previous: string | undefined,
  timeout = 120_000,
) {
  let newest: WireRun | undefined
  await expect
    .poll(
      async () => {
        ;[newest] = await agentRuns(1)
        return (
          newest !== undefined &&
          newest.runRef !== previous &&
          newest.result !== 'running'
        )
      },
      { timeout, intervals: [500] },
    )
    .toBe(true)
  return newest!
}

/** Conversations and subagents the fixture holds now for `tools`. */
export function heldBy(agents: SyntheticAgents, tools: readonly AgentTool[]) {
  const mine = agents.conversations.filter((c) => tools.includes(c.tool))
  return {
    conversations: mine.length,
    subagents: mine.reduce((sum, c) => sum + c.subagents.length, 0),
  }
}

/** "12 conversations", the way the app counts them in English. */
export function conversationsText(n: number) {
  return `${n.toLocaleString('en-US')} conversation${n === 1 ? '' : 's'}`
}

export function subagentsText(n: number) {
  return `${n.toLocaleString('en-US')} subagent${n === 1 ? '' : 's'}`
}

export async function openBackupPage(page: Page, query = '') {
  await page.goto(`/#/agent/backup${query}`)
  await unlockIfLocked(page)
  await expect(
    page.getByRole('heading', { name: 'Backup', level: 1 }),
  ).toBeVisible()
}

/** The sources card's row for a tool. */
export const sourceRow = (page: Page, tool: AgentTool) =>
  page
    .getByRole('region', { name: 'Sources' })
    .locator(`li[data-tool="${tool}"]`)

/** The rail's background-task indicator. */
export const taskIndicator = (page: Page) =>
  page.getByRole('button', {
    name: /^(No background tasks running|\d+ background tasks? running|Background tasks: \d+ needs? a look)$/,
  })

/**
 * What the rail's indicator counts as needing a look: conversation backups
 * that need a look, and the last browsing backup if it failed.
 */
export async function railAttention() {
  const tasks = await agentTasks('attention')
  const snapshot = await backend<{ recentRuns: { status: string }[] }>(
    'app_snapshot',
  )
  return tasks.length + (snapshot.recentRuns[0]?.status === 'failed' ? 1 : 0)
}

/** The indicator's name with nothing running and `n` needing a look. */
export function idleIndicatorName(n: number) {
  return n === 0
    ? 'No background tasks running'
    : `Background tasks: ${n} need${n === 1 ? 's' : ''} a look`
}

/** The list's line that leads to conversation backups needing a look. */
export function needsLookText(n: number) {
  return `${n} conversation backup${n === 1 ? ' needs' : 's need'} a look`
}

/** Hovers the indicator and waits for its list. */
export async function openTaskList(page: Page) {
  await taskIndicator(page).hover()
  const list = page.getByRole('dialog').filter({
    has: page.getByText('Background tasks', { exact: true }),
  })
  await expect(list).toBeVisible()
  return list
}

/**
 * The choose step with exactly `tools` ticked and `.git` set, whatever an
 * earlier spec left stored, then "Start saving".
 */
export async function chooseAndStart(
  page: Page,
  tools: readonly AgentTool[],
  readGit: boolean,
) {
  const dialog = enableDialog(page)
  await waitForPreview(page)
  await dialog.getByRole('button', { name: 'Next' }).click()
  for (const tool of ['claude-code', 'codex', 'pi', 'deepseek'] as const)
    await toolTick(page, tool).setChecked(tools.includes(tool))
  await dialog
    .getByRole('switch', { name: 'Note each project’s git details' })
    .setChecked(readGit)
  await dialog.getByRole('button', { name: 'Start saving' }).click()
  await expect(
    dialog
      .getByRole('heading', { name: 'Saving' })
      .or(dialog.getByRole('heading', { name: 'Saved' })),
  ).toBeVisible()
}

/** Agent conversations' state as the backend has it. */
export async function agentKindState() {
  const list = await backend<{
    kinds: { kind: string; state: string; hasBackupSource?: boolean }[]
  }>('list_data_kinds', { request: {} })
  return list.kinds.find((row) => row.kind === 'agent-history')
}
