/**
 * Typed client for the kinds of data PathKeep keeps (browsing, Agent
 * conversations): their on / paused / off state, the Agent tools' switches,
 * which tool folders exist on this computer, and the metadata preview of
 * what turning Agent conversations on would read.
 *
 * Responsible for: command names, request envelopes and the wire shapes,
 * including the refusal codes these commands use. Not responsible for:
 * the preview's transport (`src/lib/ipc/agent-preview-stream.ts`), caching
 * (`src/lib/queries/data-kinds.ts`) or any copy.
 *
 * Source of truth: `docs/architecture/desktop-command-surface.md`
 * (Conversation sources and data kinds) and the wire examples in
 * `src-tauri/tests/fixtures/data-kinds-preview.json`. Every command takes
 * one `request` object, even an empty one. Counts and byte sizes are
 * decimal strings on the wire (`WireCount`); dates are UTC epoch
 * milliseconds.
 */
import { commandErrorCode } from '../ipc/command-error'
import { openAgentPreview } from '../ipc/agent-preview-stream'
import { call } from './shared'

/** The backend's id for a kind of data. The `agent` space is `agent-history`. */
export type DataKindId = 'browsing' | 'agent-history'

/** Paused keeps the kind visible and stops scheduled backups; off also hides it. */
export type KindState = 'on' | 'paused' | 'off'

export type KindAvailability = 'available' | 'not-initialized' | 'unavailable'

/** The Agent tools PathKeep reads, in the order the backend lists them. */
export const AGENT_TOOLS = ['claude-code', 'codex', 'pi', 'deepseek'] as const
export type AgentToolId = (typeof AGENT_TOOLS)[number]

/**
 * A count or a size in bytes, as a decimal string. File counts and byte
 * sizes stay far below 2^53, so the screens read them with `Number()`.
 */
export type WireCount = string

/** What `archiveCount` counts: browsing counts visits, Agent conversations count source files. */
export type CountUnit = 'visits' | 'source-files'

/** Which part of a kind's storage failed to open. */
export type KindFailureRole = 'evidence' | 'read-model' | 'shared'
export type KindFailureStage =
  'preflight' | 'open' | 'key-check' | 'migrate' | 'recover'
export type KindFailureReason =
  | 'archive-store-missing'
  | 'wrong-key'
  | 'integrity-failed'
  | 'schema-too-new'
  | 'archive-busy'
  | 'kind-unavailable'

export interface KindFailure {
  role: KindFailureRole
  stage: KindFailureStage
  reason: KindFailureReason
}

export interface DataKindRow {
  kind: DataKindId
  order: number
  state: KindState
  availability: KindAvailability
  /** `null` when the kind's storage cannot be opened, so nobody knows. */
  hasSavedData: boolean | null
  /** Absent when the kind's storage cannot be opened. */
  archiveCount?: WireCount
  countUnit: CountUnit
  coverage: { state: string; reasons: string[] }
  /** Agent conversations only: the first backup's status, `not-started` before it. */
  firstBackup?: string
  /** Set when one part of the kind's storage failed and the rest still opens. */
  failure?: KindFailure
}

export interface DataKindList {
  kinds: DataKindRow[]
  /** Fingerprint of the stored choices; a change sent with an older one is refused (`policy-changed`). */
  policyToken: string
}

export interface KindTransition extends DataKindList {
  /** Tasks the change stopped or queued; always empty until the tasks step lands. */
  affectedTasks: unknown[]
}

export interface AgentPolicy {
  browsing: KindState
  agentHistory: KindState
  /** Tools whose switch is on, in `AGENT_TOOLS` order. */
  enabledTools: AgentToolId[]
  readGit: boolean
}

export interface AgentSettings {
  policy: AgentPolicy
  policyToken: string
}

export interface AgentSourcesPatch {
  expectedToken: string
  readGit?: boolean
  /** Only the tools named change. */
  toolChanges?: Partial<Record<AgentToolId, boolean>>
}

/** Why a folder could not be read, or was read only in part. */
export type RootReason =
  | 'root-missing'
  | 'root-unreadable'
  | 'root-changed'
  | 'scope-denied'
  | 'unsupported-format'
  | 'source-depth-limit'

/** Whether one known Agent tool folder exists. Nothing inside it was listed. */
export interface AgentToolPresence {
  kind: 'agent-history'
  tool: AgentToolId
  found: boolean
  readable: boolean
  reason?: RootReason
  rootIds: string[]
  path: string
}

export interface BrowserPresence {
  kind: 'browsing'
  found: boolean
  readable: boolean
  reason?: string
  /** Absent when the system hides every profile of a browser. */
  profileId?: string
  rootIds: string[]
}

export interface PresenceProfile {
  profileId: string
  browserName: string
  profileName: string
  readable: boolean
}

export interface DetectedSources {
  sources: (AgentToolPresence | BrowserPresence)[]
  profiles: PresenceProfile[]
  browserDiscoveryIssue?: unknown
}

export type PreviewPurpose = 'onboarding' | 'enable' | 'rescan'

export interface AgentPreviewRequest {
  purpose: PreviewPurpose
  /** Only these tools; all known tools when absent. */
  tools?: AgentToolId[]
  rootIds?: string[]
  /** A folder the person picked in the system's folder picker, for one tool. */
  pickedFolder?: { tool: AgentToolId; path: string }
}

export interface PreviewStarted {
  streamId: string
  viewToken: string
}

/**
 * What the preview counted, without opening any file. The five file
 * classes do not overlap and add up to `files`; `sourceBytes` covers all
 * five. `childFiles` is non-zero only where the folder layout shows which
 * transcripts are subagents (Claude Code); elsewhere every transcript is
 * `ambiguousFiles`.
 */
