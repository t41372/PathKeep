/**
 * Typed client for backing up Agent conversations: turning the kind on with
 * a reviewed preview (the first save), starting a backup, what the archive
 * holds and where it came from, the run ledger, and the background tasks
 * those backups are.
 *
 * Responsible for: command names, request envelopes, the wire shapes and
 * the events' names and payloads. Not responsible for: caching or keeping
 * answers current (`src/lib/queries/agent-backup.ts`), or any copy.
 *
 * Source of truth: `docs/architecture/desktop-command-surface.md`
 * (Conversation backups, runs and Tasks) and the recorded exchange in
 * `src-tauri/tests/fixtures/agent-backup.json`. Counts and sizes are
 * decimal strings (`WireCount`); times are UTC epoch milliseconds.
 */
import type { AgentToolId, DataKindId, WireCount } from './data-kinds'
import { call } from './shared'

/**
 * Where a backup came from: the enable flow's first save, a button, a
 * retry, the schedule (`scheduled`), or the menu bar icon's Back up now.
 */
export type BackupTrigger =
  'enable' | 'manual' | 'retry' | 'scheduled' | 'menu-bar' | (string & {})

/** A run's outcome; `running` until it ends. */
export type RunResult =
  'running' | 'success' | 'partial' | 'interrupted' | 'failed'

/** What a backup has to show at a moment: no total is known while it runs. */
export interface BackupProgress {
  phase: 'discovery' | 'capture' | 'index' | 'finishing' | (string & {})
  unit: 'files' | 'bytes' | 'conversations' | 'children' | (string & {})
  done: WireCount
  total: WireCount | null
}

/**
 * One tool's part of a running backup, each count since it started.
 * `newRoots` are conversations seen for the first time, `updatedRoots`
 * conversations with new content, `newChildren` subagents.
 */
export interface ToolProgress {
  tool: AgentToolId
  filesFound: WireCount
  filesCommitted: WireCount
  bytesCommitted: WireCount
  newRoots: WireCount
  updatedRoots: WireCount
  newChildren: WireCount
}

/**
 * `queued`: a first save the backend has recorded but not yet begun; it
 * offers no action for that moment and reads as still at work. `paused`:
 * its last step, making what it saved searchable, held by the person
 * (only that step pauses); it waits with Continue and Cancel.
 */
export type TaskState =
  | 'queued'
  | 'running'
  | 'cancelling'
  | 'paused'
  | 'succeeded'
  | 'partial'
  | 'interrupted'
  | 'failed'

/**
 * `dismiss` puts away what was not saved; `dismiss-notice` one format
 * notice (with its `notice`). A task can offer both, and each leaves the
 * other in place.
 */
export type TaskAction =
  'cancel' | 'retry' | 'dismiss' | 'dismiss-notice' | 'pause' | 'resume'

/**
 * A conversation backup as a background task. `snapshotToken` changes with
 * every state and phase; an action sent with an older one is refused.
 */
export interface KindTask {
  taskId: string
  kind: DataKindId
  operation: 'backup'
  trigger: BackupTrigger
  state: TaskState
  phase: string
  runRef: string
  startedAt: number
  /**
   * Ended partial or failed, or met a format version not read yet, and
   * nobody has dismissed it. Notes alone never set it.
   */
  attention: boolean
  actions: TaskAction[]
  /** Quitting PathKeep asks first while this runs. */
  holdsQuit: boolean
  snapshotToken: string
  endedAt?: number
  /** The task this one retries. */
  retryOf?: string
  /** Format versions its run met first; only `dismiss` is offered for them. */
  formatNotices?: FormatNotice[]
  /** Why it stopped: `cancelled`, `owner-restarted`, an error code. */
  reason?: string
  perTool?: ToolProgress[]
  progress?: BackupProgress
}

export interface TaskSummary {
  runningCount: number
  queuedCount: number
  attentionCount: WireCount
  running: KindTask[]
}

