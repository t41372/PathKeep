/**
 * Responsibilities: the Claude desktop app's data folder (`Claude` beneath
 * the home under CHB_AGENT_TOOLS_HOME; Application Support/Claude on macOS):
 * a Desktop Code session card for a Claude Code session the app adopted from
 * the command line, and a local Cowork task with its card, the transcript
 * nested in its own `.claude`, its audit log and a file it produced.
 * Not responsible for: the transcript format (formats/claude-code.mjs). The
 * cards and the audit log carry only keys the research names
 * (docs/architecture/research/agent-history/claude-code.md, "Desktop Code and
 * Cowork are different collections"): a Desktop Code card `cliSessionId`,
 * `cwd`, `originCwd` and `adoptedFromOtherSurface` (it names a title too, but
 * not its key, so there is none); a Cowork card only `cliSessionId`; the
 * audit log `session_id`, `_audit_timestamp`, `_audit_hmac` and the types
 * `system` and `result`. No `.audit-key` is written. The library keeps cards
 * and the audit log as raw metadata.
 */

import path from 'node:path'
import { MODELS } from '../bank.mjs'
import { dayAt, hex, jsonl, seeded, uuid4 } from '../basics.mjs'
import { begin, count, finish, put, word } from '../context.mjs'
import { claudeFolder, claudeTurns } from '../formats/claude-code.mjs'

/** Claude Code version range the research saw in Cowork transcripts: 2.1.138–2.1.260. */
const COWORK_VERSION = '2.1.260'

/**
 * Written after every other conversation, from seeds of its own, so the rest
 * of the fixture is the same with or without it.
 */
export function claudeDesktopScene(ctx) {
  const random = seeded(ctx.seed, 'claude-desktop')
  const root = ctx.roots['claude-desktop']
  const [account, organization] = [uuid4(random), uuid4(random)]

  // Desktop Code lists a session started on the command line ("adopted").
  const adopted = ctx.conversations.find((c) => c.key === 'claude-subagents')
  const codeCard = path.join(
    root,
    'claude-code-sessions',
    account,
    organization,
    `local_${uuid4(random)}.json`,
  )
  const card = {
    cliSessionId: adopted.nativeId,
    cwd: adopted.projectFolder,
    originCwd: adopted.projectFolder,
    adoptedFromOtherSurface: true,
  }
  put(
    ctx,
    'claude-desktop',
    'session-card',
    codeCard,
    `${JSON.stringify(card)}\n`,
    adopted.updatedAt + 60_000,
  )

  // A local Cowork task: its transcript has entrypoint `local-agent`.
  const c = begin(
    ctx,
    'claude-cowork',
    'claude-code',
    'trip-notes',
    dayAt(ctx.today, 10, 19, 30),
  )
  c.root = 'claude-desktop'
  const w = word(c)
  const task = `local_${uuid4(c.random)}`
  const body = path.join(
    root,
    'local-agent-mode-sessions',
    account,
    organization,
    task,
  )
  const sessionId = uuid4(c.random)
  const base = {
    sessionId,
    cwd: c.cwd,
    model: MODELS['claude-code'][1],
    entrypoint: 'local-agent',
    version: COWORK_VERSION,
  }
  const plan = `# Hualien, 4–7 December\n\n1. Train from Taipei, Taroko Gorge in the afternoon.\n2. Qingshui Cliffs at sunrise.\n3. Rent bikes along the coast.\n4. Night market, then the evening train back.\n\nPlan ${w}.`
  const out = claudeTurns(
    base,
    [
      {
        user: `Plan a four-day trip to Hualien in early December from my notes, and save it as a document. Call the plan ${w}.`,
        steps: [
          { text: 'I will read your notes first.' },
          {
            tool: 'read',
            arg: 'notes.md',
            out: '- train, not the bus\n- Taroko if the trails are open\n- one slow day by the sea',
          },
          {
            text: `Saved the plan as outputs/hualien-plan.md: train on the first morning, Taroko the same afternoon, one day by the sea, and the night market before the train home (${w}).`,
          },
        ],
      },
    ],
    c.state,
  )
  count(c, out)
  c.start = Date.parse(out.rows[0].timestamp)
  const transcript = path.join(
    body,
    '.claude',
    'projects',
    claudeFolder(c.cwd),
    `${sessionId}.jsonl`,
  )
  put(
    ctx,
    'claude-desktop',
    'conversation',
    transcript,
    jsonl(out.rows),
    c.state.at,
  )
  c.files.push(transcript)
  const audit = path.join(body, 'audit.jsonl')
  const entry = (type, at) => ({
    type,
    session_id: sessionId,
    _audit_timestamp: new Date(at).toISOString(),
    _audit_hmac: hex(c.random, 64),
  })
  put(
    ctx,
    'claude-desktop',
    'audit',
    audit,
    jsonl([entry('system', c.start), entry('result', c.state.at)]),
    c.state.at,
  )
  const output = path.join(body, 'outputs', 'hualien-plan.md')
  put(ctx, 'claude-desktop', 'output', output, `${plan}\n`, c.state.at)
  const taskCard = `${body}.json`
  put(
    ctx,
    'claude-desktop',
    'session-card',
    taskCard,
    `${JSON.stringify({ cliSessionId: sessionId })}\n`,
    c.state.at,
  )
  finish(c, sessionId)
  return {
    desktopCode: { card: codeCard, conversation: adopted.key },
    cowork: {
      conversation: c.key,
      card: taskCard,
      body,
      transcript,
      audit,
      output,
    },
  }
}
