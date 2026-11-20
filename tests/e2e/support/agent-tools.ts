/**
 * The synthetic Agent tools (`scripts/fixtures/synthetic-agents.mjs`) as the
 * enable dialog and the intro card should show them, and the dialog itself.
 *
 * The fake home holds five folders: one per tool, plus the Claude desktop
 * app's (`Claude`), which holds Claude Code conversations too. The preview
 * has a row per folder; the choose step adds a tool's folders up.
 *
 * What the preview should count for a folder, from the files the generator
 * wrote, without opening any of them. The preview walks the folder with the
 * same file rule the save uses, so it counts what the save would read:
 * - conversation files: files named like a transcript. That is the
 *   conversations (`conversation`) and also the empty transcript (`empty`):
 *   only opening it would show that it holds no conversation. A Cowork
 *   task's `audit.jsonl` (`audit`) counts only while its task folder holds
 *   no main transcript; next to one, the save keeps it as the task's
 *   metadata, not as a conversation of its own;
 * - subagent files: `subagent`, which Claude Code's layout shows;
 * - size: every file in the folder;
 * - earliest: the oldest file's modification time.
 */
import { existsSync, renameSync } from 'node:fs'
import path from 'node:path'
import { expect, type Page } from '@playwright/test'
import type {
  AgentFile,
  AgentRoot,
  AgentTool,
} from '../../../scripts/fixtures/synthetic-agents.mjs'
import { resolveDesktopBridgeEnv } from '../../../scripts/pathkeep-dev-desktop-bridge.mjs'
import { unlockIfLocked } from './app'
import { backend, fixture } from './fixture'

export const TOOL_ORDER: AgentTool[] = [
  'claude-code',
  'codex',
  'pi',
  'deepseek',
]

export const TOOL_NAMES: Record<AgentTool, string> = {
  'claude-code': 'Claude Code',
  codex: 'Codex',
  pi: 'Pi',
  deepseek: 'DeepSeek Harness',
}

/** The folders in the order the preview lists them: by tool, then as the backend walks. */
const ROOT_ORDER: AgentRoot[] = [
  'claude-code',
  'claude-desktop',
  'codex',
  'pi',
  'deepseek',
]

const ROOT_TOOL: Record<AgentRoot, AgentTool> = {
  'claude-code': 'claude-code',
  'claude-desktop': 'claude-code',
  codex: 'codex',
  pi: 'pi',
  deepseek: 'deepseek',
}

/** Whether the preview counts `file` as a conversation file (see above). */
function looksLikeConversation(file: AgentFile, files: readonly AgentFile[]) {
  if (file.kind !== 'audit')
    return file.kind === 'conversation' || file.kind === 'empty'
  const transcripts = path.join(path.dirname(file.path), '.claude', 'projects')
  return !files.some(
    (other) =>
      other.kind === 'conversation' &&
      other.path.startsWith(`${transcripts}${path.sep}`),
  )
}

/** The folders whose rows name them, as English shows it. */
const FOLDER_LABELS: Partial<Record<AgentRoot, string>> = {
  'claude-desktop': 'Claude desktop app (local Cowork sessions)',
}

export interface ExpectedFolder {
  root: AgentRoot
  tool: AgentTool
  name: string
  /** What the folder is, over its path, when the path does not say it. */
  label?: string
  path: string
  conversationFiles: number
  subagentFiles: number
  bytes: number
  earliest: number
}

/** The folders that exist in the fake home, in the preview's order. */
export function expectedFolders(): ExpectedFolder[] {
  const { agents } = fixture()
  return ROOT_ORDER.filter((root) => existsSync(agents.roots[root])).map(
    (root) => {
      const facts = agents.tools[root]
      const tool = ROOT_TOOL[root]
      return {
        root,
        tool,
        name: TOOL_NAMES[tool],
        label: FOLDER_LABELS[root],
        path: agents.roots[root],
        conversationFiles: facts.files.filter((file) =>
          looksLikeConversation(file, facts.files),
        ).length,
        subagentFiles: facts.subagentFiles.length,
        bytes: facts.totalBytes,
        earliest: facts.earliestFileDate,
      }
    },
  )
}

/** A tool's folders added up, as the choose step shows them; only folders it can read count. */
export function expectedTool(
  tool: AgentTool,
  unreadable: readonly AgentRoot[] = [],
) {
  const folders = expectedFolders().filter(
    (folder) => folder.tool === tool && !unreadable.includes(folder.root),
  )
  return {
    conversationFiles: folders.reduce((sum, f) => sum + f.conversationFiles, 0),
    bytes: folders.reduce((sum, f) => sum + f.bytes, 0),
  }
}

/** The tools with at least one folder in the fake home, by name, in the app's order. */
export function expectedToolNames() {
  const tools = new Set(expectedFolders().map((folder) => folder.tool))
  return TOOL_ORDER.filter((tool) => tools.has(tool)).map(
    (tool) => TOOL_NAMES[tool],
  )
}

/** A size the way the app writes it in English (`useFormat().bytes`). */
export function formatBytes(value: number) {
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let size = value
  let unit = 0
  while (size >= 1024 && unit < units.length - 1) {
    size /= 1024
    unit += 1
  }
  const digits = size >= 100 || unit === 0 ? 0 : size >= 10 ? 1 : 2
  return `${new Intl.NumberFormat('en-US', { maximumFractionDigits: digits }).format(size)} ${units[unit]}`
}

