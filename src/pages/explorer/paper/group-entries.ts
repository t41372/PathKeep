/**
 * Group a flat HistoryEntry stream into the paper-redesign contact-sheet
 * shape: days → sessions → (single | stack) blocks.
 *
 * Responsibilities:
 * - Bucket entries by local calendar day (YYYY-MM-DD).
 * - Within a day, split into sessions when there's a gap > sessionGapMinutes.
 * - Within a session, fold runs of consecutive same-domain visits into
 *   "stack" blocks (>= stackThreshold), otherwise emit as singles.
 *
 * The grouping is deterministic — no Date.now lookups, no Math.random —
 * because the contact sheet renders need to stay stable when re-rendered
 * with the same data.
 */

import type { HistoryEntry } from '@/lib/types/archive'

export interface PaperSingleBlock {
  type: 'single'
  entry: HistoryEntry
}

export interface PaperStackBlock {
  type: 'stack'
  domain: string
  entries: HistoryEntry[]
}

export type PaperBlock = PaperSingleBlock | PaperStackBlock

export interface PaperSession {
  id: string
  startMs: number
  endMs: number
  blocks: PaperBlock[]
  visitCount: number
}

export interface PaperDay {
  date: string
  visitCount: number
  domains: number
  sessions: PaperSession[]
}

const SESSION_GAP_MINUTES = 30
const STACK_THRESHOLD = 3

export function localDayKey(visitedAt: string | null | undefined): string {
  if (typeof visitedAt !== 'string' || visitedAt.length === 0) {
    return 'unknown'
  }
  const date = new Date(visitedAt)
  if (Number.isNaN(date.getTime())) return visitedAt.slice(0, 10)
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}

function visitTimeMs(entry: HistoryEntry): number {
  // The archive backend returns `visit_time` already in milliseconds
  // (visits.visit_time_ms). Legacy fixtures + tests historically passed
  // a 10-digit seconds-since-epoch value here, so detect by magnitude:
  // values > 1e12 are already ms, anything smaller is seconds. Blindly
  // multiplying by 1000 on a ms value pushed the time into year ~57000
  // and made every session divider show a garbage clock (e.g., a "下午
  // 8:14" header above entries that actually happened at 下午 1:11).
  if (Number.isFinite(entry.visitTime) && entry.visitTime > 0) {
    return entry.visitTime > 1e12 ? entry.visitTime : entry.visitTime * 1000
  }
  const parsed = Date.parse(entry.visitedAt)
  return Number.isNaN(parsed) ? 0 : parsed
}

function groupConsecutiveDomains(entries: HistoryEntry[]): PaperBlock[] {
  const blocks: PaperBlock[] = []
  let index = 0
  while (index < entries.length) {
    const head = entries[index]
    const run = [head]
    let next = index + 1
    while (
      next < entries.length &&
      entries[next].domain === head.domain &&
      head.domain.length > 0
    ) {
      run.push(entries[next])
      next += 1
    }
    if (run.length >= STACK_THRESHOLD) {
      blocks.push({ type: 'stack', domain: head.domain, entries: run })
    } else {
      run.forEach((entry) => blocks.push({ type: 'single', entry }))
    }
    index = next
  }
  return blocks
}

function splitIntoSessions(entries: HistoryEntry[]): PaperSession[] {
  if (entries.length === 0) return []
  const gapMs = SESSION_GAP_MINUTES * 60 * 1000
  const sessions: HistoryEntry[][] = []
  let current: HistoryEntry[] = [entries[0]]
  for (let i = 1; i < entries.length; i += 1) {
    const prevMs = visitTimeMs(current[current.length - 1])
    const currMs = visitTimeMs(entries[i])
    if (Math.abs(prevMs - currMs) > gapMs) {
      sessions.push(current)
      current = [entries[i]]
    } else {
      current.push(entries[i])
    }
  }
  sessions.push(current)
  return sessions.map((sessionEntries, idx) => {
    const times = sessionEntries.map(visitTimeMs)
    const startMs = Math.min(...times)
    const endMs = Math.max(...times)
    return {
      id: `${sessionEntries[0].id}-${idx}`,
      startMs,
      endMs,
      visitCount: sessionEntries.length,
      blocks: groupConsecutiveDomains(sessionEntries),
    }
  })
}

export function groupEntriesByDay(entries: HistoryEntry[]): PaperDay[] {
  if (entries.length === 0) return []
  const sorted = [...entries].sort((a, b) => visitTimeMs(b) - visitTimeMs(a))
  const buckets = new Map<string, HistoryEntry[]>()
  sorted.forEach((entry) => {
    const key = localDayKey(entry.visitedAt)
    const list = buckets.get(key)
    if (list) list.push(entry)
    else buckets.set(key, [entry])
  })
  const days: PaperDay[] = []
  buckets.forEach((dayEntries, date) => {
    const domains = new Set(dayEntries.map((entry) => entry.domain))
    days.push({
      date,
      visitCount: dayEntries.length,
      domains: domains.size,
      sessions: splitIntoSessions(dayEntries),
    })
  })
  return days.sort((a, b) => (a.date < b.date ? 1 : a.date > b.date ? -1 : 0))
}

export function describeDay(date: string, language: string): string {
  try {
    const parts = date.split('-').map((part) => Number.parseInt(part, 10))
    if (parts.length !== 3 || parts.some(Number.isNaN)) return date
    const [year, month, day] = parts
    const native = new Date(year, month - 1, day)
    if (Number.isNaN(native.getTime())) return date
    return native.toLocaleDateString(language, {
      weekday: 'long',
      year: 'numeric',
      month: 'long',
      day: 'numeric',
    })
  } catch {
    return date
  }
}

export function formatHourMinute(
  ms: number,
  language: string,
  options: { hour12?: boolean } = {},
): string {
  try {
    const date = new Date(ms)
    if (Number.isNaN(date.getTime())) return '--:--'
    return date.toLocaleTimeString(language, {
      hour: options.hour12 ? 'numeric' : '2-digit',
      minute: '2-digit',
      hour12: options.hour12 ?? false,
    })
  } catch {
    return '--:--'
  }
}
