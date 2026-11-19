/**
 * Responsibilities: the Codex scenes: a thread continued over three rollouts
 * after two reverts, an archived thread, and a rollout with a line cut off
 * in the middle.
 * Not responsible for: the rollout format (formats/codex.mjs).
 */

import { MODELS, TEMPLATES, turnsFrom } from '../bank.mjs'
import { dayAt, jsonl, uuid7 } from '../basics.mjs'
import { begin, count, finish, unreadWord, word } from '../context.mjs'
import {
  codexFile,
  codexMeta,
  codexTurns,
  writeCodex,
  writeCodexRollout,
} from '../formats/codex.mjs'

/** A Codex thread that was reverted twice, leaving three rollout files. */
export function codexResumedScene(ctx) {
  const c = begin(
    ctx,
    'codex-resumed',
    'codex',
    'tidepool',
    dayAt(ctx.today, 9, 10, 5),
  )
  const thread = uuid7(c.random, c.start)
  const base = { thread, cwd: c.cwd, model: MODELS.codex[0], desktop: false }
  c.state.totals = {}
  const reply = (text) => ({ text })
  const turn = (user, ...steps) => ({ user, steps })
  const segment = (turns) => {
    const out = codexTurns(base, turns, c.state)
    return out
  }
  // First rollout: two turns kept, the third reverted.
  const [w1, w2, w3] = [word(c), word(c), word(c)]
  const [gone1, gone2] = [unreadWord(c), unreadWord(c)]
  const kept0 = segment([
    turn(
      `Port the tide prediction from predict.py to Rust. Call the module ${w1}.`,
      reply(
        `Ported it to src/${w1}.rs. The harmonic constants load from the same CSV.`,
      ),
    ),
    turn(
      'Compare the output with the Python version for station 9414290.',
      {
        tool: 'run',
        arg: 'cargo run -- 9414290 --days 3 > rust.txt && diff rust.txt python.txt',
        out: '',
      },
      reply('No differences over three days.'),
    ),
  ])
  const reverted0 = segment([
    turn(
      `Now make it async with ${gone1}.`,
      reply('Switched the loader to async I/O.'),
    ),
  ])
  const rows0 = [codexMeta(base, c.start), ...kept0.rows, ...reverted0.rows]
  const file0 = codexFile(ctx, c.start, thread)
  writeCodexRollout(c, file0, rows0)
  count(c, kept0)
  // Second rollout, a day later: inherits the kept part of the first.
  const start1 = dayAt(ctx.today, 8, 9, 40)
  c.state.at = start1
  const rollout1 = uuid7(c.random, start1)
  const keep0 = 1 + kept0.rows.length
  const kept1 = segment([
    turn(
      `Skip async. Cache the parsed constants in ${w2} instead.`,
      reply(
        'The constants are parsed once per process and kept in a OnceLock.',
      ),
    ),
  ])
  const reverted1 = segment([
    turn(
      `Also memoize the daily table with ${gone2}.`,
      reply('Added a small memo for the daily table.'),
    ),
  ])
  const rows1 = [
    codexMeta(base, start1, {
      history_base: {
        thread_id: thread,
        end_ordinal_exclusive: keep0,
        end_byte_offset: Buffer.byteLength(jsonl(rows0.slice(0, keep0))),
      },
    }),
    ...kept1.rows,
    ...reverted1.rows,
  ]
  const file1 = codexFile(ctx, start1, `${thread}_${rollout1}`)
  writeCodexRollout(c, file1, rows1)
  count(c, kept1)
  // Third rollout, three days after the first: inherits the kept part of the second.
  const start2 = dayAt(ctx.today, 6, 15, 20)
  c.state.at = start2
  const rollout2 = uuid7(c.random, start2)
  const keep1 = 1 + kept1.rows.length
  const kept2 = segment([
    turn(
      `Write a benchmark for ${w3} with criterion.`,
      reply('Added benches/predict.rs; a week of predictions takes 1.8 ms.'),
    ),
  ])
  const rows2 = [
    codexMeta(base, start2, {
      history_base: {
        thread_id: rollout1,
        end_ordinal_exclusive: keep0 + keep1,
        end_byte_offset: Buffer.byteLength(jsonl(rows1.slice(0, keep1))),
      },
    }),
    ...kept2.rows,
  ]
  const file2 = codexFile(ctx, start2, `${thread}_${rollout2}`)
  writeCodexRollout(c, file2, rows2)
  count(c, kept2)
  finish(c, thread)
  return {
    conversation: c.key,
    files: [file0, file1, file2],
    rolloutIds: [thread, rollout1, rollout2],
    revertedWords: [gone1, gone2],
  }
}

/** A Codex thread that was archived: moved and compressed. */
export function codexArchivedScene(ctx) {
  const c = begin(
    ctx,
    'codex-archived',
    'codex',
    'infra-scripts',
    dayAt(ctx.today, 95, 16, 45),
  )
  const w = word(c)
  const { file } = writeCodex(c, turnsFrom(TEMPLATES[12].turns, w), {
    archived: true,
  })
  return { conversation: c.key, file }
}

/** A Codex rollout with a half-written line in the middle. */
export function malformedScene(ctx) {
  const c = begin(
    ctx,
    'codex-malformed',
    'codex',
    'pathkeep',
    dayAt(ctx.today, 25, 11, 0),
  )
  const w = word(c)
  let line = -1
  const { file } = writeCodex(c, turnsFrom(TEMPLATES[0].turns, w), {
    model: MODELS.codex[0],
    edit: (rows) => {
      // A write cut short by a crash, followed by the records written after a restart.
      line = rows.findIndex(
        (row) => row.payload.type === 'function_call_output',
      )
      const text = JSON.stringify(rows[line])
      rows[line] = text.slice(0, Math.floor(text.length * 0.6))
    },
  })
  return { conversation: c.key, file, line }
}
