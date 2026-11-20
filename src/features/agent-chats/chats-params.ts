/**
 * The list's state in the URL (`agent-history.md` §3.1): project, tools,
 * range or day, "only" flags, sort, group and the open conversation.
 * Defaults are left out of the URL, so a plain `#/agent/chats` is the
 * default list, and a link from anywhere (Insights' calendar, ⌘K) opens
 * exactly what it names.
 *
 * Responsibilities: reading the parameters (an unknown value reads as the
 * default), writing a change (replacing the history entry, or adding one
 * for "only this project"), and the rules that tie them together: a range
 * drops the day and a day drops the range; a metric sort cannot be grouped
 * by date. Not responsible for: what the backend does with them.
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

/** Sorts that put a number on each row, and cannot be grouped by date. */
export function isMetricSort(sort: ChatSort) {
  return sort === 'agentTime' || sort === 'tokens' || sort === 'turns'
}

export interface ChatsParams {
  project: string | null
  harnesses: AgentToolId[]
  range: ChatRange
  /** `YYYY-MM-DD`; set only when `range` is `all`. */
  day: string | null
  flags: ChatFlag[]
  sort: ChatSort
  group: ChatGroup
  /** The conversation open in the reader. */
  chat: string | null
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

export function parseChatsParams(params: URLSearchParams): ChatsParams {
  const day = params.get('day')
  const range = oneOf(params.get('range'), RANGES, 'all')
  const sort = oneOf(params.get('sort'), SORTS, 'lastActivity')
  const group = oneOf(params.get('group'), GROUPS, 'day')
  return {
    project: params.get('project') || null,
    harnesses: listOf(params.get('h'), AGENT_TOOLS),
    range,
    day: range === 'all' && isDateKey(day) ? day : null,
    flags: listOf(params.get('flags'), FLAGS),
    sort,
    group: group === 'day' && isMetricSort(sort) ? 'none' : group,
    chat: params.get('chat') || null,
  }
}

function write(params: URLSearchParams, next: ChatsParams) {
  const set = (key: string, value: string | null, fallback = '') => {
    if (value === null || value === fallback) params.delete(key)
    else params.set(key, value)
  }
  set('project', next.project)
  set('h', next.harnesses.join(','))
  set('range', next.range, 'all')
  set('day', next.day)
  set('flags', next.flags.join(','))
  set('sort', next.sort, 'lastActivity')
  set('group', next.group, 'day')
  set('chat', next.chat)
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
 * history entry unless `push` (only this project) asks for a new one, so
 * Back returns to the list before it.
 */
export function useChatsParams() {
  const [search, setSearch] = useSearchParams()
  const params = useMemo(() => parseChatsParams(search), [search])
  const update = useCallback(
    (change: ChatsChange, options: { push?: boolean } = {}) => {
      const current = currentSearch()
      const next = applyChange(parseChatsParams(current), change)
      setSearch(write(current, next), { replace: !options.push })
    },
    [setSearch],
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

/** How many conditions narrow the list: sort and group are not conditions. */
export function conditionCount(params: ChatsParams) {
  return (
    (params.project ? 1 : 0) +
    params.harnesses.length +
    (params.range !== 'all' ? 1 : 0) +
    (params.day ? 1 : 0) +
    params.flags.length
  )
}

/** A link to the list showing one project (the reader's project chip). */
export function projectListSearch(project: string) {
  return `?${new URLSearchParams({ project }).toString()}`
}
