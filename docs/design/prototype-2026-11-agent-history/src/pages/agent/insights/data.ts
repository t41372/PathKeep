/**
 * Insights numbers for one scope (range, harnesses, project). In the product
 * these come pre-aggregated from the backend; here they are computed from the
 * invented archive, once per scope.
 */
import { agentTime, archive, busyByDay, harnessOrder } from '~/data/archive'
import { addTokens, DAY, NOW, totalTokens, zeroTokens } from '~/data/fixture'
import { localDayKey } from '~/data/format'
import type { Chat, HarnessId, Tokens } from '~/data/types'

export type Range = '7' | '30' | '90' | '365' | 'all'

export interface Scope {
  range: Range
  harnesses: HarnessId[]
  projectId?: string
}

export const rangeLabels: Record<Range, string> = {
  '7': '7 天',
  '30': '30 天',
  '90': '90 天',
  '365': '一年',
  all: '全部',
}

export const rangeWindow: Record<Range, string> = {
  '7': '最近 7 天',
  '30': '最近 30 天',
  '90': '最近 90 天',
  '365': '最近一年',
  all: '全部時間',
}

export type Bucket = 'day' | 'week' | 'month'

export const bucketOf = (range: Range): Bucket =>
  range === '7' || range === '30' ? 'day' : range === 'all' ? 'month' : 'week'

export const bucketWord: Record<Bucket, string> = { day: '每天', week: '每週', month: '每月' }

const TZ = 8 * 3600_000

export function inScope(chat: Chat, scope: Scope) {
  if (scope.range !== 'all' && chat.started < NOW - Number(scope.range) * DAY) return false
  if (scope.harnesses.length && !scope.harnesses.includes(chat.harness)) return false
  if (scope.projectId && chat.projectId !== scope.projectId) return false
  return true
}

/** Key of the bucket a time falls in, in Taipei time. Weeks start on Monday. */
export function bucketKey(t: number, bucket: Bucket): string {
  const day = localDayKey(t)
  if (bucket === 'day') return day
  if (bucket === 'month') return day.slice(0, 7)
  const d = new Date(Date.parse(day))
  const back = (d.getUTCDay() + 6) % 7
  return new Date(d.getTime() - back * DAY).toISOString().slice(0, 10)
}

function bucketKeys(scope: Scope, bucket: Bucket, chats: Chat[]): string[] {
  const start =
    scope.range === 'all'
      ? Math.min(...chats.map((c) => c.started), NOW - 30 * DAY)
      : NOW - Number(scope.range) * DAY
  const keys: string[] = []
  let t = start
  const seen = new Set<string>()
  while (t <= NOW + DAY) {
    const key = bucketKey(Math.min(t, NOW), bucket)
    if (!seen.has(key)) {
      seen.add(key)
      keys.push(key)
    }
    t += DAY
  }
  return keys
}

export function bucketLabel(key: string, bucket: Bucket): string {
  if (bucket === 'month') return `${key.slice(2, 4)}/${Number(key.slice(5, 7))}`
  return `${Number(key.slice(5, 7))}/${Number(key.slice(8, 10))}`
}

/** Approximate time the person was there: each conversation's working span, merged across conversations. */
export function presentMs(chats: Chat[]): number {
  const spans = chats
    .map((c) => [c.started, Math.min(c.ended, c.started + c.agentMs * 1.35)] as const)
    .sort((a, b) => a[0] - b[0])
  let total = 0
  let curStart = -1
  let curEnd = -1
  const gap = 15 * 60_000
  for (const [s, e] of spans) {
    if (curEnd < 0) {
      curStart = s
      curEnd = e
    } else if (s <= curEnd + gap) {
      curEnd = Math.max(curEnd, e)
    } else {
      total += curEnd - curStart
      curStart = s
      curEnd = e
    }
  }
  if (curEnd > 0) total += curEnd - curStart
  return total
}

export function modelKey(model: string) {
  return 'm' + model.replace(/[^a-z0-9]/gi, '')
}

export const toolWeights: Record<HarnessId, readonly (readonly [string, number])[]> = {
  claude: [['Read', 0.3], ['Bash', 0.22], ['Edit', 0.18], ['Grep', 0.1], ['Glob', 0.05], ['TodoWrite', 0.05], ['Write', 0.04], ['WebSearch', 0.02], ['WebFetch', 0.02], ['其他', 0.02]],
  codex: [['shell', 0.55], ['apply_patch', 0.2], ['read_file', 0.15], ['web_search', 0.04], ['update_plan', 0.04], ['其他', 0.02]],
  pi: [['bash', 0.45], ['read', 0.3], ['edit', 0.2], ['其他', 0.05]],
  deepseek: [['bash', 0.5], ['read', 0.3], ['edit', 0.2]],
}