export interface InventoryCounts {
  folders: WireCount
  files: WireCount
  sourceBytes: WireCount
  compressedBytes: WireCount
  topLevelFiles: WireCount
  childFiles: WireCount
  ambiguousFiles: WireCount
  ancillaryFiles: WireCount
  unknownFiles: WireCount
}

/** `unavailable` when the folder could not be read at all. */
export type CountBasis = 'known-layout' | 'file-candidates' | 'unavailable'

/** One folder's reading. A readable folder with a `reason` was read only in part. */
export interface RootInventory {
  rootId: string
  tool: AgentToolId
  path: string
  readable: boolean
  reason?: RootReason
  counts: InventoryCounts
  countBasis: CountBasis
  dateBasis: 'filesystem-modified'
  earliestAt: number | null
  latestAt: number | null
}

export interface ToolInventory extends InventoryCounts {
  roots: number
  readableRoots: number
  earliestAt: number | null
  latestAt: number | null
  reasons: RootReason[]
}

export interface PreviewReport {
  /** Hands the reviewed folders to the step that saves them, until `expiresAt`. */
  previewId: string
  rows: RootInventory[]
  totals: InventoryCounts
  byTool: Partial<Record<AgentToolId, ToolInventory>>
  /** An estimate, not a measurement: see `estimateBasis`. */
  estimatedArchiveBytes: WireCount
  estimateBasis: {
    method: string
    nativeCompressedBytes: WireCount
    ordinaryBytes: WireCount
    derivedAllowanceBytes: WireCount
    overheadAllowanceBytes: WireCount
  }
  /** Free space on the disk the conversations would be saved to. */
  availableBytes: WireCount
  archiveMode: 'Plaintext' | 'Encrypted'
  expiresAt: number
}

interface FrameBase {
  streamId: string
  sequence: WireCount
}

/** One message of a running preview. `finished`, `cancelled` and `error` end it. */
export type PreviewFrame = FrameBase &
  (
    | {
        type: 'progress'
        payload: { phase: 'discovery'; unit: 'entries'; done: WireCount }
      }
    | { type: 'root'; payload: RootInventory }
    | { type: 'finished'; payload: PreviewReport }
    | { type: 'cancelled'; payload: { code: 'cancelled' } }
    | { type: 'error'; payload: { code: string } }
  )

/** The event sent after the stored choices changed (not for a change that changed nothing). */
export const DATA_KINDS_CHANGED_EVENT = 'pathkeep://data-kinds-changed'

export interface DataKindsChanged {
  policyToken: string
  changedKinds: DataKindId[]
}

/** Every refusal these commands use; App Lock and archive refusals keep their own codes too. */
export const DATA_KINDS_ERROR_CODES = [
  'last-active-kind',
  'lock-required',
  'invalid-request',
  'enable-preview-required',
  'policy-changed',
  'source-selection-empty',
  'scope-denied',
  'kind-unavailable',
  'archive-uninitialized',
  'archive-busy',
  'archive-replaced',
  'preview-expired',
  'root-changed',
  'root-unreadable',
  'destination-unavailable',
  'cancelled',
] as const
export type DataKindsErrorCode = (typeof DATA_KINDS_ERROR_CODES)[number]

function isDataKindsErrorCode(code: string): code is DataKindsErrorCode {
  return (DATA_KINDS_ERROR_CODES as readonly string[]).includes(code)
}

/**
 * The refusal code of a failed command, or of a preview's `error` frame,
 * when it is one of `DataKindsErrorCode`; otherwise `null`.
 */
export function dataKindsErrorCode(
  errorOrCode: unknown,
): DataKindsErrorCode | null {
  const code =
    typeof errorOrCode === 'string'
      ? errorOrCode
      : commandErrorCode(errorOrCode)
  return code !== null && isDataKindsErrorCode(code) ? code : null
}

/**
 * The engine's causes for a kind's own store that will not open. By then the
 * archive itself is open, so each one, `wrong-key` included, means that one
 * file can no longer be read: damage, never a wrong password.
 */
const STORE_DAMAGE_CODES: ReadonlySet<string> = new Set([
  'kind-unavailable',
  'wrong-key',
  'integrity-failed',
  'archive-store-missing',
  'schema-too-new',
])

/** Whether a failed command, or a preview's `error` frame, reports a damaged or unreadable store. */
export function isStoreDamage(errorOrCode: unknown): boolean {
  const code =
    typeof errorOrCode === 'string'
      ? errorOrCode
      : commandErrorCode(errorOrCode)
  return code !== null && STORE_DAMAGE_CODES.has(code)
}

export const dataKindsClient = {
  list: () => call<DataKindList>('list_data_kinds', { request: {} }),
  setState: (kind: DataKindId, state: KindState, expectedPolicyToken: string) =>
    call<KindTransition>('set_data_kind_state', {
      request: { kind, state, expectedPolicyToken },
    }),
  agentSettings: () =>
    call<AgentSettings>('get_agent_settings', { request: {} }),
  updateAgentSources: (patch: AgentSourcesPatch) =>
    call<AgentSettings>('update_agent_sources', { request: patch }),
  /** Only checks which folders exist; opens and lists nothing inside them. */
  detectSources: (kinds: DataKindId[]) =>
    call<DetectedSources>('detect_data_sources', { request: { kinds } }),
  /** Starts a preview; frames go to `onFrame` until it ends or is closed. */
  previewAgentSources: openAgentPreview,
  /** Answers the same for a preview that already ended. */
  cancelAgentRead: (streamId: string) =>
    call<{ state: 'cancelled' }>('cancel_agent_read', {
      request: { requestIdOrStreamId: streamId },
    }),
}
