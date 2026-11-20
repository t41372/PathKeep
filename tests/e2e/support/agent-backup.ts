/**
 * Backing up Agent conversations, for the agent-backup specs: the backup
 * page, the rail's task list, the background tasks page, and what the
 * backend says about the same runs (`list_archive_runs`, `get_archive_run`
 * with every page of its warnings and notes, `get_agent_overview`,
 * `list_agent_sources`), to check the screens against.
 *
 * Expected numbers come from the fixture's facts (`fixture().agents`):
 * what a tool holds is its conversations and their subagents as the
 * generator wrote them, after whatever the mutators did.
 */
import { mkdirSync } from 'node:fs'
import { expect, type Page } from '@playwright/test'
import type {
  AgentTool,
  SyntheticAgents,
} from '../../../scripts/fixtures/synthetic-agents.mjs'
import { unlockIfLocked } from './app'
import {
  TOOL_NAMES,
  enableDialog,
  toolTick,
  waitForPreview,
} from './agent-tools'
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
  /** Things the run could not save. */
  warningCount: string
  /** Conditions of files it saved; never a reason for attention. */
  noteCount?: string | null
  newNoteCount?: string | null
  /** Format versions met first by this run and not dismissed yet. */
  formatNotices?: WireFormatNotice[]
}

export interface WireFormatNotice {
  tool: AgentTool
  version: string
}

export interface WireNote {
  code: string
  path: string | null
  detail: string | null
  rootId: string
  tool: AgentTool | null
  isNew: boolean
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
  notes?: WireNote[]
  nextCursor?: string
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
  actions: string[]
  formatNotices?: WireFormatNotice[]
  reason?: string
}

