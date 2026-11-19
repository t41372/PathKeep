/**
 * Responsibilities: pi sessions as SessionManager writes them (format v3):
 * `sessions/--<cwd>--/<time>_<id>.jsonl`, entries chained by `parentId`,
 * usage and cost on every reply.
 * Not responsible for: branches (scenes/pi.mjs).
 */

import path from 'node:path'
import { PI_MODELS } from '../bank.mjs'
import {
  ALNUM,
  between,
  chars,
  hex,
  iso,
  jsonl,
  later,
  pick,
  round,
  uuid7,
} from '../basics.mjs'
import { count, finish, put, rendered } from '../context.mjs'

/** pi: drop the leading separator, turn `/`, `\` and `:` into `-`. */
const piFolder = (cwd) =>
  `--${cwd.replace(/^[/\\]/, '').replace(/[/\\:]/g, '-')}--`

function piUsage(random, price) {
  const input = between(random, 200, 4000)
  const output = between(random, 60, 1500)
  const cacheRead = between(random, 0, 30000)
  const cacheWrite = between(random, 0, 3000)
  const cost = {
    input: round((input * price.input) / 1e6),
    output: round((output * price.output) / 1e6),
    cacheRead: round((cacheRead * price.cacheRead) / 1e6),
    cacheWrite: round((cacheWrite * price.cacheWrite) / 1e6),
  }
  cost.total = round(
    cost.input + cost.output + cost.cacheRead + cost.cacheWrite,
  )
  return {
    input,
    output,
    cacheRead,
    cacheWrite,
    totalTokens: input + output + cacheRead + cacheWrite,
    cost,
  }
}

/** One pi entry, a child of the previous one unless the caller moved `parent`. */
function piEntry(state, type, fields) {
  const id = hex(state.random, 8)
  const entry = {
    type,
    id,
    parentId: state.parent,
    timestamp: iso(state.at),
    ...fields,
  }
  state.parent = id
  return entry
}

/** Renders turns as pi's SessionManager appends them (session format v3). */
export function piTurns(base, turns, state) {
  const out = rendered([base.model.model])
  out.usage.cost = 0
  for (const turn of turns) {
    later(state, 20, 240)
    out.rows.push(
      piEntry(state, 'message', {
        message: {
          role: 'user',
          content: [{ type: 'text', text: turn.user }],
          timestamp: state.at,
        },
      }),
    )
    out.firstUserMessage ??= turn.user
    out.user += 1
    let pending = []
    const flush = (stopReason) => {
      if (pending.length === 0) return
      later(state, 3, 30)
      const usage = piUsage(state.random, base.model.price)
      out.rows.push(
        piEntry(state, 'message', {
          message: {
            role: 'assistant',
            content: pending,
            api: base.model.api,
            provider: base.model.provider,
            model: base.model.model,
            usage,
            stopReason,
            timestamp: state.at,
          },
        }),
      )
      if (pending.some((block) => block.type === 'text')) out.assistant += 1
      out.usage.input += usage.input
      out.usage.output += usage.output
      out.usage.cacheRead += usage.cacheRead
      out.usage.cacheWrite += usage.cacheWrite
      out.usage.cost = round(out.usage.cost + usage.cost.total)
      pending = []
    }
    for (const step of turn.steps) {
      if (step.text) {
        flush('stop')
        pending.push({ type: 'text', text: step.text })
        continue
      }
      const id = `toolu_01${chars(state.random, ALNUM, 22)}`
      const name = step.tool === 'run' ? 'bash' : 'read'
      const args =
        step.tool === 'run' ? { command: step.arg } : { path: step.arg }
      pending.push({ type: 'toolCall', id, name, arguments: args })
      flush('toolUse')
      later(state, 1, 30)
      out.rows.push(
        piEntry(state, 'message', {
          message: {
            role: 'toolResult',
            toolCallId: id,
            toolName: name,
            content: [{ type: 'text', text: step.out }],
            isError: false,
            timestamp: state.at,
          },
        }),
      )
    }
    flush('stop')
  }
  return out
}

export function piHeader(c, base) {
  return [
    {
      type: 'session',
      version: 3,
      id: base.id,
      timestamp: iso(c.start),
      cwd: c.cwd,
    },
    piEntry(c.state, 'model_change', {
      provider: base.model.provider,
      modelId: base.model.model,
    }),
  ]
}

export function piFile(ctx, cwd, start, id) {
  const stamp = iso(start).replace(/[:.]/g, '-')
  return path.join(
    ctx.roots.pi,
    'sessions',
    piFolder(cwd),
    `${stamp}_${id}.jsonl`,
  )
}

export function writePi(c, turns, { model } = {}) {
  const base = {
    id: uuid7(c.random, c.start),
    model: model ?? pick(c.random, PI_MODELS),
  }
  const rows = piHeader(c, base)
  const out = piTurns(base, turns, c.state)
  count(c, out)
  rows.push(...out.rows)
  const file = piFile(c.ctx, c.cwd, c.start, base.id)
  put(c.ctx, 'pi', 'conversation', file, jsonl(rows), c.state.at)
  c.files.push(file)
  finish(c, base.id)
  return { file, rows, base }
}
