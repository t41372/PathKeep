/**
 * Reading a preview's counts the way the enable dialog shows them. The
 * preview never opens a file, so it cannot know conversations; it knows
 * files that look like conversation transcripts (`topLevelFiles` where the
 * layout says so, `ambiguousFiles` where it cannot tell a conversation
 * from a subagent) and, where the layout shows them, subagent files
 * (`childFiles`). Sizes cover every file the walk counted.
 *
 * Not responsible for: wording (`enable-agent.ts` messages) or the walk.
 */
import {
  AGENT_TOOLS,
  type AgentToolId,
  type InventoryCounts,
  type PreviewReport,
  type RootInventory,
  type RootReason,
} from '@/lib/backend-client/data-kinds'
import type { MessageKey } from '@/lib/i18n'

/** A decimal string from the wire as a number; counts and sizes stay far below 2^53. */
export function wire(value: string): number {
  const number = Number(value)
  return Number.isFinite(number) ? number : 0
}

/** Files that look like conversation transcripts. */
export function conversationFiles(counts: InventoryCounts) {
  return wire(counts.topLevelFiles) + wire(counts.ambiguousFiles)
}

export function subagentFiles(counts: InventoryCounts) {
  return wire(counts.childFiles)
}

/** A folder that is not there is not shown: the tool is simply not on this computer. */
export function isFound(row: RootInventory) {
  return row.reason !== 'root-missing'
}

/** The note under a folder that could not be read, or was read only in part. */
export function rowNote(row: RootInventory): MessageKey | null {
  const reason: RootReason | undefined = row.reason
  if (!reason || reason === 'root-missing') return null
  if (reason === 'root-changed') return 'enableAgent.preview.rows.changed'
  if (reason === 'scope-denied') return 'enableAgent.preview.rows.scopeDenied'
  if (!row.readable) return 'enableAgent.preview.rows.unreadable'
  switch (reason) {
    case 'root-unreadable':
      return 'enableAgent.preview.rows.partlyUnreadable'
    case 'source-depth-limit':
      return 'enableAgent.preview.rows.tooDeep'
    case 'unsupported-format':
      return 'enableAgent.preview.rows.unknownFormat'
  }
}

/** Folders in the backend's tool order, the walk's order within a tool. */
export function shownRows(rows: readonly RootInventory[]): RootInventory[] {
  return rows
    .filter(isFound)
    .map((row, index) => ({ row, index }))
    .sort(
      (a, b) =>
        AGENT_TOOLS.indexOf(a.row.tool) - AGENT_TOOLS.indexOf(b.row.tool) ||
        a.index - b.index,
    )
    .map(({ row }) => row)
}

/** Where the preview looked and found nothing, for the "nothing found" state. */
export function lookedIn(rows: readonly RootInventory[]): string[] {
  return rows.filter((row) => !isFound(row)).map((row) => row.path)
}

/** One tool in the choose step. */
export interface ToolChoice {
  tool: AgentToolId
  /** At least one of its folders could be read. */
  readable: boolean
  files: number
  bytes: number
}

/** The tools with a folder on this computer, in the backend's order. */
export function toolChoices(report: PreviewReport): ToolChoice[] {
  const found = new Set(report.rows.filter(isFound).map((row) => row.tool))
  return AGENT_TOOLS.filter((tool) => found.has(tool)).map((tool) => {
    const totals = report.byTool[tool]
    const readable = report.rows.some(
      (row) => row.tool === tool && row.readable,
    )
    return {
      tool,
      readable,
      files: totals ? conversationFiles(totals) : 0,
      bytes: totals ? wire(totals.sourceBytes) : 0,
    }
  })
}
