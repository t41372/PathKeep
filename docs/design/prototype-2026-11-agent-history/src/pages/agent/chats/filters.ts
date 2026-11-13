/** The chats page's filter state, kept in the URL so back/forward and links work. */
import { useCallback, useMemo } from 'react'
import { useSearchParams } from 'react-router-dom'
import { archive } from '~/data/archive'
import { DAY, NOW, totalTokens } from '~/data/fixture'
import type { Role } from '~/data/transcript'
import type { Chat, HarnessId } from '~/data/types'
import type { useChatEdits } from './chat-edits'

export type Range = 'all' | 'today' | '7d' | '30d' | '90d' | '1y'
export type Sort = 'recent' | 'started' | 'agent' | 'tokens' | 'turns'
export type Group = 'day' | 'project' | 'none'
export type Flag = 'starred' | 'gone' | 'pr' | 'subagents' | 'edits' | 'nousage' | 'archived' | 'trash'

export const rangeLabels: Record<Range, string> = {
  all: '全部時間',
  today: '今天',
  '7d': '最近 7 天',
  '30d': '最近 30 天',
  '90d': '最近 90 天',
  '1y': '最近一年',
}
export const sortLabels: Record<Sort, string> = {
  recent: '最近活動',
  started: '開始時間',
  agent: 'Agent 工作時間',
  tokens: 'Token 用量',
  turns: '來回輪數',
}
export const groupLabels: Record<Group, string> = { day: '按日期', project: '按專案', none: '不分組' }
export const flagLabels: Record<Flag, string> = {
  starred: '已加星',
  gone: '只剩 PathKeep 有',
  pr: '有開 PR',
  subagents: '用了 subagent',
  edits: '有改檔案',
  nousage: '沒有用量紀錄',
  archived: '已封存',
  trash: '垃圾桶',
}

const rangeMs: Record<Range, number> = { all: Infinity, today: DAY, '7d': 7 * DAY, '30d': 30 * DAY, '90d': 90 * DAY, '1y': 365 * DAY }
const allRoles: Role[] = ['user', 'agent', 'tool', 'thinking']

export interface Filters {
  q: string
  regex: boolean
  roles: Role[]
  project?: string
  harnesses: HarnessId[]
  range: Range
  flags: Flag[]
  sort: Sort
  group: Group
  chat?: number
  find?: string
  at?: number
}

export function useFilters() {
  const [params, setParams] = useSearchParams()
  const filters = useMemo<Filters>(
    () => ({
      q: params.get('q') ?? '',
      regex: params.get('regex') === '1',
      roles: (params.get('roles')?.split(',').filter(Boolean) as Role[] | undefined) ?? allRoles,
      project: params.get('project') ?? undefined,
      harnesses: (params.get('h')?.split(',').filter(Boolean) as HarnessId[] | undefined) ?? [],
      range: (params.get('range') as Range | null) ?? 'all',
      flags: (params.get('flags')?.split(',').filter(Boolean) as Flag[] | undefined) ?? [],
      sort: (params.get('sort') as Sort | null) ?? 'recent',
      group: (params.get('group') as Group | null) ?? 'day',
      chat: params.get('chat') ? Number(params.get('chat')) : undefined,
      find: params.get('find') ?? undefined,
      at: params.get('at') ? Number(params.get('at')) : undefined,
    }),
    [params],
  )
  const update = useCallback(
    (patch: Partial<Record<keyof Filters, string | number | boolean | string[] | undefined>>, replace = true) => {
      setParams(
        (prev) => {
          const next = new URLSearchParams(prev)
          for (const [key, value] of Object.entries(patch)) {
            const name = key === 'harnesses' ? 'h' : key
            if (value === undefined || value === '' || value === false || (Array.isArray(value) && value.length === 0)) next.delete(name)
            else if (Array.isArray(value)) next.set(name, value.join(','))
            else next.set(name, value === true ? '1' : String(value))
          }
          if (next.get('roles') === allRoles.join(',')) next.delete('roles')
          if (next.get('range') === 'all') next.delete('range')
          if (next.get('sort') === 'recent') next.delete('sort')
          if (next.get('group') === 'day') next.delete('group')
          return next
        },
        { replace },
      )
    },
    [setParams],
  )
  return { filters, update }
}

/** The user's put-away and trashed conversations, as `applyFilters` needs them. */
export type ChatEdits = Pick<ReturnType<typeof useChatEdits>, 'archived' | 'trashed'>

/**
 * Put-away conversations are listed only when 已封存 is picked, and then only
 * those. Trashed ones are listed only when 垃圾桶 is picked, and then only
 * those, put away or not. `forSearch` is for search, which looks through
 * put-away conversations too but never through the trash.
 */
export function applyFilters(f: Filters, edits: ChatEdits, forSearch = false): Chat[] {
  const since = NOW - rangeMs[f.range]
  const onlyArchived = f.flags.includes('archived')
  const onlyTrash = f.flags.includes('trash')
  if (forSearch && onlyTrash) return []
  const list = archive.chats.filter(
    (c) =>
      (onlyTrash ? edits.trashed.has(c.id) : !edits.trashed.has(c.id)) &&
      (onlyArchived ? edits.archived.has(c.id) : onlyTrash || forSearch || !edits.archived.has(c.id)) &&
      c.ended >= since &&
      (!f.project || c.projectId === f.project) &&
      (f.harnesses.length === 0 || f.harnesses.includes(c.harness)) &&
      f.flags.every((flag) =>
        flag === 'starred'
          ? c.starred
          : flag === 'gone'
            ? c.source === 'gone'
            : flag === 'pr'
              ? !!c.pr
              : flag === 'subagents'
                ? c.subagentCount > 0
                : flag === 'edits'
                  ? c.linesAdded + c.linesRemoved > 0
                  : flag === 'archived' || flag === 'trash'
                    ? true
                    : !c.tokens,
      ),
  )
  const key: Record<Sort, (c: Chat) => number> = {
    recent: (c) => c.ended,
    started: (c) => c.started,
    agent: (c) => c.agentMs,
    tokens: (c) => (c.tokens ? totalTokens(c.tokens) : -1),
    turns: (c) => c.turns,
  }
  const by = key[f.sort]
  return f.sort === 'recent' ? list : [...list].sort((a, b) => by(b) - by(a))
}
