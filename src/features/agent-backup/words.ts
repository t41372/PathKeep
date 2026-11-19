/**
 * How a conversation backup is put into words, the same on every screen
 * that shows one: its progress in the units it has at each moment (files
 * found, then files and size saved, then conversations once they are
 * known; never a percentage, since no total is known), how a tool's folder
 * fared, what a run's notes mean, and what a run did.
 *
 * Responsibilities: choosing the sentence. An outcome or note this build
 * does not know is shown with its code, never left blank.
 * Not responsible for: the copy itself (`agent-tasks` and `agent-backup`
 * messages) or where it is drawn.
 */
import type {
  AgentRunSummary,
  KindTask,
  SourceAvailability,
  ToolProgress,
} from '@/lib/backend-client/agent-backup'
import type { Translator, MessageKey } from '@/lib/i18n'
import { wireNumber } from '@/lib/queries/agent-backup'

type Bytes = (value: number) => string

/** What a running tool has done so far, in the units it has now. */
export function toolProgressText(
  t: Translator,
  bytes: Bytes,
  progress: ToolProgress | undefined,
) {
  if (!progress) return t('agentTasks.progress.waiting')
  const found = wireNumber(progress.filesFound)
  const files = wireNumber(progress.filesCommitted)
  if (files === 0) {
    return found === 0
      ? t('agentTasks.progress.looking')
      : t('agentTasks.progress.found', { count: found })
  }
  const parts: string[] = []
  const conversations =
    wireNumber(progress.newRoots) + wireNumber(progress.updatedRoots)
  if (conversations > 0)
    parts.push(t('agentTasks.progress.conversations', { count: conversations }))
  const subagents = wireNumber(progress.newChildren)
  if (subagents > 0)
    parts.push(t('agentTasks.progress.subagents', { count: subagents }))
  parts.push(
    t('agentTasks.progress.saved', {
      files: t('agentTasks.progress.files', { count: files }),
      size: bytes(wireNumber(progress.bytesCommitted)),
    }),
  )
  return parts.join(t('agentTasks.progress.join'))
}

/** Every tool's part added up: the one line the rail and the header have room for. */
export function taskProgressText(t: Translator, bytes: Bytes, task: KindTask) {
  if (task.state === 'cancelling') return t('agentTasks.progress.stopping')
  if (task.phase === 'index') return t('agentTasks.progress.indexing')
  if (task.phase === 'finishing') return t('agentTasks.progress.finishing')
  const tools = task.perTool ?? []
  if (tools.length === 0) return t('agentTasks.progress.looking')
  const sum = (field: keyof Omit<ToolProgress, 'tool'>) =>
    String(tools.reduce((total, tool) => total + wireNumber(tool[field]), 0))
  return toolProgressText(t, bytes, {
    tool: tools[0].tool,
    filesFound: sum('filesFound'),
    filesCommitted: sum('filesCommitted'),
    bytesCommitted: sum('bytesCommitted'),
    newRoots: sum('newRoots'),
    updatedRoots: sum('updatedRoots'),
    newChildren: sum('newChildren'),
  })
}

export function taskName(t: Translator, task: Pick<KindTask, 'trigger'>) {
  return t(
    task.trigger === 'enable'
      ? 'agentTasks.names.firstSave'
      : 'agentTasks.names.backup',
  )
}

const availabilityKeys: Record<string, MessageKey> = {
  available: 'agentTasks.availability.available',
  disabled: 'agentTasks.availability.disabled',
  stale: 'agentTasks.availability.stale',
  unobserved: 'agentTasks.availability.unobserved',
  partial: 'agentTasks.availability.partial',
  'root-missing': 'agentTasks.availability.rootMissing',
  'root-unreadable': 'agentTasks.availability.rootUnreadable',
  'root-changed': 'agentTasks.availability.rootChanged',
  'unknown-format': 'agentTasks.availability.unknownFormat',
  changing: 'agentTasks.availability.changing',
  unreadable: 'agentTasks.availability.unreadable',
  'destination-unavailable': 'agentTasks.availability.destination',
}

/** How a folder or a tool fared, as a sentence. */
export function availabilityText(
  t: Translator,
  availability: SourceAvailability,
) {
  const key = availabilityKeys[availability]
  return key
    ? t(key)
    : t('agentTasks.availability.other', { code: availability })
}

/**
 * Outcomes that are a problem to look at. Files kept as they are because
 * their format is not read yet are not: everything was saved.
 */
export function isProblem(availability: SourceAvailability) {
  return ![
    'available',
    'disabled',
    'stale',
    'unknown-format',
    'unobserved',
  ].includes(availability)
}

/** A run note's code as a sentence; groups of codes that mean the same to a person. */
export function warningKey(code: string): MessageKey {
  if (code === 'root-missing') return 'agentTasks.warnings.missing'
  if (code === 'root-changed' || code === 'root_changed')
    return 'agentTasks.warnings.changed'
  if (code === 'destination-unavailable')
    return 'agentTasks.warnings.destination'
  if (
    /unsupported|unrecognized|format-limited|unknown-required|session_metadata_unavailable/.test(
      code,
    )
  )
    return 'agentTasks.warnings.format'
  if (
    /source_changed_during_capture|first_record_in_progress|generation-changed|unterminated-tail/.test(
      code,
    )
  )
    return 'agentTasks.warnings.writing'
  if (/unreadable|io_error|unavailable/.test(code))
    return 'agentTasks.warnings.unreadable'
  if (/malformed|invalid/.test(code)) return 'agentTasks.warnings.damaged'
  if (/symlink|special-file|outside-root/.test(code))
    return 'agentTasks.warnings.skipped'
  return 'agentTasks.warnings.other'
}

/** What a conversation run did: new, updated, subagents; or nothing new. */
export function runSummaryText(
  t: Translator,
  summary: AgentRunSummary,
  separator: string,
) {
  const parts: string[] = []
  const added = wireNumber(summary.newRoots)
  const updated = wireNumber(summary.updatedRoots)
  const children = wireNumber(summary.newChildren)
  if (added > 0)
    parts.push(t('agentTasks.finished.newConversations', { count: added }))
  if (updated > 0)
    parts.push(t('agentTasks.finished.updated', { count: updated }))
  if (children > 0)
    parts.push(t('agentTasks.finished.newSubagents', { count: children }))
  return parts.length === 0
    ? t('agentTasks.finished.nothingNew')
    : t('agentTasks.finished.list', { items: parts.join(separator) })
}

/** Why a task ended the way it did, when the backend says. */
export function taskReasonText(t: Translator, reason: string | undefined) {
  if (!reason) return null
  if (reason === 'cancelled') return t('agentTasks.reasons.cancelled')
  if (reason === 'owner-restarted')
    return t('agentTasks.reasons.ownerRestarted')
  return t('agentTasks.reasons.other', { code: reason })
}

/** How long a run took, as the browsing page says it. */
export function runTook(t: Translator, durationMs: number | undefined) {
  if (durationMs === undefined) return null
  const seconds = durationMs / 1000
  if (seconds < 60)
    return t('backup.runs.seconds', { value: seconds.toFixed(1) })
  return t('backup.runs.minutes', {
    minutes: Math.floor(seconds / 60),
    seconds: Math.round(seconds % 60),
  })
}
