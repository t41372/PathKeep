/**
 * Typed client for the Agent space's projects: every project with its
 * totals, the pinned arrangement (sections and pins), the folder groups,
 * one project with its folders, the folder suggestions, and the edits a
 * person makes (rename, join, detach, answer, sections, pins, undo).
 *
 * Responsible for: command names, request envelopes, the wire shapes and
 * the change event's name. Not responsible for: caching, the version a
 * write needs, or any copy (`src/lib/queries/agent-projects.ts`).
 *
 * Source of truth: `docs/architecture/desktop-command-surface.md` (Agent
 * Projects, Project edits). Times are epoch milliseconds; ids, cursors
 * and view tokens are opaque. Every edit names the `version` it was made
 * against; a newer one answers `stale_cursor`.
 */
import type { AgentToolId } from './data-kinds'
import { call } from './shared'

export type ProjectKind = 'repo' | 'folder' | 'scratch'

/** One project as every view lists it. */
export interface ProjectCard {
  id: string
  /** The name shown: the person's, else the main folder's; empty for the loose folders. */
  displayName: string
  defaultName: string
  kind: ProjectKind
  /** `null` for the loose folders. */
  mainFolder: string | null
  parentFolder: string | null
  folderPaths: string[]
  /** `null` when no backup has looked. */
  mainExists: boolean | null
  folderCount: number
  /** Every folder is gone from the disk. */
  gone: boolean
  /** Another project has the same name: show the full main path. */
  nameIsShared: boolean
  remotes: string[]
  harnesses: AgentToolId[]
  conversationCount: number
  subagentCount: number
  summedAgentMs: number | null
  tokensTotal: number | null
  tokensKnownConversations: number
  firstActivity: number | null
  lastActivity: number | null
  latestConversationTitle: string | null
  hasAddedFolders: boolean
}

/** A pin or a section, in the order the person arranged them. */
export interface ArrangementNode {
  kind: 'section' | 'project'
  id: string
  /** The enclosing section; `null` at the top level. */
  parent: string | null
  /** Sections only. */
  name: string | null
  collapsed: boolean
  projectCount: number
  conversationCount: number
  lastActivity: number | null
}

/** A group of the folder view: a parent folder (its path is the id) or the loose folders. */
export interface FolderGroup {
  id: string
  parent: string | null
  kind: 'folder' | 'scratch'
  projectIds: string[]
  projectCount: number
  conversationCount: number
  lastActivity: number | null
}

export interface ProjectList {
  version: number
  viewToken: string
  /** Every visible project. */
  total: number
  /** After this request's filters. */
  matching: number
  items: ProjectCard[]
  nextCursor: string | null
  arrangement: ArrangementNode[]
  pendingSuggestions: number
  homeFolder: string | null
  tree: { topLevelProjects: string[]; groups: FolderGroup[] } | null
}

export interface ProjectQuery {
  /** Name, any folder path or remote, any case. */
  text?: string
  harnesses?: AgentToolId[]
  /** `recent`: today and the 29 days before, in `timeZone`. */
  status?: 'all' | 'recent' | 'gone'
  timeZone?: string
  /** Only projects not pinned. */
  otherOnly?: boolean
  /** Projects whose folders can be added to this one. */
  candidateFor?: string
  /** 1–500; omitted: every match. */
  limit?: number
  cursor?: string
  folderTree?: boolean
}

export type FolderRole =
  'main' | 'worktree' | 'subfolder' | 'temp' | 'userAdded'

export interface ProjectFolder {
  path: string
  role: FolderRole
  reason:
    | 'repository'
    | 'folder'
    | 'commonGitDirectory'
    | 'insideRepository'
    | 'home'
    | 'temporary'
    | 'userAssigned'
    | 'userDetached'
  ruleOwner: 'pathkeep' | 'user'
  exists: boolean | null
  branch: string | null
  remotes: string[]
  harnesses: AgentToolId[]
  conversationCount: number
  originalProjectId: string
  autoProjectId: string
  addedFromProject: {
    id: string
    displayName: string
    defaultName: string
  } | null
}

export type ProjectDetail =
  | {
      status: 'ok'
      project: ProjectCard
      folders: ProjectFolder[]
      version: number
      viewToken: string
    }
  /** Its folders were added to another project (`into`). */
  | { status: 'addedInto'; into: string; version: number }
  | { status: 'notFound'; version: number }

export type SuggestionDecision = 'pending' | 'later' | 'declined' | 'added'

export interface ProjectSuggestion {
  id: string
  kind: 'sameRemote' | 'moved' | 'toolWorktree'
  fromFolder: string
  fromProjectId: string
  intoProjectId: string
  intoDisplayName: string
  fromConversationCount: number
  fromFolderGone: boolean
  evidence: {
    remote?: string
    oldPath?: string
    newPath?: string | null
    harness?: AgentToolId
    folderName?: string
    branch?: string | null
  }
  decision: SuggestionDecision
}

export type ProjectAction =
  | { kind: 'rename'; id: string; name: string }
  | { kind: 'join'; from: string; into: string }
  | { kind: 'detach'; id: string; path: string }
  | { kind: 'answer'; id: string; decision: SuggestionDecision }
  | { kind: 'createSection'; parent: string | null; name: string }
  | { kind: 'renameSection'; id: string; name: string }
  | { kind: 'collapse'; id?: string; collapsed: boolean }
  | { kind: 'move'; id: string; parent: string | null; index?: number }
  | { kind: 'unpin'; id: string }
  | { kind: 'deleteSection'; id: string }
  | { kind: 'undo' }

export interface ProjectEditResult {
  /** The version after this edit; Undo names it. */
  version: number
  /** A new section's id. */
  createdId: string | null
  /** The conversations' view token, as their change event carries it. */
  viewToken: string
}

/** A project edit landed: `{ version }`. */
export const AGENT_PROJECTS_CHANGED_EVENT = 'pathkeep://agent-projects-changed'

export const agentProjectsClient = {
  list: (request: ProjectQuery) =>
    call<ProjectList>('list_agent_projects', { request }),
  get: (id: string) =>
    call<ProjectDetail>('get_agent_project', { request: { id } }),
  suggestions: (intoProjectId?: string) =>
    call<{ version: number; items: ProjectSuggestion[] }>(
      'list_agent_project_suggestions',
      { request: intoProjectId ? { intoProjectId } : {} },
    ),
  edit: (version: number, action: ProjectAction) =>
    call<ProjectEditResult>('edit_agent_project', {
      request: { version, action },
    }),
}
