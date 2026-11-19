/**
 * Responsibilities: changing a written fixture between two backups the way
 * the tools would: appended turns, a new conversation, a deleted one, an
 * unreadable root. Each mutator returns new facts and leaves the ones it was
 * given alone.
 * Not responsible for: the first write (generate.mjs).
 */

import {
  appendFileSync,
  chmodSync,
  readFileSync,
  rmSync,
  utimesSync,
} from 'node:fs'
import path from 'node:path'
import { zstdDecompressSync } from 'node:zlib'
import { FOLLOW_UPS, PI_MODELS, TEMPLATES, turnsFrom } from './bank.mjs'
import {
  dayAt,
  jsonl,
  newWord,
  pick,
  round,
  seeded,
  startOfDay,
  zstd,
} from './basics.mjs'
import {
  begin,
  createContext,
  toolFacts,
  unreadWord,
  word,
} from './context.mjs'
import { claudeTurns } from './formats/claude-code.mjs'
import { codexTurns } from './formats/codex.mjs'
import { deepseekTurns } from './formats/deepseek.mjs'
import { piTurns } from './formats/pi.mjs'
import { WRITERS, agentConversation } from './generate.mjs'
import { AGENT_TOOLS, PROJECTS, ROOT_KEYS, TOOL_PROJECTS } from './projects.mjs'

/** Rebuilds the writing context for a mutation from the facts. */
function reopen(agents) {
  const ctx = createContext(agents.home, agents.now, agents.seed)
  ctx.roots = agents.roots
  for (const root of ROOT_KEYS)
    ctx.files[root] = agents.tools[root].files.map(({ path: p, kind }) => ({
      path: p,
      kind,
    }))
  for (const c of [...agents.conversations, ...agents.removed])
    for (const value of [
      ...c.words,
      ...c.unreadWords,
      ...c.subagents.flatMap((s) => s.words),
    ])
      ctx.taken.add(value)
  for (const value of Object.values(agents.scenes).flatMap((scene) => [
    ...(scene.words ?? []),
    ...(scene.revertedWords ?? []),
  ]))
    ctx.taken.add(value)
  return ctx
}

function settle(agents, ctx, conversations, removed = agents.removed) {
  const next = structuredClone(agents)
  next.conversations = conversations
  next.removed = removed
  for (const root of ROOT_KEYS)
    if (!agents.unreadable.includes(root))
      next.tools[root] = toolFacts(agents.roots[root], ctx.files[root])
  return next
}

/** Mutations happen late on the day before `now`, after every written record. */
function mutationTime(agents, after) {
  return Math.max(after + 60_000, dayAt(startOfDay(agents.now), 1, 23, 0))
}

const lines = (file) =>
  readFileSync(file, 'utf8')
    .split('\n')
    .filter((line) => line.trim())
    .map((line) => {
      try {
        return JSON.parse(line)
      } catch {
        return null
      }
    })
    .filter(Boolean)

/**
 * Appends `turns` follow-up turns (each with a new unique word) to the
 * conversation's current file, the way its tool would: Claude Code and pi
 * append lines, Codex appends to its current plain rollout, DeepSeek appends
 * a zstd frame. Returns the updated facts; `agents` is left unchanged.
 */
export function appendAgentTurns(agents, key, { turns = 1 } = {}) {
  const ctx = reopen(agents)
  const conversations = structuredClone(agents.conversations)
  const conversation = conversations.find((c) => c.key === key)
  if (!conversation) throw new Error(`no synthetic agent conversation ${key}`)
  const file = conversation.files.at(-1)
  const random = seeded(agents.seed, key, 'append', conversation.appendedTurns)
  const state = {
    at: mutationTime(agents, conversation.updatedAt),
    random,
    parent: null,
  }
  const added = Array.from({ length: turns }, (_, index) => {
    const value = newWord(ctx.taken, random)
    conversation.words.push(value)
    const [user, reply] =
      FOLLOW_UPS[(conversation.appendedTurns + index) % FOLLOW_UPS.length]
    return turnsFrom([[user, reply]], value)[0]
  })
  let out
  if (conversation.tool === 'claude-code') {
    const rows = lines(file)
    const last = rows.findLast((row) => row.uuid)
    state.parent = last?.uuid ?? null
    // A Cowork transcript keeps its own entrypoint and version.
    const base = {
      sessionId: conversation.nativeId,
      cwd: conversation.projectFolder,
      model: conversation.models.at(-1),
      entrypoint: last?.entrypoint,
      version: last?.version,
    }
    out = claudeTurns(base, added, state)
    appendFileSync(file, jsonl(out.rows))
  } else if (conversation.tool === 'codex') {
    if (file.endsWith('.zst'))
      throw new Error(
        `${key} is a compressed rollout; Codex writes to a plain one when a thread is resumed`,
      )
    const rows = lines(file)
    state.totals = {
      ...(rows.findLast((row) => row.payload?.type === 'token_count')?.payload
        .info.total_token_usage ?? {}),
    }
    const base = {
      thread: conversation.nativeId,
      cwd: conversation.projectFolder,
      model: conversation.models.at(-1),
    }
    out = codexTurns(base, added, state)
    appendFileSync(file, jsonl(out.rows))
  } else if (conversation.tool === 'pi') {
    const rows = lines(file)
    state.parent = rows.at(-1).id
    const model =
      PI_MODELS.find((m) => m.model === conversation.models.at(-1)) ??
      PI_MODELS[0]
    out = piTurns({ model }, added, state)
    appendFileSync(file, jsonl(out.rows))
  } else {
    const compressed = file.endsWith('.zstd')
    const bytes = readFileSync(file)
    const rows = (compressed ? zstdDecompressSync(bytes) : bytes)
      .toString('utf8')
      .split('\n')
      .filter(Boolean)
      .map((line) => JSON.parse(line))
    const events = rows.slice(1)
    state.seq = (events.at(-1)?.seq ?? -1) + 1
    state.turn = Math.max(0, ...events.map((event) => event.data?.turn ?? 0))
    const base = {
      id: conversation.nativeId,
      cwd: conversation.projectFolder,
      model: conversation.models.at(-1),
    }
    out = deepseekTurns(base, added, state)
    appendFileSync(file, compressed ? zstd(out.rows) : jsonl(out.rows))
  }
  utimesSync(file, state.at / 1000, state.at / 1000)
  conversation.userMessages += out.user
  conversation.assistantMessages += out.assistant
  for (const [field, value] of Object.entries(out.usage))
    conversation.usage[field] = round((conversation.usage[field] ?? 0) + value)
  conversation.updatedAt = state.at
  conversation.appendedTurns += turns
  return settle(agents, ctx, conversations)
}