export interface WireSource {
  rootId: string | null
  tool: AgentTool
  path: string | null
  enabled: boolean
  availability: string
  /** Which folder at the path was approved; absent for a tool without one. */
  directoryToken?: string
  noteCount?: string
  newNoteCount?: string
  notes?: { code: string; count: string; newCount: string }[]
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

/** A run with every page of its warnings and notes (50 a page). */
export async function agentRunWhole(runRef: string) {
  const first = await agentRun(runRef)
  const warnings = [...(first.warnings ?? [])]
  const notes = [...(first.notes ?? [])]
  let cursor = first.nextCursor
  while (cursor) {
    const next = await backend<WireRunDetail>('get_archive_run', {
      request: { runRef, detailCursor: cursor },
    })
    warnings.push(...(next.warnings ?? []))
    notes.push(...(next.notes ?? []))
    cursor = next.nextCursor
  }
  return { ...first, warnings, notes }
}

/** Every source row of the backup page (a tool's folders, or the tool). */
export async function agentSources() {
  const page = await backend<{ items: WireSource[] }>('list_agent_sources', {
    request: { limit: 100 },
  })
  return page.items
}

/** The notes a tool's folders carry from their last backup, added up. */
export function sourceNotes(sources: readonly WireSource[], tool: AgentTool) {
  const mine = sources.filter((source) => source.tool === tool)
  return {
    count: mine.reduce((sum, source) => sum + count(source.noteCount), 0),
    fresh: mine.reduce((sum, source) => sum + count(source.newNoteCount), 0),
    codes: [
      ...new Set(mine.flatMap((s) => (s.notes ?? []).map((n) => n.code))),
    ],
  }
}

/**
 * Whether a note is about `file`: the backend names a file by its path
 * under the tool's folder, the fixture by its full path.
 */
export function noteIsAbout(note: WireNote, file: string) {
  if (!note.path) return false
  const normal = (value: string) => value.replaceAll('\\', '/')
  return normal(file).endsWith(`/${normal(note.path).replace(/^\//, '')}`)
}

/** "3 notes", the way the app counts them in English. */
export function notesText(n: number) {
  return `${n.toLocaleString('en-US')} note${n === 1 ? '' : 's'}`
}

export function agentOverview() {
  return backend<WireOverview>('get_agent_overview', {
    request: { purpose: 'backup' },
  })
}

export async function agentTasks(
  group: 'running' | 'waiting' | 'attention' | 'today',
) {
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

export async function openBackupPage(page: Page) {
  await page.goto('/#/agent/backup')
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

/** The backend's summary for the rail (`get_task_summary`). */
export function taskSummary() {
  return backend<{
    runningCount: number
    queuedCount: number
    attentionCount: string
    running: { taskId: string; kind: string }[]
    unavailableKinds?: string[]
  }>('get_task_summary', { request: {} })
}

/**
 * What the rail's indicator counts as needing a look, as the backend
 * counts it: conversation backups and format notices, and failed or
 * partial browsing and whole-archive work nobody dismissed.
 */
export async function railAttention() {
  return Number((await taskSummary()).attentionCount)
}

/** The indicator's name with nothing running and `n` needing a look. */
export function idleIndicatorName(n: number) {
  return n === 0
    ? 'No background tasks running'
    : `Background tasks: ${n} need${n === 1 ? 's' : ''} a look`
}

/**
 * Waits until the indicator is idle, naming what needs a look as the
 * backend counts it by then: work that just ended (the Insights jobs a
 * browsing backup queues, say) may still be running a moment.
 */
export async function expectIdleIndicator(page: Page, timeout = 60_000) {
  await expect(async () => {
    await expect(taskIndicator(page)).toHaveAccessibleName(
      idleIndicatorName(await railAttention()),
      { timeout: 1_000 },
    )
  }).toPass({ timeout })
}

/**
 * The list's line that leads to the background tasks page when `n` things
 * need a look (`railAttention`: every kind, counted together).
 */
export function needsLookText(n: number) {
  return `${n} need${n === 1 ? 's' : ''} a look`
}

/**
 * A row of one group on the background tasks page (`#/tasks`), reading on
 * with "Show more" until it is there: a group lists 20 rows at first, and
 * a day of backups and queue jobs can hold more.
 */
export async function taskRow(page: Page, group: string, selector: string) {
  const section = page.locator(`section[data-task-group="${group}"]`)
  const row = section.locator(selector)
  await expect(async () => {
    if ((await row.count()) === 0) {
      const more = section.getByRole('button', { name: 'Show more' })
      if (await more.isVisible()) await more.click()
    }
    expect(await row.count()).toBe(1)
  }).toPass({ timeout: 30_000 })
  return row
}

/** The group a browsing run is listed under on that page once it ended. */
export function browsingRunGroup(status: string) {
  return status === 'failed' || status === 'partial' ? 'attention' : 'today'
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

export interface WireKindRow {
  kind: string
  state: string
  /** Visits or conversations, as a decimal string; absent when the store can't be opened. */
  archiveCount?: string
  hasBackupSource?: boolean
  sourceReviewRequired?: boolean
}

/**
 * Each kind's state as the backend has it, and the token to change one. A
 * read that crosses a change (the window's own, say, while a spec polls) is
 * refused with `policy-changed`; the answer is to read again, as the window
 * does when the change's event arrives.
 */
export async function dataKinds() {
  for (let attempt = 1; ; attempt++) {
    try {
      return await backend<{ kinds: WireKindRow[]; policyToken: string }>(
        'list_data_kinds',
        { request: {} },
      )
    } catch (error) {
      if (attempt >= 5 || !String(error).includes('policy-changed')) throw error
    }
  }
}

/** Agent conversations' state as the backend has it. */
export async function agentKindState() {
  return (await dataKinds()).kinds.find((row) => row.kind === 'agent-history')
}

/** Sets a kind's state through the backend, as another window would. */
export async function setKindState(kind: string, state: string) {
  const { policyToken } = await dataKinds()
  return backend('set_data_kind_state', {
    request: { kind, state, expectedPolicyToken: policyToken },
  })
}

/** Switches Agent tools on or off through the backend, as another window would. */
export async function setAgentTools(tools: readonly AgentTool[], on: boolean) {
  const { policyToken } = await backend<{ policyToken: string }>(
    'get_agent_settings',
    { request: {} },
  )
  return backend('update_agent_sources', {
    request: {
      expectedToken: policyToken,
      toolChanges: Object.fromEntries(tools.map((tool) => [tool, on])),
    },
  })
}

/** The newest browsing backup in the app's snapshot. */
export async function lastBrowsingRun() {
  const snapshot = await backend<{
    recentRuns: { id: number; status: string }[]
  }>('app_snapshot')
  return snapshot.recentRuns[0]
}

/** Waits until a browsing backup newer than `previous` has ended. */
export async function waitForBrowsingRunAfter(previous: number | undefined) {
  await expect
    .poll(
      async () => {
        const now = await lastBrowsingRun()
        return (
          now !== undefined && now.id !== previous && now.status !== 'running'
        )
      },
      { timeout: 120_000, intervals: [1_000] },
    )
    .toBe(true)
}

/**
 * "Add another folder…" on the backup page: `path` (made here, empty) for
 * `tool`, looked at, then added. Changes which folders this computer has
 * approved, and switches the tool on.
 */
export async function addAgentFolder(
  page: Page,
  tool: AgentTool,
  path: string,
) {
  mkdirSync(path, { recursive: true })
  await openBackupPage(page)
  await page.getByRole('button', { name: 'Add another folder…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Add another folder' })
  await dialog.getByLabel('Folder', { exact: true }).fill(path)
  await dialog.getByLabel('It holds conversations of').click()
  await page
    .getByRole('option', { name: TOOL_NAMES[tool], exact: true })
    .click()
  await dialog.getByRole('button', { name: 'Look at it' }).click()
  await dialog
    .getByRole('button', { name: 'Add', exact: true })
    .click({ timeout: 60_000 })
  await expect(dialog).toBeHidden()
}
