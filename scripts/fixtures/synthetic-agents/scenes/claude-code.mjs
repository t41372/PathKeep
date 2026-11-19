/**
 * Responsibilities: the Claude Code scenes: three subagents, a session of
 * about 2 MB, a pasted screenshot with a long and a spilled tool output,
 * unknown record types, a deleted project folder, a renamed session and an
 * empty transcript.
 * Not responsible for: the plain transcript format (formats/claude-code.mjs).
 */

import { statSync } from 'node:fs'
import path from 'node:path'
import { crc32, deflateSync } from 'node:zlib'
import {
  CLAUDE_SUBAGENT_MODEL,
  MODELS,
  TEMPLATES,
  turnsFrom,
} from '../bank.mjs'
import {
  ALNUM,
  chars,
  dayAt,
  hex,
  jsonl,
  later,
  newWord,
  seeded,
  uuid4,
} from '../basics.mjs'
import { begin, count, finish, put, unreadWord, word } from '../context.mjs'
import {
  claudeMainFile,
  claudeRecord,
  claudeTurns,
  claudeUsage,
  writeClaude,
} from '../formats/claude-code.mjs'
import { projectFolder } from '../projects.mjs'

/** A PNG of a page whose fixed header covers the first line of a post. */
function screenshot() {
  const [width, height] = [96, 64]
  const row = width * 3 + 1
  const raw = Buffer.alloc(row * height)
  for (let y = 0; y < height; y += 1)
    for (let x = 0; x < width; x += 1) {
      const header = y < 14
      const text = !header && y % 8 < 3 && x > 8 && x < 80 - (y % 24)
      const color = header
        ? [32, 45, 78]
        : text
          ? [60, 60, 60]
          : [250, 250, 247]
      raw.set(color, y * row + 1 + x * 3)
    }
  const chunk = (type, data) => {
    const length = Buffer.alloc(4)
    length.writeUInt32BE(data.length)
    const body = Buffer.concat([Buffer.from(type), data])
    const sum = Buffer.alloc(4)
    sum.writeUInt32BE(crc32(body))
    return Buffer.concat([length, body, sum])
  }
  const header = Buffer.alloc(13)
  header.writeUInt32BE(width, 0)
  header.writeUInt32BE(height, 4)
  header.set([8, 2, 0, 0, 0], 8)
  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    chunk('IHDR', header),
    chunk('IDAT', deflateSync(raw)),
    chunk('IEND', Buffer.alloc(0)),
  ])
}

