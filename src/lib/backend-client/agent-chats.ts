/**
 * Typed client for the Agent space's conversation list and reader: the
 * list and its counts, one conversation's header, info panel and windows
 * of rows, its branches and children, one record's or attachment's bytes,
 * and the marks a person puts on a conversation (star, put away, trash,
 * rename).
 *
 * Responsible for: command names, request envelopes, the wire shapes and
 * the change event's name and payload. Not responsible for: caching or
 * keeping answers current (`src/lib/queries/agent-chats.ts`), or any copy.
 *
 * Source of truth: `docs/architecture/desktop-command-surface.md`
 * (Conversation list and reader) and the recorded exchange in
 * `src-tauri/tests/fixtures/agent-chats.json`. Times are UTC epoch
 * milliseconds, durations milliseconds, sizes bytes, money USD. IDs,
 * publications, cursors and view tokens are opaque strings. A `null`
 * metric means nothing was recorded, which is not zero.
 */
import type { AgentToolId } from './data-kinds'
import { call } from './shared'

/** `user`: the person renamed it; `tool`: the tool named it; `firstPrompt`: the first thing typed. */
export type TitleSource = 'user' | 'tool' | 'firstPrompt' | 'unknown'

/**
 * The project a conversation's working folder belongs to: an issued id
 * (the Projects commands', `src/lib/backend-client/agent-projects.ts`),
 * its display name, and whether it is a repository, a plain folder or the
 * loose folders.
 */
export interface ChatProject {
  id: string
  label: string
  kind: 'repo' | 'folder' | 'scratch' | (string & {})
}

/** One conversation (a root) as the list and the reader's header show it. */
export interface ChatSummary {
  id: string
  /** The reader publication this summary was read from. */
  publication: string
  harness: AgentToolId
  title: string | null
  titleSource: TitleSource
  project: ChatProject | null
  startedAt: number | null
  lastActivityAt: number | null
  /** The last backup that saved this conversation's files. */
  lastSavedAt: number | null
  messages: number
  turns: number
  /** Subagents and their subagents. */
  childCount: number
  /** Sum of its turns' known durations; `null` when no turn has one. */
  agentMs: number | null
  /** The four kinds of tokens, its subagents' included; `null` without usage. */
  tokensTotal: number | null
  sourceBytes: number
  starred: boolean
  archived: boolean
  trashed: boolean
  /** Every file of it is gone from the tool's own folder. */
  onlyInPathKeep: boolean
  /** Its local day, in day groups: `YYYY-MM-DD`, or `null` when undated. */
  day?: string | null
}

export type ChatFlag =
  | 'starred'
  | 'onlyInPathKeep'
  | 'hasPr'
  | 'hasSubagents'
  | 'hasEdits'
  | 'noUsage'
  | 'archived'
  | 'trash'

export type ChatSort =
  'lastActivity' | 'started' | 'agentTime' | 'tokens' | 'turns'
export type ChatGroup = 'day' | 'project' | 'none'
export type ChatRange = 'all' | 'today' | '7d' | '30d' | '90d' | '1y'

/** What the list holds; the cursor is bound to every other field. */
export interface ChatListQuery {
  harnesses?: AgentToolId[]
  projectId?: string
  flags?: ChatFlag[]
  sort?: ChatSort
  group?: ChatGroup
  range?: ChatRange
  /** `YYYY-MM-DD`; instead of `range`. */
  day?: string
  /** IANA zone the days are in. */
  timeZone?: string
  cursor?: string
  /** 1–200; rows, or groups when grouped by project. */
  limit?: number
}

/** A day group on a page: the whole filtered day's count. */
export interface ChatDayGroup {
  day: string | null
  count: number
}

/** A project group: its count, up to three rows, and how many more. */
export interface ChatProjectGroup {
  project: ChatProject | null
  count: number
  remaining: number
  items: ChatSummary[]
}

