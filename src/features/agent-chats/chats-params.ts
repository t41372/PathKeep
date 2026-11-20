/**
 * The list's state in the URL (`agent-history.md` §3.1): the search, how
 * it matches (words or meaning) and what it searches, project, tools,
 * range or day, "only" flags, sort,
 * group, the open conversation, and the reader's find and record.
 * Defaults are left out of the URL, so a plain `#/agent/chats` is the
 * default list, and a link from anywhere (Insights' calendar, ⌘K) opens
 * exactly what it names.
 *
 * Responsibilities: reading the parameters (an unknown value reads as the
 * default), writing a change (replacing the history entry, or adding one
 * for "only this project"), and the rules that tie them together: a range
 * drops the day and a day drops the range; a metric sort cannot be grouped
 * by date. On a project's page (`#/agent/projects/:id`) the project is
 * fixed: it is not in the URL, not a condition, and not cleared.
 * Not responsible for: what the backend does with them.
 */
import { useCallback, useMemo } from 'react'
import { useSearchParams } from 'react-router-dom'
import { AGENT_TOOLS, type AgentToolId } from '@/lib/backend-client/data-kinds'
import type {
  ChatFlag,
  ChatGroup,
  ChatListQuery,
  ChatRange,
  ChatSort,
} from '@/lib/backend-client/agent-chats'
import {
  SEARCH_ROLES,
  type SearchRequest,
  type SearchRole,
  type SemanticSearchRequest,
} from '@/lib/backend-client/agent-search'
import { isDateKey } from '@/lib/time-zone'

export const RANGES: readonly ChatRange[] = [
  'all',
  'today',
  '7d',
  '30d',
  '90d',
  '1y',
]
export const FLAGS: readonly ChatFlag[] = [
  'starred',
  'onlyInPathKeep',
  'hasPr',
  'hasSubagents',
  'hasEdits',
  'noUsage',
  'archived',
  'trash',
]
export const SORTS: readonly ChatSort[] = [
  'lastActivity',
  'started',
  'agentTime',
  'tokens',
  'turns',
]
export const GROUPS: readonly ChatGroup[] = ['day', 'project', 'none']

/**
 * How a search matches: the words as written (`text`: words, a "phrase"
 * or a /regular expression/), or passages ranked by meaning from the
 * semantic index (`semantic`, `mode=semantic` in the URL).
 */
export type ChatSearchMode = 'text' | 'semantic'
export const SEARCH_MODES: readonly ChatSearchMode[] = ['text', 'semantic']

/** Sorts that put a number on each row, and cannot be grouped by date. */
export function isMetricSort(sort: ChatSort) {
  return sort === 'agentTime' || sort === 'tokens' || sort === 'turns'
}

/** One record of a conversation: where the reader opens at a match. */
export interface RecordAt {
  sourceId: string
  ordinal: number
}

export interface ChatsParams {
  /** The search; `null` shows the list. */
  q: string | null
  /** Kept without a search, so the next one matches the same way. */
  mode: ChatSearchMode
  /** What a search reads besides titles; all four by default. */
  roles: SearchRole[]
  project: string | null
  /** The project is the page's own and cannot be changed here. */
  fixedProject: boolean
  harnesses: AgentToolId[]
  range: ChatRange
  /** `YYYY-MM-DD`; set only when `range` is `all`. */
  day: string | null
  flags: ChatFlag[]
  sort: ChatSort
  group: ChatGroup
  /** The conversation open in the reader. */
  chat: string | null
  /** What the reader's find bar finds, opened from a search. */
  find: string | null
  /** The record the reader opens at. */
  at: RecordAt | null
}

/** `at` in the URL: the ordinal first, so the source id can hold anything. */
export function writeAt(at: RecordAt) {
  return `${at.ordinal}:${at.sourceId}`
}

export function readAt(value: string | null): RecordAt | null {
  const colon = value?.indexOf(':') ?? -1
  if (!value || colon < 1) return null
  const ordinal = Number(value.slice(0, colon))
  const sourceId = value.slice(colon + 1)
  return Number.isInteger(ordinal) && ordinal >= 0 && sourceId
    ? { sourceId, ordinal }
    : null
}

const listOf = <T extends string>(
  value: string | null,
  allowed: readonly T[],
): T[] => {
  const wanted = new Set((value ?? '').split(','))
  return allowed.filter((item) => wanted.has(item))
}

const oneOf = <T extends string>(
  value: string | null,
  allowed: readonly T[],
  fallback: T,
): T =>
  (allowed as readonly string[]).includes(value ?? '') ? (value as T) : fallback

export function parseChatsParams(
  params: URLSearchParams,
  fixedProject: string | null = null,
): ChatsParams {
  const day = params.get('day')
  const range = oneOf(params.get('range'), RANGES, 'all')
  const sort = oneOf(params.get('sort'), SORTS, 'lastActivity')
  const parsedGroup = oneOf(params.get('group'), GROUPS, 'day')
  // One project's page has no project groups.
  const group = fixedProject && parsedGroup === 'project' ? 'day' : parsedGroup
  const roles = params.has('roles')
    ? listOf(params.get('roles'), SEARCH_ROLES)
    : [...SEARCH_ROLES]
  return {
    q: params.get('q')?.trim() ? params.get('q') : null,
    mode: oneOf(params.get('mode'), SEARCH_MODES, 'text'),
    roles: roles.length > 0 ? roles : [...SEARCH_ROLES],
    project: fixedProject ?? (params.get('project') || null),
    fixedProject: fixedProject !== null,
    harnesses: listOf(params.get('h'), AGENT_TOOLS),
    range,
    day: range === 'all' && isDateKey(day) ? day : null,
    flags: listOf(params.get('flags'), FLAGS),
    sort,
    group: group === 'day' && isMetricSort(sort) ? 'none' : group,
    chat: params.get('chat') || null,
    find: params.get('find')?.trim() ? params.get('find') : null,
    at: readAt(params.get('at')),
  }
}

