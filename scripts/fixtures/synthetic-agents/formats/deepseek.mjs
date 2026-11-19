/**
 * Responsibilities: DeepSeek Harness sessions in format v4:
 * `sessions/--<slug>--/<id>/session.v4.jsonl.zstd`, a header frame and then
 * one zstd frame of events per turn.
 * Not responsible for: older or newer generations (scenes/deepseek.mjs).
 */

import path from 'node:path'
import { MODELS } from '../bank.mjs'
import { between, hex, later, pick, uuid7, zstd } from '../basics.mjs'
import { count, finish, put, rendered } from '../context.mjs'

/** DeepSeek Harness: separators to `-`, other unsafe UTF-16 units to `~XXXX`. */
function deepseekFolder(cwd) {
  const slug = cwd
    .replace(/^[/\\]+/, '')
    .replace(/[/\\:]+/g, '-')
    .replace(/[^A-Za-z0-9._-]/g, (unit) =>
      unit
        .split('')
        .map((code) => `~${code.charCodeAt(0).toString(16).padStart(4, '0')}`)
        .join(''),
    )
  return `--${slug.slice(0, 160)}--`
}

export function deepseekHeader(base, version) {
  return {
    type: 'session',
    version,
    id: base.id,
    createdAt: base.createdAt,
    cwd: base.cwd,
    isSeeded: false,
    delegationDepth: 0,
  }
}

/**
 * Renders turns as DeepSeek Harness logs them (format v4, the library's
 * lifecycle fixture): one appended batch, written as one zstd frame, per turn.
 */
export function deepseekTurns(base, turns, state) {
  const out = rendered([base.model])
  out.frames = []
  const event = (type, data, append = false) => ({
    type,
    seq: state.seq++,
    time: state.at,
    data,
    ...(append && { surfaceOp: 'append' }),
  })
  const id = (prefix) => `${prefix}_${hex(state.random, 16)}`
  for (const turn of turns) {
    later(state, 20, 240)
    state.turn += 1
    const t = state.turn
    let step = 1
    const frame = [
      event('turn/start', { turn: t }),
      event('step/start', { turn: t, step }),
    ]
    frame.push(
      event(
        'user/message',
        {
          id: id('msg'),
          role: 'user',
          source: { kind: 'user' },
          content: [{ type: 'text', text: turn.user }],
        },
        true,
      ),
    )
    frame.push(
      event('request/header', {
        reason: 'initial',
        header: { config: { provider: 'deepseek', model: base.model } },
      }),
    )
    out.firstUserMessage ??= turn.user
    out.user += 1
    let pending = []
    const flush = () => {
      if (pending.length === 0) return
      later(state, 3, 30)
      const usage = {
        inputTokens: between(state.random, 100, 3000),
        outputTokens: between(state.random, 50, 1200),
        cacheReadTokens: between(state.random, 0, 20000),
      }
      usage.totalTokens =
        usage.inputTokens + usage.outputTokens + usage.cacheReadTokens
      frame.push(
        event(
          'assistant/message',
          {
            turn: t,
            step,
            stream: [],
            message: {
              id: id('msg'),
              role: 'assistant',
              source: {
                kind: 'model',
                provider: 'deepseek',
                model: base.model,
              },
              content: pending,
            },
            usage,
          },
          true,
        ),
      )
      if (pending.some((block) => block.type === 'text')) out.assistant += 1
      out.usage.input += usage.inputTokens
      out.usage.output += usage.outputTokens
      out.usage.cacheRead += usage.cacheReadTokens
      pending = []
    }
    const nextStep = () => {
      frame.push(event('step/end', { turn: t, step }))
      step += 1
      frame.push(event('step/start', { turn: t, step }))
    }
    for (const item of turn.steps) {
      if (item.text) {
        if (pending.length > 0) {
          flush()
          nextStep()
        }
        pending.push({ type: 'text', text: item.text })
        continue
      }
      const callId = id('call')
      const name = item.tool === 'run' ? 'bash' : 'read'
      const args = JSON.stringify(
        item.tool === 'run' ? { command: item.arg } : { path: item.arg },
      )
      pending.push({ type: 'tool-call', id: callId, name, arguments: args })
      flush()
      frame.push(
        event('tool/call', { turn: t, step, callId, name, arguments: args }),
      )
      later(state, 1, 30)
      frame.push(
        event(
          'tool/result',
          {
            turn: t,
            step,
            message: {
              id: id('msg'),
              role: 'tool',
              source: { kind: 'tool', callId },
              toolCallId: callId,
              content: [{ type: 'text', text: item.out }],
            },
          },
          true,
        ),
      )
      nextStep()
    }
    flush()
    frame.push(event('step/end', { turn: t, step }))
    frame.push(event('turn/end', { turn: t, reason: { kind: 'completed' } }))
    if (t === 1 && base.title)
      frame.push(
        event('session/title', {
          title: base.title,
          messageSeqs: [],
          source: { kind: 'user' },
        }),
      )
    out.frames.push(frame)
    out.rows.push(...frame)
  }
  return out
}

export function deepseekSession(c, title = null) {
  return {
    id: uuid7(c.random, c.start),
    createdAt: c.start,
    cwd: c.cwd,
    model: pick(c.random, MODELS.deepseek),
    title,
  }
}

export function deepseekFile(ctx, base, name) {
  return path.join(
    ctx.roots.deepseek,
    'sessions',
    deepseekFolder(base.cwd),
    base.id,
    name,
  )
}

/** Header frame first, then one frame per turn, as the harness appends them. */
export function deepseekBytes(header, frames) {
  return Buffer.concat([zstd([header]), ...frames.map((frame) => zstd(frame))])
}

export function writeDeepseek(c, turns, { title } = {}) {
  const base = deepseekSession(c, title)
  c.state.seq = 0
  c.state.turn = 0
  const out = deepseekTurns(base, turns, c.state)
  count(c, out)
  const file = deepseekFile(c.ctx, base, 'session.v4.jsonl.zstd')
  put(
    c.ctx,
    'deepseek',
    'conversation',
    file,
    deepseekBytes(deepseekHeader(base, 4), out.frames),
    c.state.at,
  )
  c.files.push(file)
  finish(c, base.id, title)
  return { file, base, frames: out.frames }
}
