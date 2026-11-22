/**
 * The reader's view of one conversation: a projection of the native records
 * (mirrors merged, active branch chosen, subagents linked, compactions
 * marked) plus the raw records for the "all events" mode. Invented text,
 * generated from the chat id so it is stable across reloads.
 */
import { subagentsOf } from './archive'
import { lognormal, mulberry32, pick, type Rng } from './rng'
import { assistantReplies, bashCommands, filePaths, followUps, plainReplies, thinkingTexts } from './text-bank'
import type { Chat, Subagent, Tokens } from './types'

export type ToolName = 'Read' | 'Grep' | 'Glob' | 'Bash' | 'Edit' | 'Write' | 'TodoWrite' | 'WebSearch' | 'shell' | 'apply_patch' | 'read_file'

export interface ToolCall {
  id: string
  tool: ToolName
  /** The argument a person would recognise: a path, a command, a pattern. */
  arg: string
  ms: number
  status: 'ok' | 'error' | 'denied'
  output?: string
  diff?: { add: number; del: number; text: string }
}

export type TEvent =
  | { k: 'user'; id: string; ts: number; turn: number; text: string; attachments?: { name: string; missing: boolean }[]; branches?: { count: number; current: number } }
  | { k: 'thinking'; id: string; ts: number; turn: number; text: string; ms: number }
  | { k: 'tools'; id: string; ts: number; turn: number; calls: ToolCall[] }
  | { k: 'assistant'; id: string; ts: number; turn: number; md: string }
  | { k: 'subagent'; id: string; ts: number; turn: number; sub: Subagent }
  | { k: 'compaction'; id: string; ts: number; turn: number; tokensBefore: number }
  | { k: 'turn-end'; id: string; ts: number; turn: number; ms: number; tokens: Tokens | null; model: string }
  | { k: 'notice'; id: string; ts: number; turn: number; tone: 'info' | 'warn' | 'error'; text: string }

export interface RawRecord {
  ordinal: number
  type: string
  ts: number
  bytes: number
  /** Index of the projected event this record feeds, if any. */
  event: number | null
}

export interface Transcript {
  events: TEvent[]
  raw: RawRecord[]
  /** Event indexes of the user's prompts, for the outline. */
  prompts: number[]
}

const cache = new Map<number, Transcript>()

const diffText = (rng: Rng, path: string) => {
  const fn = pick(rng, ['parse_timestamp', 'list_sessions', 'useSearch', 'openArchive', 'renderRow'])
  return [
    `--- a/${path}`,
    `+++ b/${path}`,
    '@@ -38,9 +38,12 @@',
    `   fn ${fn}(input: &str) -> Result<Value> {`,
    '-    let value = input.parse::<i64>()?;',
    '-    Ok(Value::from_utc(value))',
    '+    let value = input.parse::<i64>()?;',
    '+    // Safari stores local time; convert with the zone of the import.',
    '+    let zone = self.zone.unwrap_or(Zone::system());',
    '+    Ok(Value::from_local(value, zone))',
    '   }',
  ].join('\n')
}

function toolCalls(rng: Rng, chat: Chat, base: string, count: number): ToolCall[] {
  const codex = chat.harness === 'codex'
  return Array.from({ length: count }, (_, i) => {
    const tool: ToolName = codex
      ? pick(rng, ['shell', 'shell', 'read_file', 'apply_patch'] as const)
      : pick(rng, ['Read', 'Read', 'Grep', 'Bash', 'Edit', 'Glob', 'Bash', 'Edit', 'TodoWrite'] as const)
    const path = pick(rng, filePaths)
    const ms = Math.round(lognormal(rng, tool === 'Bash' || tool === 'shell' ? 4200 : 300, 1))
    const status: ToolCall['status'] = rng() < 0.04 ? 'error' : rng() < 0.02 ? 'denied' : 'ok'
    const call: ToolCall = { id: `${base}-c${i}`, tool, arg: path, ms, status }
    if (tool === 'Bash' || tool === 'shell') {
      call.arg = pick(rng, bashCommands)
      call.output = status === 'error' ? 'error[E0308]: mismatched types\n  --> src/query/sessions.rs:88:17' : `running 412 tests\n${'.'.repeat(40)}\ntest result: ok. 412 passed; 0 failed; finished in 8.31s`
    } else if (tool === 'Grep') {
      call.arg = `"${pick(rng, ['open_archive', 'useSearch', 'zone', 'parse_ts', 'TODO', 'unwrap()'])}"`
      call.output = `${filePaths.slice(0, 3).map((p, n) => `${p}:${40 + n * 7}`).join('\n')}`
    } else if (tool === 'Edit' || tool === 'apply_patch') {
      const add = Math.round(lognormal(rng, 6, 0.8))
      call.diff = { add, del: Math.round(add * 0.5), text: diffText(rng, path) }
    } else if (tool === 'TodoWrite') {
      call.arg = '更新待辦（3 項）'
    } else if (tool === 'Read' || tool === 'read_file') {
      call.output = `（${Math.round(lognormal(rng, 180, 0.7))} 行）`
    }
    return call
  })
}