/** A Claude Code session that hands three areas to three subagents. */
export function subagentScene(ctx) {
  const c = begin(
    ctx,
    'claude-subagents',
    'claude-code',
    'pathkeep',
    dayAt(ctx.today, 2, 14, 10),
  )
  const w = word(c)
  const sessionId = uuid4(c.random)
  const base = { sessionId, cwd: c.cwd, model: MODELS['claude-code'][0] }
  const opening = claudeTurns(
    base,
    [
      {
        user: `Before the ${w} release, check the scheduler, the importer and the restore path for anything that can lose data. Split it up.`,
        steps: [
          {
            text: 'I will give each area to its own agent so they read in parallel, then put the findings together.',
          },
        ],
      },
    ],
    c.state,
  )
  count(c, opening)
  const rows = [...opening.rows]
  const areas = [
    [
      'scheduler',
      'src-tauri/src/scheduler/mod.rs',
      'A run that starts while the laptop sleeps is marked done without copying anything.',
    ],
    [
      'importer',
      'src-tauri/src/import/queue.rs',
      'A file that fails twice is dropped from the queue with no record of it.',
    ],
    [
      'restore path',
      'src-tauri/src/restore/mod.rs',
      'Restore replaces the archive before checking the copy, so a bad copy loses both.',
    ],
  ]
  const response = {
    id: `msg_01${chars(c.random, ALNUM, 22)}`,
    usage: claudeUsage(c.random),
  }
  const calls = areas.map(([area]) => {
    later(c.state, 1, 3)
    const id = `toolu_01${chars(c.random, ALNUM, 22)}`
    const prompt = `Read the ${area} code and report every path where a backup can be skipped or lost without an error. Quote file and line.`
    const record = claudeRecord(base, c.state, 'assistant', {
      id: response.id,
      type: 'message',
      role: 'assistant',
      model: base.model,
      content: [
        {
          type: 'tool_use',
          id,
          name: 'Task',
          input: {
            description: `Audit the ${area}`,
            prompt,
            subagent_type: 'general-purpose',
          },
        },
      ],
      usage: response.usage,
    })
    rows.push(record)
    return { id, prompt, uuid: record.uuid, at: c.state.at }
  })
  const folder = path.join(
    path.dirname(claudeMainFile(ctx, c.cwd, sessionId)),
    sessionId,
    'subagents',
  )
  areas.forEach(([area, file, finding], index) => {
    const call = calls[index]
    const agentId = hex(c.random, 8)
    const own = newWord(ctx.taken, c.random)
    const state = { at: call.at + 2000, random: c.random, parent: null }
    const sub = claudeTurns(
      { sessionId, cwd: c.cwd, model: CLAUDE_SUBAGENT_MODEL, agentId },
      [
        {
          user: call.prompt,
          steps: [
            {
              text: `Starting with \`${own}\` in ${file}, the function that decides what the ${area} does next.`,
            },
            {
              tool: 'read',
              arg: file,
              out: `// ${area}\npub fn ${own}(state: &State) -> Next {\n    if state.paused { return Next::Done }\n    Next::Copy\n}`,
            },
            { text: `${finding} See ${file}:3.` },
          ],
        },
      ],
      state,
    )
    const transcript = path.join(folder, `agent-${agentId}.jsonl`)
    const meta = path.join(folder, `agent-${agentId}.meta.json`)
    put(ctx, 'claude-code', 'subagent', transcript, jsonl(sub.rows), state.at)
    put(
      ctx,
      'claude-code',
      'subagent-meta',
      meta,
      `${JSON.stringify({ toolUseId: call.id })}\n`,
      state.at,
    )
    c.subagents.push({
      agentId,
      file: transcript,
      metaFile: meta,
      toolUseId: call.id,
      firstUserMessage: call.prompt,
      userMessages: sub.user,
      assistantMessages: sub.assistant,
      models: [CLAUDE_SUBAGENT_MODEL],
      words: [own],
    })
    call.done = state.at
    call.finding = `${finding} See ${file}:3.`
  })
  for (const call of calls) {
    c.state.at = Math.max(c.state.at, call.done) + 1000
    c.state.parent = call.uuid
    rows.push(
      claudeRecord(base, c.state, 'user', {
        role: 'user',
        content: [
          {
            tool_use_id: call.id,
            type: 'tool_result',
            content: [{ type: 'text', text: call.finding }],
          },
        ],
      }),
    )
  }
  later(c.state, 10, 40)
  const closing = claudeUsage(c.random)
  rows.push(
    claudeRecord(base, c.state, 'assistant', {
      id: `msg_01${chars(c.random, ALNUM, 22)}`,
      type: 'message',
      role: 'assistant',
      model: base.model,
      content: [
        {
          type: 'text',
          text: `Three problems, one per area:\n\n1. ${calls[0].finding}\n2. ${calls[1].finding}\n3. ${calls[2].finding}\n\nThe restore one can lose data outright; I would fix it before ${w}.`,
        },
      ],
      usage: closing,
    }),
  )
  for (const usage of [response.usage, closing]) {
    c.usage.input += usage.input_tokens
    c.usage.output += usage.output_tokens
    c.usage.cacheRead += usage.cache_read_input_tokens
    c.usage.cacheWrite += usage.cache_creation_input_tokens
  }
  c.assistant += 1
  c.start = Date.parse(rows[0].timestamp)
  const file = claudeMainFile(ctx, c.cwd, sessionId)
  put(ctx, 'claude-code', 'conversation', file, jsonl(rows), c.state.at)
  c.files.push(file)
  finish(c, sessionId)
  return {
    conversation: c.key,
    words: [...c.words, ...c.subagents.flatMap((s) => s.words)],
  }
}