export type TaskGroup = 'running' | 'waiting' | 'attention' | 'today'

/** One page of a list; pass `nextCursor` back for the next. */
export interface CursorPage<T> {
  items: T[]
  viewToken: string
  coverage: { state: string; reasons: string[] }
  nextCursor?: string
}

/** What a conversation run did, in its own units; `null` where an older run did not count it. */
export interface AgentRunSummary {
  type: 'agent-history'
  retainedBytes: WireCount | null
  filesVisited: WireCount | null
  filesAbsent: WireCount | null
  filesCaptured: WireCount | null
  filesUnchanged: WireCount | null
  sourceBytes: WireCount | null
  newRoots: WireCount | null
  updatedRoots: WireCount | null
  newChildren: WireCount | null
  unavailable: WireCount | null
  unstable: WireCount | null
  unknownFormat: WireCount | null
  unclassified: WireCount | null
  attachmentsChanged: WireCount | null
}

/** A browsing-ledger run listed beside conversation runs (archive-wide ones on the Agent page). */
export interface LedgerRunSummary {
  type: 'browsing' | 'import' | 'maintenance'
  stats: unknown
}

/**
 * A format version of a tool that this PathKeep cannot read yet. Its files
 * are saved as they are; the notice asks for attention once per tool and
 * version until dismissed.
 */
export interface FormatNotice {
  tool: AgentToolId
  version: string
}

export interface ArchiveRun {
  /** `agent-history:<id>` or `browsing:<origin>/<id>`. */
  runRef: string
  scope: 'agent-history' | 'browsing' | 'archive'
  operation: string
  trigger: string
  result: RunResult | (string & {})
  startedAt: number | null
  summary: AgentRunSummary | LedgerRunSummary
  /** Data the user chose that was not saved (conversation runs since notes). */
  warningCount: WireCount
  /**
   * Conversation runs: conditions of saved files (empty, a damaged line, a
   * format not read yet, written while read). They never make a run fail.
   * Absent on runs recorded before notes were counted.
   */
  noteCount?: WireCount | null
  /** Of those, the ones no earlier run had seen. */
  newNoteCount?: WireCount | null
  /** Format notices of this run still waiting to be dismissed. */
  formatNotices?: FormatNotice[]
  finishedAt?: number
  durationMs?: number
  batchId?: string
}

/** Something a run could not save: a source problem, a file it could not read, a failed write. */
export interface RunWarning {
  code: string
  path: string | null
  ordinal: WireCount | null
  detail: string | null
  rootId?: string | null
  tool?: AgentToolId | null
}

/**
 * A condition of a file the run saved: the bytes are in the archive, but
 * reading them as a conversation met something (`code`). `isNew`: this run
 * saw it first; later runs list it again without counting it as new.
 */
export interface RunNote {
  code: string
  path: string | null
  ordinal: WireCount | null
  detail: string | null
  rootId: string
  tool: AgentToolId | null
  isNew: boolean
}

/** A source's notes from its last run, one entry per code. */
export interface SourceNoteGroup {
  code: string
  count: WireCount
  newCount: WireCount
}

/**
 * How a folder fared. `available`: read; `stale`: no outcome yet (it is
 * running, or never got there); `disabled`: its tool is switched off; the
 * rest say why it was not read, or not read whole.
 */
export type SourceAvailability =
  | 'available'
  | 'stale'
  | 'disabled'
  | 'unobserved'
  | 'partial'
  | 'root-missing'
  | 'root-unreadable'
  | 'root-changed'
  | 'unknown-format'
  | 'changing'
  | 'unreadable'
  | 'destination-unavailable'
  | (string & {})

export interface RunRoot {
  rootId: string
  tool: AgentToolId
  path: string
  availability: SourceAvailability
  counts: AgentRunSummary
}

export interface RunTool {
  tool: AgentToolId
  availability: SourceAvailability
  /** How many of its folders ended each way. */
  rootOutcomes: Record<string, WireCount>
  counts: AgentRunSummary
}

