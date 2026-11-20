/**
 * The Agent tools onboarding's sources step lists: one walk of every known
 * tool folder (counts, sizes and dates only; no file is opened), read as
 * one row per tool, since a tool is what the person ticks.
 *
 * Responsibilities: running the walk while it is wanted (the steps from
 * "Sources" on, with Agent conversations chosen), adding a tool's folders
 * up, and which tools are chosen and can be read.
 * Not responsible for: the walk itself (`use-agent-preview.ts` and the
 * backend), or drawing (`agent-sources.tsx`).
 */
import { useMemo } from 'react'
import {
  type AgentPreviewRequest,
  type AgentToolId,
  type RootInventory,
} from '@/lib/backend-client/data-kinds'
import {
  conversationFiles,
  shownRows,
  subagentFiles,
  wire,
} from '@/features/enable-agent/preview-model'
import { useAgentPreview } from '@/features/enable-agent/use-agent-preview'
import type { Draft } from './draft'

/** One tool and its folders, as the sources step lists it. */
export interface AgentToolSource {
  tool: AgentToolId
  /** Its folders that are on this computer, in the walk's order. */
  roots: RootInventory[]
  /** At least one of its folders can be read. */
  readable: boolean
  /** Folders whose layout lets their files be counted. */
  counted: boolean
  conversationFiles: number
  subagentFiles: number
  bytes: number
}

/**
 * The folders found so far, one entry per tool, in the backend's tool order.
 * A folder counted twice (a walk started again while the step was left and
 * reopened adds to what the stopped one had counted) is taken once, at its
 * latest count.
 */
export function toolSources(rows: readonly RootInventory[]): AgentToolSource[] {
  const latest = new Map(rows.map((row) => [row.rootId, row]))
  const tools = new Map<AgentToolId, AgentToolSource>()
  for (const row of shownRows([...latest.values()])) {
    const entry = tools.get(row.tool) ?? {
      tool: row.tool,
      roots: [],
      readable: false,
      counted: false,
      conversationFiles: 0,
      subagentFiles: 0,
      bytes: 0,
    }
    entry.roots.push(row)
    if (row.readable) {
      entry.readable = true
      entry.bytes += wire(row.counts.sourceBytes)
      if (row.countBasis !== 'unavailable') {
        entry.counted = true
        entry.conversationFiles += conversationFiles(row.counts)
        entry.subagentFiles += subagentFiles(row.counts)
      }
    }
    tools.set(row.tool, entry)
  }
  return [...tools.values()]
}

const ONBOARDING: AgentPreviewRequest = { purpose: 'onboarding' }

export function useAgentSources(active: boolean) {
  const preview = useAgentPreview(active, ONBOARDING)
  const { state } = preview
  const rows =
    state.phase === 'walking'
      ? state.roots
      : state.phase === 'finished'
        ? state.report.rows
        : null
  const tools = useMemo(() => toolSources(rows ?? []), [rows])
  const readable = useMemo(
    () => tools.filter((tool) => tool.readable).map((tool) => tool.tool),
    [tools],
  )
  return {
    state,
    tools,
    /** Tools with a folder PathKeep can read, once the walk has ended. */
    readable: state.phase === 'finished' ? readable : null,
    rerun: preview.rerun,
  }
}

export type AgentSources = ReturnType<typeof useAgentSources>

/** The ticked tools: the person's, else every tool PathKeep can read. */
export function chosenTools(
  draft: Draft,
  sources: AgentSources,
): AgentToolId[] {
  if (draft.agentTools) return draft.agentTools
  return sources.tools.filter((tool) => tool.readable).map((tool) => tool.tool)
}

/** The ticked tools PathKeep can read; `null` until the walk has ended. */
export function chosenReadableTools(
  draft: Draft,
  sources: AgentSources,
): AgentToolId[] | null {
  if (!sources.readable) return null
  const readable = new Set(sources.readable)
  return chosenTools(draft, sources).filter((tool) => readable.has(tool))
}
