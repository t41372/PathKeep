/**
 * The shapes the prototype draws. They are what the product's read model
 * would have to provide, so they double as a list of what the core crate
 * still lacks (conversation grouping, projects, usage, durations, git).
 */

export type HarnessId = 'claude' | 'codex' | 'pi' | 'deepseek'

/** Token counts as the harness recorded them. `null` means not recorded. */
export interface Tokens {
  input: number
  cacheWrite: number
  cacheRead: number
  output: number
  /** Codex and Pi report reasoning separately; Claude folds it into output. */
  reasoning: number
}

export type ProjectKind = 'repo' | 'folder' | 'scratch'

export interface Location {
  path: string
  role: 'main' | 'worktree' | 'subfolder' | 'former' | 'temp'
  /** The folder is still on disk. Known only from the last backup. */
  exists: boolean
  branch?: string
  chatCount: number
}

export interface Project {
  id: string
  name: string
  kind: ProjectKind
  /** Remote URL when the folder is a git repo with one. */
  remote?: string
  /** Parent folder shown in the folder tree, e.g. `~/code`. */
  parent: string
  locations: Location[]
  pinned: boolean
  /** Every location is gone from disk. */
  gone: boolean
  /** Filled by aggregation. */
  chatIds: number[]
  harnesses: HarnessId[]
  firstActive: number
  lastActive: number
  agentMs: number
  tokens: Tokens
  linesAdded: number
  linesRemoved: number
  /** Chats per week, oldest first, last 26 weeks. */
  weekly: number[]
}

export interface Subagent {
  id: string
  /** What the parent asked for, e.g. "Explore: find every caller of open_archive". */
  title: string
  agentType: string
  model: string
  agentMs: number
  tokens: Tokens
  events: number
}

export type TitleSource = 'custom' | 'ai' | 'summary' | 'first-prompt'
export type SourceState = 'present' | 'gone'
export type Entrypoint = 'cli' | 'vscode' | 'desktop' | 'sdk' | 'app'

export interface Chat {
  id: number
  harness: HarnessId
  projectId: string
  /** The folder the agent worked in (the last cwd). */
  cwd: string
  title: string
  titleSource: TitleSource
  firstPrompt: string
  started: number
  ended: number
  /** Time the agent spent working on turns (prompt → turn end). */
  agentMs: number
  /** `recorded` when the harness wrote turn durations, `derived` from timestamps. */
  agentMsSource: 'recorded' | 'derived'
  turns: number
  toolCalls: number
  subagentCount: number
  /** Null when the harness wrote no usage for this conversation. */
  tokens: Tokens | null
  models: string[]
  branch?: string
  commit?: string
  pr?: { number: number; repo: string }
  linesAdded: number
  linesRemoved: number
  filesTouched: number
  entrypoint: Entrypoint
  version: string
  effort?: string
  permissionMode?: string
  compactions: number
  apiErrors: number
  starred: boolean
  source: SourceState
  sizeBytes: number
  events: number
  /** Cost the harness itself recorded (Claude Code `cost-state`, Pi usage.cost). */
  recordedCostUsd?: number
}

export interface MergeSuggestion {
  id: string
  kind: 'same-remote' | 'moved' | 'worktree' | 'subfolder'
  /** Project that would absorb the other. */
  intoId: string
  fromId: string
  evidence: string
}