/** One run in full. Warnings and folders come a page at a time (`nextCursor` → `detailCursor`). */
export interface ArchiveRunDetail extends ArchiveRun {
  warnings?: RunWarning[]
  /** Conversation runs: the notes, a page at a time with the warnings. */
  notes?: RunNote[]
  /** Conversation runs: whether `.git` was read. */
  readGit?: boolean
  roots?: RunRoot[]
  perTool?: RunTool[]
  coverage?: { state: string; reasons: string[] }
  nextCursor?: string
  /**
   * The run's task with its actions and current token, for a conversation
   * run started from the app, also after a restart or the next day.
   */
  task?: KindTask
}

/** What the archive holds of one source, from what backups committed. */
export interface SourceCounts {
  transcriptFiles: WireCount
  attachmentFiles: WireCount
  /** Conversations. */
  roots: WireCount
  /** Subagents. */
  children: WireCount
  /** Conversations whose files are gone from the tool's folder. */
  onlyInPathkeep: WireCount
  sourceBytes: WireCount
  /** Transcripts saved whose conversation is not known yet. */
  unclassified?: WireCount
}

export type SourcePart =
  'conversations' | 'subagents' | 'attachments' | 'retained-after-removal'

/**
 * A source on the backup page: a folder approved for a tool, or a tool with
 * no approved folder (`rootId` and `path` null, `counts` null).
 */
export interface AgentSource {
  rootId: string | null
  tool: AgentToolId
  path: string | null
  enabled: boolean
  availability: SourceAvailability
  parts: SourcePart[]
  counts: SourceCounts | null
  sourceBytes: WireCount | null
  /** The tool deletes old conversations unless told otherwise (Claude Code). */
  autoDeletesByDefault: boolean
  /**
   * Which folder at `path` was approved. A folder made again at the same
   * path has another token, and is read only once approved again.
   */
  directoryToken?: string
  /** The last run that read this folder. */
  lastRun?: string
  /** Why the last run could not read it, or not all of it. */
  reason?: string
  /** Notes from its last run; absent before notes were counted. */
  noteCount?: WireCount
  newNoteCount?: WireCount
  notes?: SourceNoteGroup[]
}

export interface ToolTotals extends SourceCounts {
  tool: AgentToolId
  enabled: boolean
}

/** The backup page's header and footer, and the switcher's line, from committed facts. */
export interface AgentOverview extends SourceCounts {
  lookedIn: string[]
  sources: AgentSource[]
  perTool: ToolTotals[]
  coverage: { state: string; reasons: string[] }
  /** The first conversation run's result, `not-started` before there is one. */
  firstBackup: RunResult | 'not-started' | (string & {})
  /** The newest run, which may still be running. */
  lastRun?: ArchiveRun
  /** The backup running now. */
  activeTask?: FirstSave
  /**
   * Bytes the saved conversations take in the archive. Counting reads the
   * archive files, so the backup page asks for it after it is drawn.
   */
  archiveBytes?: WireCount
}

/** The run a request started, or the one already running. */
export interface FirstSave {
  taskId: string
  runRef: string
  requestId: string
}

export interface EnableRequest {
  /** A fresh UUID per request; the same id with the same body answers the same. */
  requestId: string
  previewId: string
  selectedRootIds: string[]
  readGit: boolean
  expectedPolicyToken: string
}

export interface EnableResult {
  state: 'on'
  policyToken: string
  firstSave: FirstSave
}

export type BackupScope =
  | { type: 'kind'; kind: DataKindId }
  | { type: 'tool'; kind: DataKindId; tool: AgentToolId }

export interface StartResult {
  batchId: string
  kinds: {
    kind: DataKindId
    decision: 'started' | 'already-running' | (string & {})
    taskId?: string
    runRef?: string
    reason?: string
  }[]
}

