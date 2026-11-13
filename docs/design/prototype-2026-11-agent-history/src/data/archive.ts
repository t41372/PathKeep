/** The archive the prototype reads, built once, plus lookups every screen shares. */
import { buildArchive, DAY, NOW, totalTokens, zeroTokens, addTokens } from './fixture'
import { localDayKey } from './format'
import { lognormal, mulberry32, pick } from './rng'
import { codexSubagentTasks, subagentTasks } from './text-bank'
import type { Chat, HarnessId, Project, Subagent, Tokens } from './types'
import claudeIcon from '~/assets/harness-icons/claude-code.svg'
import codexIcon from '~/assets/harness-icons/codex.svg'
import codexDarkIcon from '~/assets/harness-icons/codex-dark.svg'
import deepseekIcon from '~/assets/harness-icons/deepseek.png'
import piIcon from '~/assets/harness-icons/pi.svg'

export const archive = buildArchive()
export { NOW }

export interface HarnessMeta {
  id: HarnessId
  name: string
  /** The harness's own icon (see assets/harness-icons/README.md); `iconDark` when it needs one on dark. */
  icon: string
  iconDark?: string
  root: string
  vendor: string
}

export const harnesses: Record<HarnessId, HarnessMeta> = {
  claude: { id: 'claude', name: 'Claude Code', icon: claudeIcon, root: '~/.claude/projects', vendor: 'Anthropic' },
  codex: { id: 'codex', name: 'Codex', icon: codexIcon, iconDark: codexDarkIcon, root: '~/.codex/sessions', vendor: 'OpenAI' },
  pi: { id: 'pi', name: 'Pi', icon: piIcon, root: '~/.pi/agent/sessions', vendor: 'Earendil' },
  deepseek: { id: 'deepseek', name: 'DeepSeek Harness', icon: deepseekIcon, root: '~/.dsh/sessions', vendor: 'DeepSeek' },
}

export const harnessOrder: HarnessId[] = ['claude', 'codex', 'pi', 'deepseek']

/** Series colours for charts split by harness. Fixed, so they don't follow the space's accent. */
export const harnessColor: Record<HarnessId, string> = {
  claude: 'var(--harness-claude)',
  codex: 'var(--harness-codex)',
  pi: 'var(--harness-pi)',
  deepseek: 'var(--harness-deepseek)',
}

export const projectOf = (chat: Chat): Project => archive.projectById.get(chat.projectId)!

export function sessionId(chat: Chat) {
  const rng = mulberry32(chat.id * 7919)
  const h = (n: number) => Array.from({ length: n }, () => Math.floor(rng() * 16).toString(16)).join('')
  return `${h(8)}-${h(4)}-${h(4)}-${h(4)}-${h(12)}`
}

export function resumeCommand(chat: Chat) {
  const id = sessionId(chat)
  switch (chat.harness) {
    case 'claude':
      return `cd ${chat.cwd} && claude --resume ${id}`
    case 'codex':
      return `cd ${chat.cwd} && codex resume ${id}`
    case 'pi':
      return `cd ${chat.cwd} && pi --session ${id}`
    default:
      return `cd ${chat.cwd} && dsh resume ${id}`
  }
}

export function sourcePath(chat: Chat) {
  const id = sessionId(chat)
  if (chat.harness === 'claude') return `~/.claude/projects/${chat.cwd.replace(/[~/.]/g, '-')}/${id}.jsonl`
  if (chat.harness === 'codex') {
    const d = new Date(chat.started).toISOString().slice(0, 10).split('-').join('/')
    return `~/.codex/sessions/${d}/rollout-${id}.jsonl.zst`
  }
  if (chat.harness === 'pi') return `~/.pi/agent/sessions/${id}.jsonl`
  return `~/.dsh/sessions/${id}/events.jsonl`
}

const subCache = new Map<number, Subagent[]>()