function write(params: URLSearchParams, next: ChatsParams) {
  const set = (key: string, value: string | null, fallback = '') => {
    if (value === null || value === fallback) params.delete(key)
    else params.set(key, value)
  }
  set('q', next.q)
  set('mode', next.mode, 'text')
  set(
    'roles',
    next.roles.length === SEARCH_ROLES.length
      ? null
      : SEARCH_ROLES.filter((role) => next.roles.includes(role)).join(','),
  )
  if (!next.fixedProject) set('project', next.project)
  set('h', next.harnesses.join(','))
  set('range', next.range, 'all')
  set('day', next.day)
  set('flags', next.flags.join(','))
  set('sort', next.sort, 'lastActivity')
  set('group', next.group, 'day')
  set('chat', next.chat)
  set('find', next.find)
  set('at', next.at ? writeAt(next.at) : null)
  return params
}

/** The hash's search part as it is now (see `src/lib/search-params.ts`). */
function currentSearch() {
  const hash = window.location.hash
  const query = hash.indexOf('?')
  return new URLSearchParams(query < 0 ? '' : hash.slice(query + 1))
}

/** A change to apply over the current parameters. */
export type ChatsChange = Partial<ChatsParams>

/** Applies `change` with the rules that tie the parameters together. */
export function applyChange(
  now: ChatsParams,
  change: ChatsChange,
): ChatsParams {
  const next = { ...now, ...change }
  if (now.fixedProject) {
    next.project = now.project
    if (next.group === 'project') next.group = 'day'
  }
  if (change.range !== undefined && change.range !== 'all') next.day = null
  if (change.day) next.range = 'all'
  if (
    change.sort !== undefined &&
    isMetricSort(next.sort) &&
    next.group === 'day'
  )
    next.group = 'none'
  // Date groups need a time order: choosing them goes back to last activity.
  if (change.group === 'day' && isMetricSort(next.sort))
    next.sort = 'lastActivity'
  return next
}

/**
 * The parameters, and `update(change, { push })`: a change replaces the
 * history entry unless `push` (a search sent, only this project) asks for
 * a new one, so Back returns to the list before it. On a project's page,
 * pass its id as `fixedProject`.
 */
export function useChatsParams(fixedProject: string | null = null) {
  const [search, setSearch] = useSearchParams()
  const params = useMemo(
    () => parseChatsParams(search, fixedProject),
    [search, fixedProject],
  )
  const update = useCallback(
    (change: ChatsChange, options: { push?: boolean } = {}) => {
      const current = currentSearch()
      const next = applyChange(parseChatsParams(current, fixedProject), change)
      setSearch(write(current, next), { replace: !options.push })
    },
    [setSearch, fixedProject],
  )
  return [params, update] as const
}

/** The list request for these parameters (no cursor), in `timeZone`. */
export function listQuery(
  params: ChatsParams,
  timeZone: string | undefined,
): Omit<ChatListQuery, 'cursor' | 'limit'> {
  return {
    harnesses: params.harnesses,
    ...(params.project ? { projectId: params.project } : {}),
    flags: params.flags,
    sort: params.sort,
    group: params.group,
    ...(params.day ? { day: params.day } : { range: params.range }),
    timeZone,
  }
}

/**
 * The search for these parameters: the list's filters, never its sort or
 * groups (results come in last-activity order).
 */
export function searchRequest(
  params: ChatsParams,
  timeZone: string | undefined,
): SearchRequest | null {
  if (!params.q) return null
  return {
    query: params.q,
    roles: params.roles,
    filters: {
      harnesses: params.harnesses,
      ...(params.project ? { projectId: params.project } : {}),
      flags: params.flags,
      ...(params.day ? { day: params.day } : { range: params.range }),
      timeZone,
    },
  }
}

/**
 * The same search by meaning: the same text, kinds of text and filters,
 * passages instead of cards.
 */
export function semanticRequest(
  params: ChatsParams,
  timeZone: string | undefined,
): SemanticSearchRequest | null {
  const words = searchRequest(params, timeZone)
  return words && { mode: 'semantic', ...words }
}

/**
 * How many conditions narrow the list: sort and group are not conditions,
 * and neither is a project's own page.
 */
export function conditionCount(params: ChatsParams) {
  return (
    (params.project && !params.fixedProject ? 1 : 0) +
    params.harnesses.length +
    (params.range !== 'all' ? 1 : 0) +
    (params.day ? 1 : 0) +
    params.flags.length
  )
}

/** A link to History showing one project. */
export function projectListSearch(project: string) {
  return `?${new URLSearchParams({ project }).toString()}`
}

/** A link to History with this search. */
export function searchListSearch(q: string) {
  return `?${new URLSearchParams({ q }).toString()}`
}
