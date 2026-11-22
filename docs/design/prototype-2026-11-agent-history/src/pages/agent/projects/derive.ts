/**
 * Numbers the projects screens show, derived from the fixture. In the
 * product these come pre-aggregated from the backend; nothing here should be
 * read as "compute on the render path".
 */
import { archive, harnessOrder } from '~/data/archive'
import { DAY, NOW, totalTokens } from '~/data/fixture'
import type { Chat, HarnessId, Project } from '~/data/types'

const WEEK = 7 * DAY
export const WEEKS = 26
export const weekStart = NOW - WEEKS * WEEK

export function chatsOf(project: Project): Chat[] {
  // Ids run newest first, so ascending id order is newest first.
  return project.chatIds.map((id) => archive.chatById.get(id)!)
}

export interface Row {
  project: Project
  /** Folders (each once shown as its own project) the user added to this one. */
  absorbed: Project[]
  chatCount: number
  subagents: number
  agentMs: number
  tokens: number
  tokensKnown: number
  lastActive: number
  firstActive: number
  harnesses: HarnessId[]
  weekly: number[]
}

export function buildRow(project: Project, absorbed: Project[]): Row {
  const all = [project, ...absorbed]
  const weekly = new Array<number>(WEEKS).fill(0)
  let chatCount = 0
  let subagents = 0
  let agentMs = 0
  let tokens = 0
  let tokensKnown = 0
  let lastActive = 0
  let firstActive = Infinity
  const hs = new Set<HarnessId>()
  for (const p of all) {
    p.weekly.forEach((v, i) => (weekly[i] += v))
    for (const chat of chatsOf(p)) {
      chatCount++
      subagents += chat.subagentCount
      agentMs += chat.agentMs
      if (chat.tokens) {
        tokens += totalTokens(chat.tokens)
        tokensKnown++
      }
      lastActive = Math.max(lastActive, chat.ended)
      firstActive = Math.min(firstActive, chat.started)
      hs.add(chat.harness)
    }
  }
  return {
    project,
    absorbed,
    chatCount,
    subagents,
    agentMs,
    tokens,
    tokensKnown,
    lastActive,
    firstActive,
    harnesses: harnessOrder.filter((h) => hs.has(h)),
    weekly,
  }
}

export type Metric = 'chats' | 'agent' | 'tokens'

/** Last 26 weeks, one value per harness, for the stacked activity chart. */
export function weeklyByHarness(chats: Chat[], metric: Metric) {
  const rows = Array.from({ length: WEEKS }, (_, i) => {
    const start = new Date(weekStart + i * WEEK + 8 * 3600_000)
    const row: Record<string, number | string> = {
      week: `${start.getUTCMonth() + 1}/${start.getUTCDate()}`,
    }
    for (const h of harnessOrder) row[h] = 0
    return row
  })
  for (const chat of chats) {
    if (chat.started < weekStart) continue
    const i = Math.min(WEEKS - 1, Math.floor((chat.started - weekStart) / WEEK))
    const value =
      metric === 'chats'
        ? 1
        : metric === 'agent'
          ? chat.agentMs / 3_600_000
          : chat.tokens
            ? totalTokens(chat.tokens) / 1e4
            : 0
    rows[i][chat.harness] = (rows[i][chat.harness] as number) + value
  }
  if (metric !== 'chats') {
    for (const row of rows) for (const h of harnessOrder) row[h] = Math.round((row[h] as number) * 10) / 10
  }
  return rows
}

export interface BranchInfo {
  branch: string
  chats: number
  lastActive: number
  prs: number[]
  harnesses: HarnessId[]
}

export function branchesOf(chats: Chat[]): BranchInfo[] {
  const map = new Map<string, BranchInfo>()
  for (const chat of chats) {
    if (!chat.branch) continue
    let info = map.get(chat.branch)
    if (!info) {
      info = { branch: chat.branch, chats: 0, lastActive: 0, prs: [], harnesses: [] }
      map.set(chat.branch, info)
    }
    info.chats++
    info.lastActive = Math.max(info.lastActive, chat.ended)
    if (chat.pr && !info.prs.includes(chat.pr.number)) info.prs.push(chat.pr.number)
    if (!info.harnesses.includes(chat.harness)) info.harnesses.push(chat.harness)
  }
  return [...map.values()].sort((a, b) => b.lastActive - a.lastActive)
}

export function modelsOf(chats: Chat[]) {
  const counts = new Map<string, number>()
  for (const chat of chats) for (const m of chat.models) counts.set(m, (counts.get(m) ?? 0) + 1)
  return [...counts.entries()].sort((a, b) => b[1] - a[1])
}

/** Parent folder label for the folder tree. */
export function parentLabel(parent: string) {
  return parent === '' ? '家目錄與暫存資料夾' : parent
}