/** A long Claude Code session of benchmark runs, about 2 MB. */
export function largeScene(ctx) {
  const c = begin(
    ctx,
    'claude-large',
    'claude-code',
    'pathkeep-search',
    dayAt(ctx.today, 15, 8, 30),
  )
  const prompts = [
    'Run the search benchmark again and tell me what changed.',
    'Profile the tokenizer on the big fixture and run it once more.',
    'Why did the p95 go up in that run? Run it again with the cache warm.',
    'Try the prefix index and run the benchmark once more.',
    'Check memory while indexing 100k pages, then rerun the benchmark.',
    'Compare with the run from this morning.',
  ]
  const total = 112
  const middle = Math.floor(total / 2)
  const words = { [0]: word(c), [middle]: word(c), [total - 1]: word(c) }
  const turns = Array.from({ length: total }, (_, index) => {
    const lines = Array.from({ length: 220 }, (__, line) => {
      const time = (1 + c.random() * 3).toFixed(4)
      const change = (c.random() * 6 - 3).toFixed(2)
      return `search/${['prefix', 'phrase', 'fuzzy', 'cjk'][line % 4]}/${line.toString().padStart(3, '0')}  time: [${time} ms]  change: [${change}%] (p = ${c.random().toFixed(2)})`
    })
    const marker = words[index] ? ` (${words[index]})` : ''
    return {
      user: `${prompts[index % prompts.length]}${marker}`,
      steps: [
        { text: `Running run ${index + 1} now.` },
        {
          tool: 'run',
          arg: `cargo bench --bench search -- --save-baseline run-${index + 1}`,
          out: lines.join('\n'),
        },
        {
          text: `Run ${index + 1}: the prefix group moved by ${(c.random() * 4 - 2).toFixed(1)} %, within noise; nothing else changed.`,
        },
      ],
    }
  })
  const { file } = writeClaude(c, turns, { model: MODELS['claude-code'][1] })
  return {
    conversation: c.key,
    bytes: statSync(file).size,
    turns: total,
    firstWord: words[0],
    middleWord: words[middle],
    lastWord: words[total - 1],
  }
}

/** A Claude Code session with a pasted screenshot and two long tool outputs. */
export function attachmentScene(ctx) {
  const c = begin(
    ctx,
    'claude-attachments',
    'claude-code',
    'notes-site',
    dayAt(ctx.today, 7, 16, 40),
  )
  // The second word ends the link report, which only the spilled file keeps whole.
  const [w1, w2] = [word(c), unreadWord(c)]
  const image = screenshot()
  const pages = Array.from(
    { length: 300 },
    (_, index) =>
      `  ├─ /posts/${2019 + (index % 8)}/note-${String(index).padStart(3, '0')}/index.html (+${3 + (index % 17)}ms)`,
  )
  const build = `> notes-site@1.4.0 build\n> astro build\n\n14:02:11 [build] output: "static"\n14:02:11 [build] Collecting build info...\n14:02:13 [build] ✓ Completed in 1.62s.\n14:02:13 [build] Building static entrypoints...\n${pages.join('\n')}\n14:02:31 [build] ${pages.length} page(s) built in 19.80s\n14:02:31 [build] Complete!`
  const links = Array.from(
    { length: 900 },
    (_, index) =>
      `${index % 211 === 7 ? '[404]' : '[200]'} https://notes.example.com/posts/${2019 + (index % 8)}/note-${String(index % 300).padStart(3, '0')}/#section-${index % 9}`,
  )
  const linkReport = `${links.join('\n')}\nScanned ${links.length} links, 5 broken (${w2}).`
  const spill = path.join(
    path.dirname(claudeMainFile(ctx, c.cwd, '-')),
    '{session}',
    'tool-results',
    `${chars(c.random, 'abcdefghijklmnopqrstuvwxyz0123456789', 9)}.txt`,
  )
  const turns = [
    {
      user: `This screenshot is from my phone: the header covers the first line of every post. Fix it (${w1}).`,
      steps: [
        {
          text: 'The header is position: fixed and the main column has no top padding, so the first line sits under it. I added padding equal to the header height.',
        },
        { tool: 'run', arg: 'npm run build', out: build },
        {
          text: `The build passes and all ${pages.length} pages are generated.`,
        },
      ],
    },
    {
      user: 'Run the link checker over the whole site.',
      steps: [
        { tool: 'run', arg: 'npx linkinator dist --recurse', out: linkReport },
        {
          text: 'Five links point at sections that were renamed. I updated the anchors.',
        },
      ],
    },
  ]
  turns[0].content = [
    {
      type: 'image',
      source: {
        type: 'base64',
        media_type: 'image/png',
        data: image.toString('base64'),
      },
    },
    { type: 'text', text: turns[0].user },
  ]
  let spillFile = null
  const { file, rows } = writeClaude(c, turns, {
    model: MODELS['claude-code'][0],
    edit: (records, base) => {
      // Claude Code keeps an output this long in tool-results/ and puts a preview in the transcript.
      spillFile = spill.replace('{session}', base.sessionId)
      const kb = (Buffer.byteLength(linkReport) / 1024).toFixed(1)
      const preview = `<persisted-output>\nOutput too large (${kb}KB). Full output saved to: ${spillFile}\n\nPreview (first 2KB):\n${linkReport.slice(0, 2048)}\n...\n</persisted-output>`
      const record = records.find(
        (row) => row.message.content?.[0]?.tool_use_id === turns[1].steps[0].id,
      )
      record.message.content[0].content = preview
    },
  })
  put(
    ctx,
    'claude-code',
    'tool-output',
    spillFile,
    linkReport,
    statSync(file).mtimeMs,
  )
  const longOutput = rows.find(
    (row) => row.message.content?.[0]?.tool_use_id === turns[0].steps[1].id,
  )
  return {
    conversation: c.key,
    image: {
      mediaType: 'image/png',
      bytes: image.length,
      width: 96,
      height: 64,
    },
    longOutput: {
      toolUseId: turns[0].steps[1].id,
      bytes: Buffer.byteLength(longOutput.message.content[0].content),
    },
    spilledOutput: {
      toolUseId: turns[1].steps[0].id,
      file: spillFile,
      bytes: Buffer.byteLength(linkReport),
      word: w2,
    },
  }
}

