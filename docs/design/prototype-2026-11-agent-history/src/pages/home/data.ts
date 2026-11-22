/**
 * Home's numbers. Agent numbers come from the invented archive; the other
 * data types are not built in this prototype, so their figures are invented
 * placeholders that only show how Home scales with more types.
 */
import { agentTime, archive, busyByDay, harnessOrder, totals } from '~/data/archive'
import { DAY, NOW, totalTokens } from '~/data/fixture'
import { localDayKey } from '~/data/format'
import { mulberry32 } from '~/data/rng'
import type { KindId } from '~/kinds'
import { bucketKey, dailyAgentHours } from '../agent/insights/data'

export interface KindStatus {
  value: string
  unit: string
  sub: string
  onlyHere?: string
  sample?: boolean
}

export const kindStatus: Record<KindId, KindStatus> = {
  agent: {
    value: totals.total.toLocaleString('zh-TW'),
    unit: '段對話',
    sub: `${harnessOrder.length} 個工具 · ${totals.projects} 個專案`,
    onlyHere: `只剩這裡有 ${totals.gone.toLocaleString('zh-TW')} 段`,
  },
  browsing: { value: '182 萬', unit: '筆瀏覽', sub: '4 個瀏覽器 · 2019 年起', onlyHere: '只剩這裡有 61 萬筆' },
  terminal: { value: '8.4 萬', unit: '條指令', sub: 'zsh、Atuin', onlyHere: '只剩這裡有 5.2 萬條', sample: true },
  messages: { value: '3.1 萬', unit: '則訊息', sub: 'Slack 2 個工作區', onlyHere: '只剩這裡有 1.4 萬則', sample: true },
  webchat: { value: '1,240', unit: '段對話', sub: 'ChatGPT、Claude.ai 匯出檔', sample: true },
  notifications: { value: '2.6 萬', unit: '則通知', sub: 'macOS 通知中心', onlyHere: '只剩這裡有 2.5 萬則', sample: true },
}

/** This week (Monday to now) and last week, Taipei time. */
export const week = (() => {
  const monday = Date.parse(bucketKey(NOW, 'week')) - 8 * 3600_000
  const thisWeek = archive.chats.filter((c) => c.started >= monday)
  // Last week up to the same weekday and hour, so a Friday isn't compared with a whole week.
  const lastWeek = archive.chats.filter((c) => c.started >= monday - 7 * DAY && c.started < NOW - 7 * DAY)
  const now = agentTime(thisWeek)
  const before = agentTime(lastWeek)
  const tokens = thisWeek.reduce((s, c) => s + (c.tokens ? totalTokens(c.tokens) : 0), 0)
  const known = thisWeek.filter((c) => c.tokens).length
  // Split at midnight, so a conversation running past it counts on both days.
  const perDay = busyByDay(thisWeek)
  const days = Array.from({ length: 7 }, (_, i) => {
    const start = monday + i * DAY
    return { label: '一二三四五六日'[i], hours: (perDay.get(localDayKey(start)) ?? 0) / 3_600_000, future: start > NOW }
  })
  return {
    chats: thisWeek.length,
    subagents: thisWeek.reduce((s, c) => s + c.subagentCount, 0),
    busy: now.busy,
    summed: now.summed,
    concurrency: now.concurrency,
    lastBusy: before.busy,
    tokens,
    known,
    days,
  }
})()

/** Conversations from this day in earlier years. */
export const onThisDay = (() => {
  const today = localDayKey(NOW).slice(5)
  const years = [...new Set(archive.chats.map((c) => localDayKey(c.started).slice(0, 4)))]
    .filter((y) => y < localDayKey(NOW).slice(0, 4))
    .sort()
    .reverse()
  return years.map((year) => {
    const key = `${year}-${today}`
    return { key, chats: archive.chats.filter((c) => localDayKey(c.started) === key) }
  })
})()

/** Invented daily counts for the types this prototype doesn't build. */
export function invented(kind: KindId, year: number): Map<string, number> {
  const rng = mulberry32(kind.length * 7919 + year)
  const out = new Map<string, number>()
  const start = Date.UTC(year, 0, 1)
  for (let t = start; t < Date.UTC(year + 1, 0, 1) && t <= NOW; t += DAY) {
    const weekday = new Date(t).getUTCDay()
    const base = weekday === 0 || weekday === 6 ? 0.5 : 1
    const v = rng() < 0.08 ? 0 : Math.round(base * (40 + rng() * 260))
    if (v) out.set(new Date(t).toISOString().slice(0, 10), v)
  }
  return out
}

export function yearCounts(kind: KindId, year: number): Map<string, number> {
  return kind === 'agent' ? dailyAgentHours(year, { harnesses: [] }) : invented(kind, year)
}