/** A date the way the app writes it in English (`useFormat().date`). */
export function formatDate(at: number) {
  return new Intl.DateTimeFormat('en-US', { dateStyle: 'medium' }).format(at)
}

/** "18 conversation files · 2.05 MB", the choose step's line for a tool. */
export function toolSummary(conversationFiles: number, bytes: number) {
  return `${conversationFiles.toLocaleString('en-US')} conversation file${conversationFiles === 1 ? '' : 's'} · ${formatBytes(bytes)}`
}

export const enableDialog = (page: Page) =>
  page
    .getByRole('dialog')
    .filter({ has: page.getByRole('list', { name: 'Steps' }) })

export const previewTable = (page: Page) =>
  enableDialog(page).getByRole('table', {
    name: 'Tool folders found on this computer',
  })

/** The preview's row for the folder at `path`. */
export const folderRow = (page: Page, path: string) =>
  previewTable(page)
    .locator('tbody tr')
    .filter({ has: page.getByText(path, { exact: true }) })

/** The choose step's tick for a tool, named by the tool alone. */
export const toolTick = (page: Page, tool: AgentTool) =>
  enableDialog(page).getByRole('checkbox', {
    name: TOOL_NAMES[tool],
    exact: true,
  })

/**
 * The choose step's line for a tool: its tick and what it would save. (A
 * `has` locator is looked for inside the row, so it starts from the page.)
 */
export const toolChoice = (page: Page, tool: AgentTool) =>
  enableDialog(page)
    .getByRole('listitem')
    .filter({
      has: page.getByRole('checkbox', { name: TOOL_NAMES[tool], exact: true }),
    })

/** Opens the enable dialog over Home, the way its address does. */
export async function openEnableDialog(page: Page) {
  await page.goto('/#/browsing/home?enable=agent')
  await unlockIfLocked(page)
  await expect(enableDialog(page)).toBeVisible()
}

/** Waits until the preview has finished: "Next" turns on. */
export async function waitForPreview(page: Page) {
  await expect(
    enableDialog(page).getByRole('button', { name: 'Next' }),
  ).toBeEnabled({ timeout: 60_000 })
}

/** The folder's row reads as the fixture's facts. */
export async function expectFolderRow(page: Page, folder: ExpectedFolder) {
  const cells = folderRow(page, folder.path).getByRole('cell')
  await expect(cells.nth(0)).toHaveText(folder.name)
  await expect(cells.nth(1)).toHaveText(`${folder.label ?? ''}${folder.path}`)
  const files = folder.conversationFiles.toLocaleString('en-US')
  const subagents = `plus ${folder.subagentFiles} subagent file${folder.subagentFiles === 1 ? '' : 's'}`
  await expect(cells.nth(2)).toHaveText(
    new RegExp(
      `^${files}${folder.subagentFiles > 0 ? `\\s*${subagents}` : ''}$`,
    ),
  )
  await expect(cells.nth(3)).toHaveText(formatBytes(folder.bytes))
  await expect(cells.nth(4)).toHaveText(formatDate(folder.earliest))
}

/**
 * Moves every tool folder out of the fake home until the returned function
 * puts them back, so the app finds no tool at all.
 */
export function hideAllToolFolders() {
  const { agents } = fixture()
  const moved = ROOT_ORDER.map((root) => agents.roots[root]).filter((path) =>
    existsSync(path),
  )
  for (const path of moved) renameSync(path, `${path}.away`)
  return () => {
    for (const path of moved)
      if (existsSync(`${path}.away`)) renameSync(`${path}.away`, path)
  }
}

interface AgentSettings {
  policy: { enabledTools: AgentTool[]; readGit: boolean }
  policyToken: string
}

export function agentSettings() {
  return backend<AgentSettings>('get_agent_settings', { request: {} })
}

/**
 * Puts the tool switches and `.git` back the way a new archive shows them:
 * every tool ticked, `.git` on. (The backend tells "never chosen" from
 * "every tool unticked" (`toolsChosen`), and nothing turns a choice back
 * into "never chosen"; every tool ticked is what both show while off.)
 */
export async function resetAgentSettings() {
  const now = await agentSettings()
  await backend('update_agent_sources', {
    request: {
      expectedToken: now.policyToken,
      readGit: true,
      toolChanges: Object.fromEntries(TOOL_ORDER.map((tool) => [tool, true])),
    },
  })
}

export interface PreviewFrame {
  type: string
  payload: { rows?: unknown[]; byTool?: object; code?: string }
}

/**
 * A preview run through the backend, the way the window starts one, with
 * its frames read to the end over the bridge
 * (`/previews/{streamId}/events`, one reader, closed after the last frame).
 */
export async function previewFrames(request: object): Promise<PreviewFrame[]> {
  const { devIpcUrl } = resolveDesktopBridgeEnv(process.env)
  const started = await backend<{ streamId: string }>('preview_agent_sources', {
    request,
  })
  const response = await fetch(
    `${devIpcUrl}/previews/${encodeURIComponent(started.streamId)}/events`,
  )
  return (await response.text())
    .split('\n')
    .filter((line) => line.startsWith('data:'))
    .map((line) => JSON.parse(line.slice('data:'.length)) as PreviewFrame)
}
