/**
 * Search over every conversation, streamed: results arrive grouped by
 * conversation while the scan runs, the count only ever grows, and the
 * total is exact when it finishes. No top-k, no cut-off.
 */
import { archive } from './archive'
import { searchLines, type Role } from './transcript'
import type { Chat, HarnessId } from './types'

export interface SearchQuery {
  q: string
  regex: boolean
  roles: Role[]
  harnesses: HarnessId[]
  projectId?: string
  /** Conversations left after the list's other filters (time, flags); searched newest first. */
  pool?: Chat[]
}

export interface Snippet {
  role: Role
  event: number
  /** Text around the match, with the match range inside it. */
  before: string
  match: string
  after: string
}

export interface ChatHit {
  chat: Chat
  hits: number
  snippets: Snippet[]
  titleHit: boolean
}

export interface SearchState {
  scanned: number
  total: number
  hits: number
  results: ChatHit[]
  done: boolean
  error?: string
}

export function matcher(q: string, regex: boolean): RegExp | 'invalid' | null {
  if (!q.trim()) return null
  if (regex) {
    try {
      return new RegExp(q, 'gi')
    } catch {
      return 'invalid'
    }
  }
  return new RegExp(q.trim().replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'gi')
}

function snippet(text: string, index: number, length: number, role: Role, event: number): Snippet {
  const start = Math.max(0, index - 36)
  const end = Math.min(text.length, index + length + 60)
  return {
    role,
    event,
    before: (start > 0 ? '…' : '') + text.slice(start, index).replace(/\s+/g, ' '),
    match: text.slice(index, index + length),
    after: text.slice(index + length, end).replace(/\s+/g, ' ') + (end < text.length ? '…' : ''),
  }
}

/** Runs the scan in slices of about 8 ms so the page stays responsive. Returns a cancel function. */
export function runSearch(query: SearchQuery, onUpdate: (state: SearchState) => void): () => void {
  const found0 = matcher(query.q, query.regex)
  const pool = (query.pool ?? archive.chats).filter(
    (c) =>
      (query.harnesses.length === 0 || query.harnesses.includes(c.harness)) &&
      (!query.projectId || c.projectId === query.projectId),
  )
  const state: SearchState = { scanned: 0, total: pool.length, hits: 0, results: [], done: false }
  if (found0 === 'invalid') {
    onUpdate({ ...state, done: true, error: '這個正規表示式寫錯了' })
    return () => {}
  }
  if (!found0) {
    onUpdate({ ...state, done: true })
    return () => {}
  }
  const re: RegExp = found0
  const roles = new Set(query.roles)
  let cancelled = false
  let i = 0
  const step = () => {
    if (cancelled) return
    const until = performance.now() + 8
    const found: ChatHit[] = []
    while (i < pool.length && performance.now() < until) {
      const chat = pool[i++]
      let hits = 0
      const snippets: Snippet[] = []
      re.lastIndex = 0
      const titleHit = re.test(chat.title)
      for (const line of searchLines(chat)) {
        if (!roles.has(line.role)) continue
        re.lastIndex = 0
        let m: RegExpExecArray | null
        while ((m = re.exec(line.text))) {
          if (m[0].length === 0) {
            re.lastIndex++
            continue
          }
          hits++
          if (snippets.length < 3) snippets.push(snippet(line.text, m.index, m[0].length, line.role, line.event))
        }
      }
      if (hits > 0 || titleHit) found.push({ chat, hits, snippets, titleHit })
      state.hits += hits
    }
    state.scanned = i
    if (found.length) state.results = state.results.concat(found)
    state.done = i >= pool.length
    onUpdate({ ...state })
    if (!state.done) setTimeout(step, 0)
  }
  setTimeout(step, 0)
  return () => {
    cancelled = true
  }
}

export const roleLabels: Record<Role, string> = {
  user: '你說的',
  agent: 'Agent 說的',
  tool: '工具輸出',
  thinking: '思考',
}
