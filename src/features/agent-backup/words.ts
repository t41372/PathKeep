/**
 * How a conversation backup is put into words, the same on every screen
 * that shows one: its progress in the units it has at each moment (files
 * found, then files and size saved, then conversations once they are
 * known; never a percentage, since no total is known), how a tool's folder
 * fared, what was not saved (warnings), what a saved file's notes mean, a
 * format version not read yet, and what a run did.
 *
 * Responsibilities: choosing the sentence. An outcome or note this build
 * does not know is shown with its code, never left blank.
 * Not responsible for: the copy itself (`agent-tasks` and `agent-backup`
 * messages) or where it is drawn.
 */
import type {
  AgentRunSummary,
  FormatNotice,
  KindTask,
  SourceAvailability,
  ToolProgress,
} from '@/lib/backend-client/agent-backup'
import type { Translator, MessageKey } from '@/lib/i18n'
import { wireNumber } from '@/lib/queries/agent-backup'
import { toolName } from '@/features/data-kinds/tools'

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
 * Outcomes that are a problem to look at: something chosen was not saved.
 * Files kept as they are because their format is not read yet, and files
 * read again next time because they were being written, are not: what was
 * there is saved, and the run left notes about it.
 */
export function isProblem(availability: SourceAvailability) {
  return ![
    'available',
    'disabled',
    'stale',
    'unknown-format',
    'changing',
    'unobserved',
  ].includes(availability)
}

/**
 * Something a run could not save, as a sentence. Since notes are counted
 * apart (`counted`), every warning is data that was not saved, so a code
 * this build has no words for says just that. Runs recorded before then
 * listed reading conditions of saved files here too; those keep the words
 * they had, and an unknown code there is only "a note".
 */
export function warningKey(code: string, counted = true): MessageKey {
  if (code === 'root-missing') return 'agentTasks.warnings.missing'
  if (code === 'root-changed' || code === 'root_changed')
    return 'agentTasks.warnings.changed'
  if (code === 'destination-unavailable')
    return 'agentTasks.warnings.destination'
  if (code === 'archive-publication-failed')
    return 'agentTasks.warnings.writeFailed'
  if (code === 'pi-local-output-limit') return 'agentTasks.warnings.outputLimit'
  if (code === 'authority_unavailable')
    return 'agentTasks.warnings.attachmentUnreadable'
  if (/layout-unrecognized|layout_unrecognized/.test(code))
    return 'agentTasks.warnings.layout'
  if (/depth/.test(code)) return 'agentTasks.warnings.tooDeep'
  if (/non_utf8_path|non-utf8-path|unrepresentable/.test(code))
    return 'agentTasks.warnings.badName'
  if (/^git_/.test(code)) return 'agentTasks.warnings.git'
  if (code === 'attachment_not_retained')
    return 'agentTasks.warnings.attachment'
  if (code === 'source-path-outside-root') return 'agentTasks.warnings.skipped'
  if (counted)
    return /unreadable|unavailable|io_error|not_directory|not-directory|not-regular|enumeration-failed|discovery_failed|root-invalid/.test(
      code,
    )
      ? 'agentTasks.warnings.unreadable'
      : 'agentTasks.warnings.unsaved'
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
  if (/symlink|special-file/.test(code)) return 'agentTasks.warnings.skipped'
  return 'agentTasks.warnings.other'
}

/**
 * Something a run could not save, as a sentence: an attachment it points
 * to that is gone reads differently from one it could not read.
 */
export function warningText(
  t: Translator,
  warning: { code: string; detail?: string | null },
  counted: boolean,
) {
  if (warning.code === 'attachment_not_retained') {
    if (warning.detail === 'missing')
      return t('agentTasks.warnings.attachmentMissing')
    if (warning.detail === 'authority_unavailable')
      return t('agentTasks.warnings.attachmentUnreadable')
  }
  return t(warningKey(warning.code, counted))
}

/** Note codes a person reads the same way, from every code the capture emits. */
const NOTE_GROUPS: [RegExp, MessageKey][] = [
  [/^empty_transcript$/, 'agentTasks.notes.empty'],
  // A file's own details from the file system, not a session's.
  [/^source_metadata_unavailable$/, 'agentTasks.notes.other'],
  [
    /^(source_changed_during_capture|first_record_in_progress|incomplete_tail|source_changed)$|generation-changed|unterminated-tail/,
    'agentTasks.notes.writing',
  ],
  [/^source_decompression_failed$/, 'agentTasks.notes.compressed'],
  [
    /unsupported|format-limited|^session_metadata_unavailable$/,
    'agentTasks.notes.format',
  ],
  [/unrecognized|unknown-required-event/, 'agentTasks.notes.unknownRecords'],
  [
    /metadata|missing_native|missing_history_array|header|entry-identity/,
    'agentTasks.notes.details',
  ],
  [
    /malformed|non_object_record|invalid_utf8|record_too_large|byte_limit|incomplete_document|unexpected_json_shape|record-invalid|sequence-invalid|source_interpretation_failed/,
    'agentTasks.notes.damaged',
  ],
  [
    /symlink|special-file|non_regular_file|not_regular_file/,
    'agentTasks.notes.skipped',
  ],
  [
    /attachment|reference-budget|reference-limit|spill-coverage/,
    'agentTasks.notes.attachment',
  ],
  [
    /lineage|mixed-origins|multiple-session-ids|legacy-derived-identity|ambiguous-generation/,
    'agentTasks.notes.lineage',
  ],
]

/** What a note's code means, without its file's particulars. */
export function noteKey(code: string): MessageKey {
  return (
    NOTE_GROUPS.find(([pattern]) => pattern.test(code))?.[1] ??
    'agentTasks.notes.other'
  )
}

/** One note as a sentence; a format version the parser recognised is named. */
export function noteText(
  t: Translator,
  note: { code: string; detail?: string | null },
) {
  const key = noteKey(note.code)
  if (
    key === 'agentTasks.notes.format' &&
    note.detail &&
    /^\d+$/.test(note.detail)
  )
    return t('agentTasks.notes.formatVersion', { version: note.detail })
  return t(key)
}

/** A format version of a tool that this PathKeep cannot read yet. */
export function formatNoticeText(t: Translator, notice: FormatNotice) {
  return t('agentTasks.formatNotice', {
    tool: t(toolName(notice.tool)),
    version: notice.version,
  })
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
  if (reason === 'terminal-publication-failed')
    return t('agentTasks.reasons.terminalPublication')
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