function generate(chat: Chat): Transcript {
  const rng = mulberry32(chat.id * 31337)
  const events: TEvent[] = []
  const prompts: number[] = []
  const subs = subagentsOf(chat)
  let subIndex = 0
  const turns = chat.turns
  const perTurn = chat.agentMs / turns
  let ts = chat.started
  const compactAt = new Set<number>()
  for (let c = 1; c <= chat.compactions; c++) compactAt.add(Math.floor((turns * c) / (chat.compactions + 1)))
  const branchTurn = turns > 3 && rng() < 0.35 ? 1 + Math.floor(rng() * (turns - 2)) : -1
  const missingTurn = turns > 2 && rng() < 0.25 ? Math.floor(rng() * turns) : -1
  const interruptedTurn = turns > 4 && rng() < 0.3 ? 2 + Math.floor(rng() * (turns - 3)) : -1
  const errorTurn = chat.apiErrors ? Math.floor(rng() * turns) : -1

  for (let turn = 0; turn < turns; turn++) {
    const id = (n: string) => `${chat.id}-${turn}-${n}`
    if (compactAt.has(turn)) {
      events.push({ k: 'compaction', id: id('compact'), ts, turn, tokensBefore: Math.round(lognormal(rng, 150_000, 0.2)) })
    }
    const text = turn === 0 ? chat.firstPrompt : pick(rng, followUps)
    prompts.push(events.length)
    events.push({
      k: 'user',
      id: id('u'),
      ts,
      turn,
      text,
      attachments: turn === missingTurn ? [{ name: '截圖 2026-11-03 下午 3.12.04.png', missing: true }] : undefined,
      branches: turn === branchTurn ? { count: 2, current: 2 } : undefined,
    })
    const turnMs = perTurn * lognormal(rng, 1, 0.5)
    let local = ts + 2000
    if (rng() < 0.55) {
      events.push({ k: 'thinking', id: id('th'), ts: local, turn, text: pick(rng, thinkingTexts), ms: Math.round(lognormal(rng, 6000, 0.8)) })
      local += 6000
    }
    if (turn === errorTurn) {
      events.push({ k: 'notice', id: id('err'), ts: local, turn, tone: 'warn', text: 'API 回傳 529（過載），工具自動重試了 2 次後成功' })
    }
    const groups = 1 + (rng() < 0.4 ? 1 : 0)
    for (let g = 0; g < groups; g++) {
      const count = Math.max(1, Math.round(lognormal(rng, chat.toolCalls / turns / groups, 0.6)))
      events.push({ k: 'tools', id: id(`t${g}`), ts: local, turn, calls: toolCalls(rng, chat, id(`t${g}`), Math.min(count, 30)) })
      local += turnMs / (groups + 1)
      if (subIndex < subs.length && rng() < Math.min(0.9, subs.length / turns + 0.1)) {
        const take = Math.min(subs.length - subIndex, 1 + (rng() < 0.3 ? Math.floor(rng() * 3) : 0))
        for (let s = 0; s < take; s++) {
          events.push({ k: 'subagent', id: id(`s${g}-${s}`), ts: local, turn, sub: subs[subIndex++] })
        }
      }
    }
    if (turn === interruptedTurn) {
      events.push({ k: 'notice', id: id('int'), ts: local, turn, tone: 'info', text: '你中斷了這一輪，Agent 沒有回覆' })
    } else {
      events.push({ k: 'assistant', id: id('a'), ts: local, turn, md: pick(rng, rng() < 0.18 ? assistantReplies : plainReplies) })
    }
    const share = 1 / turns
    events.push({
      k: 'turn-end',
      id: id('end'),
      ts: ts + turnMs,
      turn,
      ms: Math.round(turnMs),
      tokens: chat.tokens
        ? {
            input: Math.round(chat.tokens.input * share),
            cacheWrite: Math.round(chat.tokens.cacheWrite * share),
            cacheRead: Math.round(chat.tokens.cacheRead * share),
            output: Math.round(chat.tokens.output * share),
            reasoning: Math.round(chat.tokens.reasoning * share),
          }
        : null,
      model: chat.models[0],
    })
    ts += turnMs + lognormal(rng, 80_000, 1)
  }
  // Leftover subagents attach to the last turn.
  while (subIndex < subs.length) {
    events.splice(events.length - 2, 0, { k: 'subagent', id: `${chat.id}-late-${subIndex}`, ts, turn: turns - 1, sub: subs[subIndex++] })
  }

  // Native records: each projected event comes from one or more raw lines.
  const raw: RawRecord[] = []
  const nativeTypes: Record<TEvent['k'], string[]> = chat.harness === 'codex'
    ? { user: ['response_item:message'], thinking: ['response_item:reasoning'], tools: ['response_item:function_call', 'response_item:function_call_output'], assistant: ['response_item:message', 'event_msg:agent_message'], subagent: ['event_msg:collab_agent_spawn'], compaction: ['compacted'], 'turn-end': ['event_msg:token_count', 'event_msg:task_complete'], notice: ['event_msg:error'] }
    : { user: ['user'], thinking: ['assistant:thinking'], tools: ['assistant:tool_use', 'user:tool_result'], assistant: ['assistant:text', 'assistant:text'], subagent: ['assistant:tool_use(Agent)', 'user:tool_result'], compaction: ['system:compact_boundary', 'user:isCompactSummary'], 'turn-end': ['system:turn_duration'], notice: ['system:api_error'] }
  events.forEach((event, i) => {
    const kinds = nativeTypes[event.k]
    const repeat = event.k === 'tools' ? event.calls.length : 1
    for (let r = 0; r < repeat; r++) {
      for (const type of kinds) raw.push({ ordinal: raw.length, type, ts: event.ts, bytes: Math.round(lognormal(rng, 1800, 1)), event: i })
    }
    if (chat.harness === 'claude' && rng() < 0.12) raw.push({ ordinal: raw.length, type: pick(rng, ['file-history-snapshot', 'attachment', 'permission-mode', 'progress']), ts: event.ts, bytes: Math.round(lognormal(rng, 900, 1)), event: null })
  })
  if (chat.harness === 'claude') raw.push({ ordinal: raw.length, type: 'cost-state', ts: chat.ended, bytes: 2100, event: null })
  return { events, raw, prompts }
}

export function transcriptOf(chat: Chat): Transcript {
  const hit = cache.get(chat.id)
  if (hit) return hit
  const t = generate(chat)
  cache.set(chat.id, t)
  if (cache.size > 40) cache.delete(cache.keys().next().value!)
  return t
}

/** Searchable lines of a conversation, by who wrote them. */
export type Role = 'user' | 'agent' | 'tool' | 'thinking'

export function searchLines(chat: Chat): { role: Role; text: string; event: number }[] {
  // Search walks every conversation; don't push the reader's cache out for it.
  const { events } = cache.get(chat.id) ?? generate(chat)
  const out: { role: Role; text: string; event: number }[] = []
  events.forEach((e, i) => {
    if (e.k === 'user') out.push({ role: 'user', text: e.text, event: i })
    else if (e.k === 'assistant') out.push({ role: 'agent', text: e.md, event: i })
    else if (e.k === 'thinking') out.push({ role: 'thinking', text: e.text, event: i })
    else if (e.k === 'tools') for (const c of e.calls) out.push({ role: 'tool', text: `${c.arg}\n${c.output ?? ''}`, event: i })
  })
  return out
}