/**
 * Writes a new conversation for `tool` late on the day before `now`, with a
 * unique word, as a tool does when a new session starts between two backups.
 */
export function addAgentConversation(
  agents,
  tool,
  { project, formatVersion } = {},
) {
  if (!AGENT_TOOLS.includes(tool)) throw new Error(`unknown agent tool ${tool}`)
  // Only pi names its format version in each session file.
  if (formatVersion !== undefined && tool !== 'pi')
    throw new Error(`${tool} sessions carry no format version of their own`)
  const ctx = reopen(agents)
  const taken = new Set(
    [...agents.conversations, ...agents.removed].map((c) => c.key),
  )
  let index = 1
  while (taken.has(`${tool}-added-${index}`)) index += 1
  const key = `${tool}-added-${index}`
  const random = seeded(agents.seed, key)
  const chosen = project ?? pick(random, TOOL_PROJECTS[tool])
  const latest = Math.max(...agents.conversations.map((c) => c.updatedAt))
  const c = begin(ctx, key, tool, chosen, mutationTime(agents, latest), random)
  const { stacks } = PROJECTS[chosen]
  const fitting = TEMPLATES.filter((t) =>
    t.stacks.some((s) => stacks.includes(s)),
  )
  if (fitting.length === 0)
    throw new Error(`the conversation bank has nothing for ${chosen}`)
  const template = pick(random, fitting)
  if (formatVersion === undefined) {
    WRITERS[tool](c, turnsFrom(template.turns, word(c)))
  } else {
    // A reader that does not know the format may not show its text.
    WRITERS[tool](c, turnsFrom(template.turns, unreadWord(c)), {
      version: formatVersion,
    })
    ctx.conversations.at(-1).formatVersion = formatVersion
  }
  return settle(agents, ctx, [
    ...structuredClone(agents.conversations),
    ...ctx.conversations,
  ])
}

/**
 * Deletes the conversation's files, as its tool does when the person deletes
 * it (Claude Code also drops the session's folder of subagents and tool
 * outputs; DeepSeek the session folder with every generation).
 */
export function deleteAgentConversation(agents, key) {
  const ctx = reopen(agents)
  const conversation = agentConversation(agents, key)
  const gone = [
    ...conversation.files,
    ...conversation.subagents.flatMap((s) => [s.file, s.metaFile]),
  ]
  for (const file of conversation.files) rmSync(file, { force: true })
  if (conversation.root === 'claude-desktop') {
    // The desktop app removes a Cowork task's card and its whole folder.
    const body = conversation.files[0].split(`${path.sep}.claude${path.sep}`)[0]
    for (const entry of ctx.files['claude-desktop'])
      if (
        entry.path === `${body}.json` ||
        entry.path.startsWith(`${body}${path.sep}`)
      )
        gone.push(entry.path)
    rmSync(`${body}.json`, { force: true })
    rmSync(body, { recursive: true, force: true })
  } else if (conversation.tool === 'claude-code') {
    const folder = conversation.files[0].replace(/\.jsonl$/, '')
    for (const entry of ctx.files['claude-code'])
      if (entry.path.startsWith(`${folder}${path.sep}`)) gone.push(entry.path)
    rmSync(folder, { recursive: true, force: true })
  }
  if (conversation.tool === 'deepseek')
    rmSync(path.dirname(conversation.files[0]), {
      recursive: true,
      force: true,
    })
  ctx.files[conversation.root] = ctx.files[conversation.root].filter(
    (entry) => !gone.includes(entry.path),
  )
  return settle(
    agents,
    ctx,
    structuredClone(agents.conversations.filter((c) => c.key !== key)),
    [...structuredClone(agents.removed), structuredClone(conversation)],
  )
}

/**
 * Takes away (or gives back) every permission on one root folder (a tool's,
 * or `claude-desktop`), so a backup cannot read it. POSIX only: on Windows `chmod` only sets the
 * read-only attribute, which does not stop reads (a test there would need an
 * ACL that denies the user), and for root nothing is denied at all.
 */
export function setAgentToolReadable(agents, root, readable) {
  if (process.platform === 'win32')
    throw new Error(
      'setAgentToolReadable needs POSIX permissions; Windows chmod does not stop reads',
    )
  if (process.getuid?.() === 0)
    throw new Error('setAgentToolReadable cannot deny anything to root')
  if (!(root in agents.roots)) throw new Error(`unknown root ${root}`)
  chmodSync(agents.roots[root], readable ? 0o755 : 0o000)
  const next = structuredClone(agents)
  next.unreadable = readable
    ? agents.unreadable.filter((r) => r !== root)
    : [...new Set([...agents.unreadable, root])]
  return next
}
