/**
 * Responsibilities: Claude Code transcripts as the tool writes them:
 * `projects/<encoded cwd>/<session>.jsonl`, one record per content block,
 * chained by `parentUuid`.
 * Not responsible for: subagents, spilled outputs or other scene records
 * (scenes/claude-code.mjs).
 */

import { createHash } from 'node:crypto'
import path from 'node:path'
import { MODELS } from '../bank.mjs'
import {
  ALNUM,
  between,
  chars,
  iso,
  jsonl,
  later,
  pick,
  uuid4,
} from '../basics.mjs'
import { count, finish, put, rendered } from '../context.mjs'

const CLAUDE_VERSION = '2.1.322'

/** Claude Code's project folder: every non-alphanumeric character becomes `-`. */
export function claudeFolder(cwd) {
  const name = cwd.replace(/[^a-zA-Z0-9]/g, '-')
  if (name.length <= 200) return name
  const hash = createHash('sha256').update(cwd).digest('hex').slice(0, 8)
  return `${name.slice(0, 200)}-${hash}`
}

/** One transcript record, chained to the previous one by `parentUuid`. */
export function claudeRecord(base, state, type, message, chain = true) {
  const uuid = uuid4(state.random)
  const record = {
    parentUuid: state.parent,
    isSidechain: Boolean(base.agentId),
    ...(base.agentId && { agentId: base.agentId }),
    cwd: base.cwd,
    sessionId: base.sessionId,
    version: base.version ?? CLAUDE_VERSION,
    // `local-agent` for a Cowork task in the Claude desktop app.
    entrypoint: base.entrypoint ?? 'cli',
    type,
    message,
    uuid,
    timestamp: iso(state.at),
  }
  if (chain) state.parent = uuid
  return record
}

export function claudeUsage(random) {
  return {
    input_tokens: between(random, 3, 40),
    cache_creation_input_tokens: between(random, 0, 4000),
    cache_read_input_tokens: between(random, 8000, 60000),
    output_tokens: between(random, 20, 900),
  }
}

function claudeToolCall(base, step) {
  return step.tool === 'run'
    ? { name: 'Bash', input: { command: step.arg } }
    : { name: 'Read', input: { file_path: path.join(base.cwd, step.arg) } }
}

/** Claude Code's Read output numbers the lines like `cat -n`. */
const numbered = (text) =>
  text
    .split('\n')
    .map((line, index) => `${String(index + 1).padStart(6)}\t${line}`)
    .join('\n')

/**
 * Renders turns as Claude Code writes them: every content block of a
 * response is its own record sharing the response's `message.id` and usage.
 */
export function claudeTurns(base, turns, state) {
  const out = rendered([base.model])
  const usage = (response) => {
    if (response.counted) return
    response.counted = true
    out.usage.input += response.usage.input_tokens
    out.usage.output += response.usage.output_tokens
    out.usage.cacheRead += response.usage.cache_read_input_tokens
    out.usage.cacheWrite += response.usage.cache_creation_input_tokens
  }
  for (const turn of turns) {
    later(state, 20, 240)
    out.rows.push(
      claudeRecord(base, state, 'user', {
        role: 'user',
        content: turn.content ?? turn.user,
      }),
    )
    out.firstUserMessage ??= turn.user
    out.user += 1
    let response = null
    for (const step of turn.steps) {
      later(state, 4, 40)
      response ??= {
        id: `msg_01${chars(state.random, ALNUM, 22)}`,
        usage: claudeUsage(state.random),
      }
      const message = (content) => ({
        id: response.id,
        type: 'message',
        role: 'assistant',
        model: base.model,
        content,
        usage: response.usage,
      })
      usage(response)
      if (step.text) {
        out.rows.push(
          claudeRecord(
            base,
            state,
            'assistant',
            message([{ type: 'text', text: step.text }]),
          ),
        )
        out.assistant += 1
        continue
      }
      const id = `toolu_01${chars(state.random, ALNUM, 22)}`
      step.id = id
      const call = claudeToolCall(base, step)
      out.rows.push(
        claudeRecord(
          base,
          state,
          'assistant',
          message([{ type: 'tool_use', id, ...call }]),
        ),
      )
      later(state, 1, 30)
      const result =
        step.result ?? (step.tool === 'read' ? numbered(step.out) : step.out)
      out.rows.push(
        claudeRecord(base, state, 'user', {
          role: 'user',
          content: [
            {
              tool_use_id: id,
              type: 'tool_result',
              content: result,
              is_error: false,
            },
          ],
        }),
      )
      response = null
    }
  }
  return out
}

export function claudeMainFile(ctx, cwd, sessionId) {
  return path.join(
    ctx.roots['claude-code'],
    'projects',
    claudeFolder(cwd),
    `${sessionId}.jsonl`,
  )
}

/** A plain Claude Code conversation; `edit` may add records before writing. */
export function writeClaude(c, turns, { model, title, edit } = {}) {
  const sessionId = uuid4(c.random)
  const base = {
    sessionId,
    cwd: c.cwd,
    model: model ?? pick(c.random, MODELS['claude-code']),
  }
  const out = claudeTurns(base, turns, c.state)
  count(c, out)
  const rows = out.rows
  edit?.(rows, base)
  if (title) rows.push({ type: 'custom-title', sessionId, customTitle: title })
  // Claude Code has no header record: the session starts with its first message.
  c.start = Date.parse(rows[0].timestamp)
  const file = claudeMainFile(c.ctx, c.cwd, sessionId)
  put(c.ctx, 'claude-code', 'conversation', file, jsonl(rows), c.state.at)
  c.files.push(file)
  finish(c, sessionId, title)
  return { file, sessionId, rows, base }
}
