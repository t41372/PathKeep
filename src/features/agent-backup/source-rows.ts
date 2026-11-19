/**
 * The backup page's sources as rows: one per tool, listing only the tools
 * that are on this computer, have an approved folder, or already have
 * conversations in the archive (`UI-slice1-features.md` E: never all four
 * regardless). A tool can have several folders (Claude Code's own and the
 * Claude desktop app's); its row adds them up and its state is the one
 * that most needs a look, and its notes are the folders' notes added up
 * by code.
 *
 * Not responsible for: drawing (`sources-card.tsx`).
 */
import type {
  AgentOverview,
  AgentSource,
  SourceAvailability,
  SourceNoteGroup,
  ToolTotals,
} from '@/lib/backend-client/agent-backup'
import { AGENT_TOOLS, type AgentToolId } from '@/lib/backend-client/data-kinds'
import { wireNumber } from '@/lib/queries/agent-backup'

export interface ToolRow {
  tool: AgentToolId
  /** Its approved folders. None: not backed up from this computer. */
  roots: AgentSource[]
  /** Its folder is on this computer. */
  found: boolean
  enabled: boolean
  totals: ToolTotals | undefined
  autoDeletes: boolean
  availability: SourceAvailability
  /** The last run that read one of its folders. */
  lastRun: string | undefined
  /** Notes its folders' last backups left; the files are saved. */
  noteCount: number
  newNoteCount: number
  /** One entry per code, in the backend's order. */
  notes: { code: string; count: number; newCount: number }[]
}

/**
 * The worse of two outcomes, for a row with several folders. Files being
 * written and formats not read yet are saved; they rank below anything
 * that was not.
 */
const rank = (availability: SourceAvailability) =>
  availability === 'available'
    ? 0
    : availability === 'changing'
      ? 1
      : availability === 'stale'
        ? 2
        : availability === 'unknown-format'
          ? 3
          : 4

function addNotes(roots: readonly AgentSource[]) {
  const byCode = new Map<
    string,
    { code: string; count: number; newCount: number }
  >()
  for (const group of roots.flatMap(
    (root): SourceNoteGroup[] => root.notes ?? [],
  )) {
    const entry = byCode.get(group.code) ?? {
      code: group.code,
      count: 0,
      newCount: 0,
    }
    entry.count += wireNumber(group.count)
    entry.newCount += wireNumber(group.newCount)
    byCode.set(group.code, entry)
  }
  return [...byCode.values()]
}

export function toolRows(
  sources: readonly AgentSource[],
  overview: AgentOverview | undefined,
  found: ReadonlySet<AgentToolId>,
): ToolRow[] {
  return AGENT_TOOLS.flatMap((tool) => {
    const mine = sources.filter((source) => source.tool === tool)
    const roots = mine.filter((source) => source.rootId !== null)
    const totals = overview?.perTool.find((item) => item.tool === tool)
    const saved =
      wireNumber(totals?.roots) > 0 || wireNumber(totals?.transcriptFiles) > 0
    if (roots.length === 0 && !found.has(tool) && !saved) return []
    const enabled = mine.some((source) => source.enabled)
    const worst = roots.reduce<SourceAvailability>(
      (current, root) =>
        rank(root.availability) > rank(current) ? root.availability : current,
      'available',
    )
    return [
      {
        tool,
        roots,
        found: found.has(tool),
        enabled,
        totals,
        autoDeletes: mine.some((source) => source.autoDeletesByDefault),
        availability: !enabled
          ? 'disabled'
          : roots.length > 0
            ? worst
            : 'stale',
        lastRun: roots.find((root) => root.lastRun)?.lastRun,
        noteCount: roots.reduce(
          (sum, root) => sum + wireNumber(root.noteCount),
          0,
        ),
        newNoteCount: roots.reduce(
          (sum, root) => sum + wireNumber(root.newNoteCount),
          0,
        ),
        notes: addNotes(roots),
      },
    ]
  })
}