export interface Insights {
  chats: Chat[]
  bucket: Bucket
  keys: string[]
  tokens: Tokens
  tokensKnown: number
  agentMs: number
  /** Clock time with at least one agent working; overlap counted once. */
  busy: number
  concurrency: number
  recordedDurations: number
  present: number
  subagents: number
  linesAdded: number
  linesRemoved: number
  tokenSeries: Record<string, number | string>[]
  harnessTokenSeries: Record<string, number | string>[]
  modelTokenSeries: Record<string, number | string>[]
  models: { key: string; model: string; tokens: number; chats: number }[]
  hoursSeries: Record<string, number | string>[]
  harnessMix: { id: HarnessId; chats: number; agentMs: number; tokens: number }[]
  rhythm: number[][]
  topProjects: { id: string; agentMs: number; chats: number; tokens: number; harnesses: HarnessId[] }[]
  projectCount: number
  tools: { tool: string; count: number; harness: HarnessId }[]
  totalToolCalls: number
  longest: Chat[]
  cost: { total: number; chats: number; byHarness: Partial<Record<HarnessId, number>> }
}

const cache = new Map<string, Insights>()

export function insightsFor(scope: Scope): Insights {
  const id = JSON.stringify(scope)
  const hit = cache.get(id)
  if (hit) return hit
  const chats = archive.chats.filter((c) => inScope(c, scope))
  const bucket = bucketOf(scope.range)
  const keys = bucketKeys(scope, bucket, chats)
  const index = new Map(keys.map((k, i) => [k, i]))

  const tokens = zeroTokens()
  let tokensKnown = 0
  let agentMs = 0
  let recordedDurations = 0
  let subagents = 0
  let linesAdded = 0
  let linesRemoved = 0
  const tokenSeries = keys.map((k) => ({ k, output: 0, input: 0, cacheWrite: 0, cacheRead: 0 }) as Record<string, number | string>)
  const harnessTokenSeries = keys.map((k) => ({ k, claude: 0, codex: 0, pi: 0, deepseek: 0 }) as Record<string, number | string>)
  const modelTokenSeries = keys.map((k) => ({ k }) as Record<string, number | string>)
  const hoursSeries = keys.map((k) => ({ k, busy: 0, summed: 0 }) as Record<string, number | string>)
  const perBucket = keys.map(() => [] as Chat[])
  const modelTotals = new Map<string, { tokens: number; chats: number }>()
  const mix = new Map<HarnessId, { chats: number; agentMs: number; tokens: number }>()
  const rhythm = Array.from({ length: 24 }, () => new Array<number>(7).fill(0))
  const projects = new Map<string, { agentMs: number; chats: number; tokens: number; harnesses: Set<HarnessId> }>()
  const tools = new Map<string, { count: number; harness: HarnessId }>()
  let totalToolCalls = 0
  const cost = { total: 0, chats: 0, byHarness: {} as Partial<Record<HarnessId, number>> }

  const add = (row: Record<string, number | string>, key: string, v: number) => {
    row[key] = ((row[key] as number) ?? 0) + v
  }

  for (const chat of chats) {
    const i = index.get(bucketKey(chat.started, bucket))
    agentMs += chat.agentMs
    if (chat.agentMsSource === 'recorded') recordedDurations++
    subagents += chat.subagentCount
    linesAdded += chat.linesAdded
    linesRemoved += chat.linesRemoved
    const m = mix.get(chat.harness) ?? { chats: 0, agentMs: 0, tokens: 0 }
    m.chats++
    m.agentMs += chat.agentMs
    const total = chat.tokens ? totalTokens(chat.tokens) : 0
    m.tokens += total
    mix.set(chat.harness, m)
    if (chat.tokens) {
      tokensKnown++
      addTokens(tokens, chat.tokens)
      if (i !== undefined) {
        add(tokenSeries[i], 'output', chat.tokens.output)
        add(tokenSeries[i], 'input', chat.tokens.input)
        add(tokenSeries[i], 'cacheWrite', chat.tokens.cacheWrite)
        add(tokenSeries[i], 'cacheRead', chat.tokens.cacheRead)
        add(harnessTokenSeries[i], chat.harness, total)
        add(harnessTokenSeries[i], chat.harness + 'NoRead', total - chat.tokens.cacheRead)
        add(modelTokenSeries[i], modelKey(chat.models[0]), total)
        add(modelTokenSeries[i], modelKey(chat.models[0]) + 'NoRead', total - chat.tokens.cacheRead)
      }
      const mt = modelTotals.get(chat.models[0]) ?? { tokens: 0, chats: 0 }
      mt.tokens += total
      modelTotals.set(chat.models[0], mt)
    }
    const mc = modelTotals.get(chat.models[0]) ?? { tokens: 0, chats: 0 }
    mc.chats++
    modelTotals.set(chat.models[0], mc)
    if (i !== undefined) perBucket[i].push(chat)

    // Prompts spread over the conversation's span, for the weekday × hour grid.
    const span = Math.max(60_000, Math.min(chat.ended - chat.started, chat.agentMs * 1.6))
    const prompts = Math.min(chat.turns, 60)
    for (let p = 0; p < prompts; p++) {
      const t = chat.started + (span * p) / prompts + TZ
      const d = new Date(t)
      rhythm[d.getUTCHours()][(d.getUTCDay() + 6) % 7] += chat.turns / prompts
    }

    const pr = projects.get(chat.projectId) ?? { agentMs: 0, chats: 0, tokens: 0, harnesses: new Set<HarnessId>() }
    pr.agentMs += chat.agentMs
    pr.chats++
    pr.tokens += total
    pr.harnesses.add(chat.harness)
    projects.set(chat.projectId, pr)

    totalToolCalls += chat.toolCalls
    for (const [tool, w] of toolWeights[chat.harness]) {
      const key = `${chat.harness}:${tool}`
      const t = tools.get(key) ?? { count: 0, harness: chat.harness }
      t.count += chat.toolCalls * w
      tools.set(key, t)
    }
    if (chat.subagentCount) {
      const key = `${chat.harness}:${chat.harness === 'claude' ? 'Task（subagent）' : 'spawn_agent'}`
      const t = tools.get(key) ?? { count: 0, harness: chat.harness }
      t.count += chat.subagentCount
      tools.set(key, t)
      totalToolCalls += chat.subagentCount
    }
    if (chat.recordedCostUsd !== undefined) {
      cost.total += chat.recordedCostUsd
      cost.chats++
      cost.byHarness[chat.harness] = (cost.byHarness[chat.harness] ?? 0) + chat.recordedCostUsd
    }
  }

  perBucket.forEach((group, i) => {
    if (!group.length) return
    const t = agentTime(group)
    hoursSeries[i].busy = t.busy / 3_600_000
    hoursSeries[i].summed = t.summed / 3_600_000
  })
  const time = agentTime(chats)

  const models = [...modelTotals.entries()]
    .map(([model, v]) => ({ key: modelKey(model), model, ...v }))
    .sort((a, b) => b.tokens - a.tokens || b.chats - a.chats)

  const result: Insights = {
    chats,
    bucket,
    keys,
    tokens,
    tokensKnown,
    agentMs,
    busy: time.busy,
    concurrency: time.concurrency,
    recordedDurations,
    present: presentMs(chats),
    subagents,
    linesAdded,
    linesRemoved,
    tokenSeries,
    harnessTokenSeries,
    modelTokenSeries,
    models,
    hoursSeries,
    harnessMix: harnessOrder.filter((h) => mix.has(h)).map((h) => ({ id: h, ...mix.get(h)! })),
    rhythm,
    topProjects: [...projects.entries()]
      .map(([id, v]) => ({ id, agentMs: v.agentMs, chats: v.chats, tokens: v.tokens, harnesses: harnessOrder.filter((h) => v.harnesses.has(h)) }))
      .sort((a, b) => b.agentMs - a.agentMs),
    projectCount: projects.size,
    tools: [...tools.entries()]
      .map(([key, v]) => ({ tool: key.split(':')[1], count: Math.round(v.count), harness: v.harness }))
      .sort((a, b) => b.count - a.count),
    totalToolCalls,
    longest: [...chats].sort((a, b) => b.agentMs - a.agentMs).slice(0, 6),
    cost,
  }
  cache.set(id, result)
  return result
}

/** Hours with at least one agent working, per day of one year (overlap counted once). */
export function dailyAgentHours(year: number, scope: Omit<Scope, 'range'>): Map<string, number> {
  const chats = archive.chats.filter(
    (chat) =>
      (!scope.harnesses.length || scope.harnesses.includes(chat.harness)) &&
      (!scope.projectId || chat.projectId === scope.projectId) &&
      // A conversation can start on 31 December and keep going into the new year.
      (localDayKey(chat.started).startsWith(`${year}-`) || localDayKey(chat.ended).startsWith(`${year}-`)),
  )
  const out = new Map<string, number>()
  for (const [key, ms] of busyByDay(chats)) if (key.startsWith(`${year}-`)) out.set(key, ms / 3_600_000)
  return out
}