export interface ChatListPage {
  /** Empty when grouped by project. */
  items: ChatSummary[]
  groups: ChatDayGroup[] | ChatProjectGroup[]
  nextCursor: string | null
  viewToken: string
}

export interface ChatCounts {
  /** Conversations the filters match, and their subagents. */
  matchingRoots: number
  matchingChildren: number
  /** Conversations not in the trash (put away included), for the switcher. */
  spaceRoots: number
  spaceOnlyInPathKeep: number
  /** Everything saved, the trash included. */
  savedRoots: number
  savedChildren: number
  savedOnlyInPathKeep: number
  /** Working folders of conversations not in the trash. */
  folderCount: number
  viewToken: string
}

export type FolderState = 'present' | 'gone' | 'worktreeGone'

/** The reader's header for one conversation. */
export interface ChatHeader {
  summary: ChatSummary
  publication: string
  /** The source of the line the tool last used. */
  currentSource: string
  parentId: string | null
  parentStatus: 'none' | 'resolved' | 'unresolved'
  parent: ChatSummary | null
  folderState: FolderState | null
  nativeSessionId: string | null
  /** `null` until a command is confirmed for the tool's version. */
  resumeCommand: string | null
  physicalRecords: number
  /** Rows of the selected line. */
  visibleRecords: number
  /** Prompts of the selected line (the outline's length). */
  promptCount: number
  /** Records that could not be read as JSON or framed. */
  unparsedRecords: number
  coverage: { complete: boolean; reason: string | null }
  attachmentsNotKept: number
  viewToken: string
}

export interface TokenKinds {
  input: number
  cacheWrite: number
  cacheRead: number
  output: number
}

export interface PullRequest {
  repository: string | null
  number: number | null
  url: string | null
}

/** What a stretch of work did. Usage and money fields are `null` when nothing was recorded. */
export interface ChatFacts {
  messages: number
  turns: number
  calls: number
  failedCalls: number
  addedLines: number
  removedLines: number
  editedFileCount: number
  unknownEditCounts: number
  pullRequests: PullRequest[]
  compactions: number
  apiErrors: number
  tokens: TokenKinds | null
  reasoningTokens: number | null
  usageResponses: number
  ownershipPartial: boolean
  unpricedResponses: number
  recordedUsd?: number | null
  estimatedUsd?: number | null
  agentMs: number | null
  recordedTurns: number
  derivedTurns: number
  unknownDurationTurns: number
}

/** What the backup read from `.git` in the working folder. */
export interface ChatGit {
  topLevel: string | null
  commonDir: string | null
  branch: string | null
  /** Remote URLs. */
  remotes: string[] | null
}

export interface ChatInfo {
  publication: string
  /** This conversation's own work, abandoned branches included. */
  own: ChatFacts
  /** With its subagents. */
  total: ChatFacts
  currentLineTokens: TokenKinds | null
  subagentTokens: number | null
  usageCoverage: { known: number; conversations: number }
  models: { model: string; outputTokens: number | null }[]
  primaryModel: string | null
  timeSource: 'recorded' | 'derived' | 'mixed' | 'unknown' | (string & {})
  nativeSessionId: string | null
  launchOrigin: string | null
  toolVersion: string | null
  folder: string | null
  folderState: FolderState | null
  folderObservation: {
    path: string
    exists: boolean
    observedAt: number
  } | null
  git: ChatGit | null
  startCommit: string | null
  permissionMode: string | null
  effort: string | null
  agentType: string | null
  allErrorsRetried: boolean | null
  sourcePath: string | null
  sourceBytes: number
  sourceCount: number
  subagentSourceCount: number
  lastSavedAt: number | null
  physicalRecords: number
  visibleRecords: number
  hiddenRecords: number
  unparsedRecords: number
  editedFileCount: number
  resumeCommand: string | null
  /** The price table estimates came from; absent with estimates off. */
  priceTable?: string | null
}

export type ChatView = 'visible' | 'raw' | 'prompts'