export interface ControlResult {
  task: KindTask
  /** A retry that started: the new task. */
  replacementTaskId?: string
  /** The folders a retry would read are no longer approved as they were. */
  previewRequired?: boolean
}

/** Sent with each step of a running backup, at most about ten a second. */
export const KIND_BACKUP_PROGRESS_EVENT = 'pathkeep://kind-backup-progress'
export interface KindBackupProgressEvent extends BackupProgress {
  perTool?: ToolProgress[]
  batchId: string
  taskId: string
  kind: DataKindId
  trigger: BackupTrigger
}

/** A task changed state, phase, actions or progress. The live `progress` carries `perTool` inside it. */
export const BACKGROUND_TASK_CHANGED_EVENT =
  'pathkeep://background-task-changed'
export interface BackgroundTaskChangedEvent {
  taskId: string
  state: TaskState
  phase: string
  actions: TaskAction[]
  attention: boolean
  snapshotToken: string
  progress?: BackupProgress & { perTool?: ToolProgress[] }
  formatNotices?: FormatNotice[]
}

/** A backup of a kind ended, however it was started. */
export const KIND_BACKUP_FINISHED_EVENT = 'pathkeep://kind-backup-finished'
export interface KindBackupFinishedEvent {
  batchId: string
  taskId: string
  runRef: string
  kind: DataKindId
  trigger: BackupTrigger
  result: RunResult
  summary: AgentRunSummary
  /** What was not saved; empty when only notes were left. */
  reasonCodes: string[]
  noteCount?: WireCount | null
  newNoteCount?: WireCount | null
  formatNotices?: FormatNotice[]
}

/** What the backup page and the switcher's line read changed. */
export const AGENT_CATALOG_CHANGED_EVENT = 'pathkeep://agent-catalog-changed'
/** The tools, folders or `.git` choice changed while the kind is on or paused. */
export const AGENT_SETTINGS_CHANGED_EVENT = 'pathkeep://agent-settings-changed'

export const agentBackupClient = {
  /** Turns conversations on with the reviewed folders and starts the first save. */
  enable: (request: EnableRequest) =>
    call<EnableResult>('enable_agent_history', { request }),
  /** One backup per kind at a time: a second request gets the running one back. */
  start: (scope: BackupScope, requestId: string, shared = false) =>
    call<StartResult>('start_backup', {
      request: { scope, requestId, shared },
    }),
  overview: (purpose: 'backup' | 'space') =>
    call<AgentOverview>('get_agent_overview', { request: { purpose } }),
  sources: (limit: number, cursor?: string) =>
    call<CursorPage<AgentSource>>('list_agent_sources', {
      request: { limit, ...(cursor ? { cursor } : {}) },
    }),
  /** This space's runs, newest first, with what was done to the whole archive. */
  runs: (limit: number, cursor?: string) =>
    call<CursorPage<ArchiveRun>>('list_archive_runs', {
      request: {
        space: 'agent-history',
        includeArchiveWide: true,
        limit,
        ...(cursor ? { cursor } : {}),
      },
    }),
  run: (runRef: string, detailCursor?: string) =>
    call<ArchiveRunDetail>('get_archive_run', {
      request: { runRef, ...(detailCursor ? { detailCursor } : {}) },
    }),
  taskSummary: () => call<TaskSummary>('get_task_summary', { request: {} }),
  tasks: (group: TaskGroup, limit: number, cursor?: string) =>
    call<CursorPage<KindTask>>('list_background_tasks', {
      request: { group, limit, ...(cursor ? { cursor } : {}) },
    }),
  /** `notice` names the one format notice `dismiss-notice` puts away. */
  control: (
    taskId: string,
    action: TaskAction,
    expectedToken: string,
    requestId: string,
    notice?: FormatNotice,
  ) =>
    call<ControlResult>('control_background_task', {
      request: {
        taskId,
        action,
        expectedToken,
        requestId,
        ...(notice ? { notice } : {}),
      },
    }),
}