/** A Claude Code session with record types the library has no reading for. */
export function unknownEventsScene(ctx) {
  const c = begin(
    ctx,
    'claude-unknown-events',
    'claude-code',
    'pathkeep',
    dayAt(ctx.today, 20, 11, 15),
  )
  const w = word(c)
  const records = []
  writeClaude(c, turnsFrom(TEMPLATES[15].turns, w), {
    edit: (rows, base) => {
      const at = rows[1].timestamp
      const added = [
        {
          type: 'session-checkpoint',
          uuid: uuid4(c.random),
          parentUuid: rows[0].uuid,
          sessionId: base.sessionId,
          timestamp: at,
          checkpoint: {
            reason: 'auto',
            files: ['src/import/queue.rs', 'src/import/mod.rs'],
            sizes: { before: 18233, after: 18011 },
            note: '保留原樣 – keep as written ✓',
          },
        },
        {
          type: 'x-review-note',
          sessionId: base.sessionId,
          timestamp: at,
          review: {
            verdict: 'needs-changes',
            comments: [{ line: 42, text: 'Why two seconds?' }],
            nested: { values: [1, 2.5, true, null, 'ünïcødé'] },
          },
        },
      ]
      rows.splice(1, 0, ...added)
      added.forEach((record, index) =>
        records.push({ line: 1 + index, type: record.type, record }),
      )
    },
  })
  return { conversation: c.key, records }
}

/** A Claude Code session in a project folder that has since been deleted. */
export function missingFolderScene(ctx) {
  const c = begin(
    ctx,
    'claude-missing-folder',
    'claude-code',
    'old-prototype',
    dayAt(ctx.today, 118, 13, 20),
  )
  const w = word(c)
  writeClaude(c, turnsFrom(TEMPLATES[6].turns, w))
  return { conversation: c.key, folder: c.cwd }
}

/** A Claude Code session the person renamed (`/rename` writes `custom-title`). */
export function customTitleScene(ctx) {
  const c = begin(
    ctx,
    'claude-custom-title',
    'claude-code',
    'ledger',
    dayAt(ctx.today, 40, 20, 5),
  )
  const w = word(c)
  const title = '匯出金額四捨五入修正'
  writeClaude(c, turnsFrom(TEMPLATES[8].turns, w), { title })
  return { conversation: c.key, title }
}

/** An empty transcript, as Claude Code leaves when a session ends before its first message. */
export function emptyFileScene(ctx) {
  const random = seeded(ctx.seed, 'claude-empty')
  const file = claudeMainFile(
    ctx,
    projectFolder(ctx.home, 'pathkeep'),
    uuid4(random),
  )
  put(ctx, 'claude-code', 'empty', file, '', dayAt(ctx.today, 1, 18, 20))
  return { file }
}