export type ChatAnchor =
  | { kind: 'start' }
  | { kind: 'end' }
  | { kind: 'rank'; rank: number }
  | { kind: 'record'; sourceId: string; ordinal: number }
  | { kind: 'native'; sourceId: string; nativeId: string }

export interface ChatWindowQuery {
  id: string
  publication: string
  view?: ChatView
  /** A source other than the current line. */
  branch?: string
  /** A Pi leaf: the line that ends there. */
  leaf?: number
  anchor?: ChatAnchor
  /** Rows before an address anchor; it is centred by default. */
  before?: number
  /** 1–200. */
  limit?: number
  /** A fresh UUID, to cancel with `cancel_agent_read`. */
  requestId?: string
  showCost?: boolean
  estimateCost?: boolean
}

export interface ChatPart {
  kind: 'text' | 'thinking' | 'callInput' | 'callResult'
  text: string
}

export interface ChatEdit {
  files: string[]
  /** `null`: a write that replaced a file of unknown length. */
  addedLines: number | null
  removedLines: number | null
  diff: string | null
  truncated: boolean
}

export type ChatOperation =
  | {
      kind: 'call'
      name: string
      callId: string | null
      resultOrdinal?: number | null
      status?: 'ok' | 'error' | 'denied' | null
      durationMs?: number | null
      edit?: ChatEdit
    }
  | {
      kind: 'result'
      callId: string | null
      status: 'ok' | 'error' | 'denied' | null
      durationMs?: number | null
      agentId?: string
    }
  | { kind: 'thinking'; durationMs: number }

export interface ChatAttachment {
  sourceId: string
  ordinal: number
  key: string
  /** `inline`/`retained`: PathKeep has the bytes; `missing`: it does not (`reason`). */
  availability: 'inline' | 'retained' | 'missing' | (string & {})
  name?: string | null
  kind?: string | null
  mimeType?: string | null
  bytes?: number | null
  reason?: string | null
}

export interface ChatNotice {
  ordinal: number
  code: 'interrupted' | 'retry' | 'apiError' | (string & {})
  tone: 'info' | 'warning' | 'error' | (string & {})
  at: number | null
}

export interface ChatResponse {
  ordinal: number
  at: number | null
  tokens: TokenKinds | null
  reasoningTokens: number | null
  recordedUsd?: number | null
  estimatedUsd?: number | null
  durationMs: number | null
  model: string | null
}

export interface ChatTurnFooter {
  ordinal: number
  lastOrdinal: number
  footerOrdinal: number
  prompt: string | null
  startedAt: number | null
  endedAt: number | null
  endReason: 'completed' | 'interrupted' | 'nextPrompt' | 'open' | (string & {})
  durationSource: 'recorded' | 'derived' | 'unknown' | (string & {})
  model: string | null
  facts: ChatFacts
}

export interface ChatBoundary {
  kind: 'compaction' | 'context_edit' | 'unresolved_parent' | (string & {})
  tokens_before?: number | null
}

/** One row of the conversation as the reader shows it. */
export interface ChatRow {
  sourceId: string
  ordinal: number
  nativeId: string | null
  parentId: string | null
  /** The tool's own record type. */
  kind: string
  role: string | null
  at: number | null
  parts: ChatPart[]
  notices: ChatNotice[]
  bytes: number
  truncated: boolean
  boundary: ChatBoundary | null
  turnOrdinal: number | null
  turnNumber: number | null
  /** Versions of this prompt (rewritten and asked again). */
  versionCount: number | null
  footer: ChatTurnFooter | null
  response: ChatResponse | null
  operations: ChatOperation[]
  attachments: ChatAttachment[]
}

/** One row of the outline: a prompt. */
export interface ChatPromptRow {
  /** One-based. */
  turn: number
  sourceId: string
  ordinal: number
  visibleRank: number
  preview: string
}

