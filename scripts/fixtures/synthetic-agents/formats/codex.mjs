/**
 * Responsibilities: Codex rollouts as the tool writes them:
 * `sessions/YYYY/MM/DD/rollout-<local time>-<thread>.jsonl`, compressed once
 * older than a month, in the paginated record format.
 * Not responsible for: chains of rollouts or archiving (scenes/codex.mjs).
 */

import path from 'node:path'
import { MODELS } from '../bank.mjs'
import {
  ALNUM,
  between,
  chars,
  dayAt,
  hex,
  iso,
  jsonl,
  later,
  pick,
  uuid7,
  zstd,
} from '../basics.mjs'
import { count, finish, put, rendered } from '../context.mjs'

const CODEX_VERSION = '0.167.1'

/** Codex names rollouts and their day folders by local time. */
function codexStamp(at) {
  const day = new Date(at)
  const two = (value) => String(value).padStart(2, '0')
  const date = [
    String(day.getFullYear()),
    two(day.getMonth() + 1),
    two(day.getDate()),
  ]
  const time = [
    two(day.getHours()),
    two(day.getMinutes()),
    two(day.getSeconds()),
  ]
  return {
    folder: path.join(...date),
    stamp: `${date.join('-')}T${time.join('-')}`,
  }
}

function codexUsage(random) {
  const input = between(random, 4000, 30000)
  const output = between(random, 200, 3000)
  const cached = between(random, 0, input - 1000)
  const reasoning = between(random, 0, output)
  return {
    input_tokens: input,
    cached_input_tokens: cached,
    cache_write_input_tokens: 0,
    output_tokens: output,
    reasoning_output_tokens: reasoning,
    total_tokens: input + output,
  }
}

export function codexMeta(base, at, extra = {}) {
  return {
    timestamp: iso(at),
    type: 'session_meta',
    payload: {
      id: base.thread,
      session_id: base.thread,
      timestamp: iso(at),
      cwd: base.cwd,
      originator: base.desktop ? 'Codex Desktop' : 'codex_cli_rs',
      cli_version: CODEX_VERSION,
      source: base.desktop ? 'vscode' : 'cli',
      history_mode: 'paginated',
      model_provider: 'openai',
      ...extra,
    },
  }
}

/**
 * Renders turns the way a paginated Codex rollout records them (the
 * library's paginated.jsonl): the user message as a model input item and as a
 * completed UserMessage, tool calls as function calls, replies as completed
 * AgentMessage items, and token usage after each turn.
 */
export function codexTurns(base, turns, state) {
  const out = rendered([base.model])
  const row = (type, payload) => ({ timestamp: iso(state.at), type, payload })
  const completed = (turnId, item) =>
    row('event_msg', {
      type: 'item_completed',
      thread_id: base.thread,
      turn_id: turnId,
      item,
      completed_at_ms: state.at,
    })
  for (const turn of turns) {
    later(state, 20, 240)
    const turnId = uuid7(state.random, state.at)
    out.rows.push(
      row('turn_context', {
        cwd: base.cwd,
        model: base.model,
        turn_id: turnId,
      }),
    )
    out.rows.push(
      row('response_item', {
        type: 'message',
        role: 'user',
        content: [{ type: 'input_text', text: turn.user }],
      }),
    )
    out.rows.push(
      completed(turnId, {
        type: 'UserMessage',
        id: `item_${hex(state.random, 16)}`,
        content: [{ type: 'text', text: turn.user, text_elements: [] }],
      }),
    )
    out.firstUserMessage ??= turn.user
    out.user += 1
    turn.steps.forEach((step, index) => {
      later(state, 4, 40)
      if (step.text) {
        out.rows.push(
          completed(turnId, {
            type: 'AgentMessage',
            id: `item_${hex(state.random, 16)}`,
            content: [{ type: 'Text', text: step.text }],
            phase:
              index === turn.steps.length - 1 ? 'final_answer' : 'commentary',
          }),
        )
        out.assistant += 1
        return
      }
      const callId = `call_${chars(state.random, ALNUM, 24)}`
      const command =
        step.tool === 'run' ? step.arg : `sed -n '1,200p' ${step.arg}`
      out.rows.push(
        row('response_item', {
          type: 'function_call',
          id: `fc_${hex(state.random, 24)}`,
          call_id: callId,
          name: 'shell',
          arguments: JSON.stringify({
            command: ['bash', '-lc', command],
            workdir: base.cwd,
          }),
        }),
      )
      later(state, 1, 30)
      out.rows.push(
        row('response_item', {
          type: 'function_call_output',
          call_id: callId,
          output: step.out,
        }),
      )
    })
    const last = codexUsage(state.random)
    for (const [field, value] of Object.entries(last))
      state.totals[field] = (state.totals[field] ?? 0) + value
    out.usage.input += last.input_tokens
    out.usage.output += last.output_tokens
    out.usage.cacheRead += last.cached_input_tokens
    out.rows.push(
      row('event_msg', {
        type: 'token_count',
        info: {
          total_token_usage: { ...state.totals },
          last_token_usage: last,
          model_context_window: 272000,
        },
        rate_limits: null,
      }),
    )
  }
  return out
}

/** `sessions/YYYY/MM/DD/rollout-<local time>-<thread>[_<rollout>].jsonl[.zst]`. */
export function codexFile(
  ctx,
  at,
  name,
  { archived = false, compressed = false } = {},
) {
  const { folder, stamp } = codexStamp(at)
  const file = `rollout-${stamp}-${name}.jsonl${compressed ? '.zst' : ''}`
  return archived
    ? path.join(ctx.roots.codex, 'archived_sessions', file)
    : path.join(ctx.roots.codex, 'sessions', folder, file)
}

export function writeCodexRollout(c, file, rows) {
  const bytes = file.endsWith('.zst') ? zstd(rows) : jsonl(rows)
  put(c.ctx, 'codex', 'conversation', file, bytes, c.state.at)
  c.files.push(file)
}

/** A one-file Codex thread; rollouts older than a month are compressed. */
export function writeCodex(c, turns, { model, archived = false, edit } = {}) {
  const thread = uuid7(c.random, c.start)
  const base = {
    thread,
    cwd: c.cwd,
    model: model ?? pick(c.random, MODELS.codex),
    desktop: c.random() < 0.3,
  }
  c.state.totals = {}
  const out = codexTurns(base, turns, c.state)
  count(c, out)
  const rows = [codexMeta(base, c.start), ...out.rows]
  edit?.(rows)
  const compressed = archived || c.start < dayAt(c.ctx.today, 30, 0, 0)
  const file = codexFile(c.ctx, c.start, thread, { archived, compressed })
  writeCodexRollout(c, file, rows)
  finish(c, thread)
  return { file, thread, rows }
}