export function subagentsOf(chat: Chat): Subagent[] {
  const hit = subCache.get(chat.id)
  if (hit) return hit
  const rng = mulberry32(chat.id * 104729)
  const bank = chat.harness === 'codex' ? codexSubagentTasks : subagentTasks
  const share = chat.tokens ? 0.05 + chat.subagentCount * 0.02 : 0
  const list: Subagent[] = Array.from({ length: chat.subagentCount }, (_, i) => {
    const [agentType, title] = pick(rng, bank)
    const tokens = zeroTokens()
    if (chat.tokens) {
      const part = (Math.min(0.6, share) / chat.subagentCount) * (0.5 + rng())
      tokens.input = Math.round(chat.tokens.input * part)
      tokens.cacheWrite = Math.round(chat.tokens.cacheWrite * part)
      tokens.cacheRead = Math.round(chat.tokens.cacheRead * part)
      tokens.output = Math.round(chat.tokens.output * part)
      tokens.reasoning = Math.round(chat.tokens.reasoning * part)
    }
    return {
      id: `${chat.id}-s${i + 1}`,
      title,
      agentType,
      model: chat.models[1] ?? chat.models[0],
      agentMs: Math.round(lognormal(rng, 70_000, 0.9)),
      tokens,
      events: Math.round(lognormal(rng, 40, 0.8)),
    }
  })
  subCache.set(chat.id, list)
  return list
}

export function sumTokens(chats: Iterable<Chat>): { tokens: Tokens; known: number; total: number } {
  const tokens = zeroTokens()
  let known = 0
  let total = 0
  for (const chat of chats) {
    total++
    if (chat.tokens) {
      known++
      addTokens(tokens, chat.tokens)
    }
  }
  return { tokens, known, total }
}

export { totalTokens }

/**
 * Time ranges in which the agent was working on a turn. The fixture keeps
 * only per-chat totals, so turns are spread evenly over the chat's span.
 */
function turnRanges(chat: Chat): [number, number][] {
  const span = Math.max(chat.ended - chat.started, chat.agentMs)
  const each = chat.agentMs / chat.turns
  const gap = (span - chat.agentMs) / Math.max(1, chat.turns)
  return Array.from({ length: chat.turns }, (_, i) => {
    const start = chat.started + i * (each + gap)
    return [start, start + each] as [number, number]
  })
}

function unionLength(ranges: [number, number][]): number {
  ranges.sort((a, b) => a[0] - b[0])
  let total = 0
  let [s, e] = ranges[0] ?? [0, 0]
  for (const [a, b] of ranges) {
    if (a > e) {
      total += e - s
      s = a
      e = b
    } else e = Math.max(e, b)
  }
  return total + (e - s)
}

/**
 * Two different "how long" numbers. `summed` adds every conversation's
 * agent time, so parallel agents count once each; `busy` is the clock time
 * during which at least one agent was working (overlap counted once).
 * `concurrency` = summed / busy, the average number running at once.
 */
export function agentTime(chats: Iterable<Chat>) {
  const ranges: [number, number][] = []
  let summed = 0
  for (const chat of chats) {
    summed += chat.agentMs
    for (const r of turnRanges(chat)) ranges.push(r)
  }
  const busy = ranges.length ? unionLength(ranges) : 0
  return { summed, busy, concurrency: busy ? summed / busy : 0 }
}

/** Taipei has no daylight saving, so a local day is always the same 24-hour window. */
const TZ_OFFSET = 8 * 3_600_000

/**
 * `busy` per local day. A turn that runs past midnight counts toward both
 * days, each only for its own part, so no day can come out above 24 hours.
 */
export function busyByDay(chats: Iterable<Chat>): Map<string, number> {
  const perDay = new Map<string, [number, number][]>()
  for (const chat of chats) {
    for (const [a, b] of turnRanges(chat)) {
      for (let s = a; s < b; ) {
        const midnight = Math.floor((s + TZ_OFFSET) / DAY) * DAY - TZ_OFFSET + DAY
        const e = Math.min(b, midnight)
        const key = localDayKey(s)
        const list = perDay.get(key)
        if (list) list.push([s, e])
        else perDay.set(key, [[s, e]])
        s = e
      }
    }
  }
  return new Map([...perDay].map(([key, ranges]) => [key, unionLength(ranges)]))
}

/** Projects sorted by last activity, newest first (excluding projects that only exist as merge ghosts). */
export const projectsByRecent = [...archive.projects]
  .filter((p) => p.chatIds.length > 0)
  .sort((a, b) => b.lastActive - a.lastActive)

export const totals = (() => {
  const t = sumTokens(archive.chats)
  const agentMs = archive.chats.reduce((s, c) => s + c.agentMs, 0)
  const gone = archive.chats.filter((c) => c.source === 'gone').length
  const subagents = archive.chats.reduce((s, c) => s + c.subagentCount, 0)
  const sourceBytes = archive.chats.reduce((s, c) => s + c.sizeBytes, 0)
  const recordedDuration = archive.chats.filter((c) => c.agentMsSource === 'recorded').length
  return { ...t, agentMs, gone, subagents, sourceBytes, recordedDuration, projects: projectsByRecent.length }
})()
