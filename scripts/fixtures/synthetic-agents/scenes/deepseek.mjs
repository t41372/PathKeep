/**
 * Responsibilities: the DeepSeek Harness scenes: a session that moved from
 * format v3 to v4, one left in v3 without compression, and one in a version
 * newer than v4.
 * Not responsible for: the v4 format itself (formats/deepseek.mjs).
 */

import { MODELS } from '../bank.mjs'
import { dayAt, jsonl } from '../basics.mjs'
import { begin, count, finish, put, unreadWord, word } from '../context.mjs'
import {
  deepseekBytes,
  deepseekFile,
  deepseekHeader,
  deepseekSession,
  deepseekTurns,
} from '../formats/deepseek.mjs'

/** A DeepSeek session started under format v3 and continued under v4. */
export function deepseekCurrentScene(ctx) {
  const c = begin(
    ctx,
    'deepseek-current',
    'deepseek',
    'ledger',
    dayAt(ctx.today, 31, 14, 0),
  )
  const [w1, w2, w3] = [word(c), word(c), word(c)]
  const base = deepseekSession(c, '月結報表')
  base.model = MODELS.deepseek[0]
  c.state.seq = 0
  c.state.turn = 0
  const first = deepseekTurns(
    base,
    [
      {
        user: `幫我寫月結報表：每個分類的支出加總，輸出成 CSV，檔名用 ${w1}。`,
        steps: [
          { text: '寫好了 report.py：讀當月的帳目，按分類加總，寫成 CSV。' },
        ],
      },
    ],
    c.state,
  )
  const older = deepseekFile(ctx, base, 'session.v3.jsonl.zstd')
  put(
    ctx,
    'deepseek',
    'conversation',
    older,
    deepseekBytes(deepseekHeader(base, 3), first.frames),
    c.state.at,
  )
  c.state.at = dayAt(ctx.today, 5, 10, 20)
  const rest = deepseekTurns(
    base,
    [
      {
        user: `加上跟上個月的比較，差額超過兩成的分類標紅（${w2}）。`,
        steps: [
          {
            tool: 'run',
            arg: 'python report.py --month 2026-10',
            out: '分類,本月,上月,差額\n餐飲,8420,7980,+5.5%\n交通,3120,2110,+47.9%\n',
          },
          { text: '交通比上個月多了 47.9%，在 CSV 裡加了一欄標記。' },
        ],
      },
      {
        user: `最後把 ${w3} 這個暫時的檔名改回 monthly.csv。`,
        steps: [{ text: '改好了，輸出檔名是 monthly.csv。' }],
      },
    ],
    c.state,
  )
  const frames = [...first.frames, ...rest.frames]
  const file = deepseekFile(ctx, base, 'session.v4.jsonl.zstd')
  put(
    ctx,
    'deepseek',
    'conversation',
    file,
    deepseekBytes(deepseekHeader(base, 4), frames),
    c.state.at,
  )
  count(c, first)
  count(c, rest)
  c.files.push(older, file)
  finish(c, base.id, base.title)
  return {
    conversation: c.key,
    file,
    olderGeneration: older,
    frames: frames.length + 1,
  }
}

/** A DeepSeek session in format v3, written without compression. */
export function deepseekPlainScene(ctx) {
  const c = begin(
    ctx,
    'deepseek-plain',
    'deepseek',
    'tidepool',
    dayAt(ctx.today, 110, 15, 30),
  )
  const [w1, w2] = [word(c), word(c)]
  const base = deepseekSession(c, 'Station list cleanup')
  c.state.seq = 0
  c.state.turn = 0
  const out = deepseekTurns(
    base,
    [
      {
        user: `The station list has duplicates. Remove them and keep the newest entry (${w1}).`,
        steps: [
          {
            text: 'Deduplicated by station id, keeping the row with the latest survey date. 1,204 stations left.',
          },
        ],
      },
      {
        user: `Sort it by name and save as stations-${w2}.csv.`,
        steps: [{ text: 'Sorted by name and saved.' }],
      },
    ],
    c.state,
  )
  count(c, out)
  const file = deepseekFile(ctx, base, 'session.v3.jsonl')
  put(
    ctx,
    'deepseek',
    'conversation',
    file,
    jsonl([deepseekHeader(base, 3), ...out.rows]),
    c.state.at,
  )
  c.files.push(file)
  finish(c, base.id, base.title)
  return { conversation: c.key, file, version: 3 }
}

/** A DeepSeek session in a format version newer than any the library knows. */
export function newerVersionScene(ctx) {
  const c = begin(
    ctx,
    'deepseek-newer',
    'deepseek',
    'tidepool',
    dayAt(ctx.today, 3, 16, 0),
  )
  const w = unreadWord(c)
  const base = deepseekSession(c)
  c.state.seq = 0
  c.state.turn = 0
  const out = deepseekTurns(
    base,
    [
      {
        user: `Plot the next week of tides for ${w} as an SVG.`,
        steps: [{ text: 'Wrote tides.svg with the seven days side by side.' }],
      },
    ],
    c.state,
  )
  count(c, out)
  out.frames[0].splice(-1, 0, {
    type: 'plan/update',
    seq: 0,
    time: c.state.at,
    data: { items: [{ text: 'Plot tides', done: true }] },
  })
  out.frames[0].forEach((event, index) => (event.seq = index))
  const file = deepseekFile(ctx, base, 'session.v5.jsonl.zstd')
  put(
    ctx,
    'deepseek',
    'conversation',
    file,
    deepseekBytes(deepseekHeader(base, 5), out.frames),
    c.state.at,
  )
  c.files.push(file)
  finish(c, base.id)
  return { conversation: c.key, file, version: 5 }
}
