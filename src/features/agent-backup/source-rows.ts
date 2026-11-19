/**
 * The backup page's sources as rows: one per tool, listing only the tools
 * that are on this computer, have an approved folder, or already have
 * conversations in the archive (`UI-slice1-features.md` E: never all four
 * regardless). A tool can have several folders (Claude Code's own and the
 * Claude desktop app's); its row adds them up and its state is the one
 * that most needs a look.
 *
 * Not responsible for: drawing (`sources-card.tsx`).
 */
import type {
  AgentOverview,
  AgentSource,
  SourceAvailability,
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
}

/** The worse of two outcomes, for a row with several folders. */
const rank = (availability: SourceAvailability) =>
  availability === 'available'
    ? 0
    : availability === 'stale'
      ? 1
      : availability === 'unknown-format'
        ? 2
        : 3

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
      },
    ]
  })
}