/** One physical record, as the raw view lists it. */
export interface ChatRawRow {
  sourceId: string
  ordinal: number
  nativeId: string | null
  /** The tool's own record type. */
  kind: string
  role: string | null
  at: number | null
  bytes: number
  /** How the bytes framed: `Object` is a record; the rest say what was there instead. */
  coverage: string
  /** It feeds a row of the conversation, at `visibleRank`. */
  visible: boolean
  visibleRank: number | null
  /** A second copy of this earlier record. */
  mirrorOfOrdinal: number | null
}

export interface ChatWindow<Row> {
  publication: string
  first: number
  total: number
  previous: number | null
  next: number | null
  rows: Row[]
}

export interface ChatChildCard extends ChatSummary {
  /** Where in this conversation it was sent off, when the call says. */
  launch: { sourceId: string; ordinal: number; callId: string } | null
  agentType: string | null
  model: string | null
}

export interface ChatVersion {
  ordinal: number
  nativeId: string
  leaf: number
  preview: string | null
}

export interface ChatBranches {
  publication: string
  sources: unknown[]
  sourcesAfter: string | null
  children: ChatChildCard[]
  childrenAfter: string | null
  versions: { count: number; items: ChatVersion[]; next: number | null } | null
  leaves: { ordinals: number[]; total: number; latestRecorded: number | null }
  leafNext: number | null
}

export interface ChatBranchesQuery {
  id: string
  publication?: string
  sourceId?: string
  promptOrdinal?: number
  versionAfter?: number
  childrenAfter?: string
  limit?: number
  requestId?: string
}

export interface ChatBytes {
  bytes: number[]
  /** The next offset, `null` at the end. */
  next: number | null
  totalBytes?: number
}

export type ChatEditRequest =
  | { action: 'star' | 'archive'; id: string; on: boolean }
  | { action: 'trash' | 'restore'; id: string }
  | { action: 'rename'; id: string; title: string }

/**
 * A conversation's marks changed (`id`), or a backup published
 * conversations (`id: null`, with its `runRef`).
 */
export const AGENT_CONVERSATIONS_CHANGED_EVENT =
  'pathkeep://agent-conversations-changed'
export interface AgentConversationsChangedEvent {
  id: string | null
  viewToken: string
  runRef?: string
}

export const agentChatsClient = {
  list: (request: ChatListQuery) =>
    call<ChatListPage>('list_agent_conversations', { request }),
  counts: (request: ChatListQuery) =>
    call<ChatCounts>('get_agent_conversation_counts', { request }),
  header: (id: string) =>
    call<ChatHeader>('get_agent_conversation', { request: { id } }),
  /**
   * `costs` are the saved cost switches; the backend applies the saved
   * ones as well, so an absent switch never shows what they turned off.
   */
  info: (
    id: string,
    costs: { showCost?: boolean; estimateCost?: boolean } = {},
  ) =>
    call<ChatInfo>('get_agent_conversation_info', {
      request: { id, ...costs },
    }),
  window: <Row = ChatRow>(request: ChatWindowQuery) =>
    call<ChatWindow<Row>>('read_agent_conversation_window', { request }),
  branches: (request: ChatBranchesQuery) =>
    call<ChatBranches>('list_agent_conversation_branches', { request }),
  /** One page of a record's exact bytes; concatenate before decoding. */
  record: (request: {
    id: string
    publication: string
    sourceId: string
    ordinal: number
    offset: number
    limit: number
  }) => call<ChatBytes>('read_agent_record', { request }),
  attachment: (request: {
    id: string
    publication: string
    sourceId: string
    ordinal: number
    key: string
    offset: number
    limit: number
  }) => call<ChatBytes>('read_agent_attachment', { request }),
  /** The committed header after the change. */
  edit: (request: ChatEditRequest) =>
    call<ChatHeader>('edit_agent_conversation', { request }),
  cancel: (requestIdOrStreamId: string) =>
    call<{ state: string }>('cancel_agent_read', {
      request: { requestIdOrStreamId },
    }),
}
