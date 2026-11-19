/**
 * Responsibilities: the writing context and the conversation builder. Files
 * are written and dated here, counts, models and words are collected here,
 * and each tool's file facts (paths, bytes, dates) are read back from disk.
 * Not responsible for: any tool's record format or folder naming.
 */

import { mkdirSync, statSync, utimesSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { newWord, round, seeded, startOfDay } from './basics.mjs'
import { ROOTS, ROOT_KEYS, projectFolder } from './projects.mjs'

export function createContext(home, now, seed) {
  return {
    home,
    now,
    seed,
    today: startOfDay(now),
    taken: new Set(),
    roots: Object.fromEntries(
      ROOT_KEYS.map((root) => [root, path.join(home, ROOTS[root])]),
    ),
    files: Object.fromEntries(ROOT_KEYS.map((root) => [root, []])),
    conversations: [],
  }
}

/** Writes a file under a root, dated by its last record, and lists it in the facts. */
export function put(ctx, root, kind, file, bytes, at) {
  mkdirSync(path.dirname(file), { recursive: true })
  writeFileSync(file, bytes)
  utimesSync(file, at / 1000, at / 1000)
  if (!ctx.files[root].some((entry) => entry.path === file))
    ctx.files[root].push({ path: file, kind })
}

/** A conversation being written: its own random stream, clock and counts. */
export function begin(
  ctx,
  key,
  tool,
  project,
  start,
  random = seeded(ctx.seed, key),
) {
  return {
    ctx,
    key,
    tool,
    // The root its files go under; a Cowork task changes it to `claude-desktop`.
    root: tool,
    cwd: projectFolder(ctx.home, project),
    random,
    start,
    state: { at: start, random, parent: null },
    words: [],
    unreadWords: [],
    files: [],
    subagents: [],
    firstUserMessage: null,
    user: 0,
    assistant: 0,
    models: new Set(),
    usage: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
  }
}

export const word = (c) => {
  const value = newWord(c.ctx.taken, c.random)
  c.words.push(value)
  return value
}

export const unreadWord = (c) => {
  const value = newWord(c.ctx.taken, c.random)
  c.unreadWords.push(value)
  return value
}

/** Adds what a batch of rendered turns holds to the conversation's counts. */
export function count(c, rendered) {
  c.firstUserMessage ??= rendered.firstUserMessage
  c.user += rendered.user
  c.assistant += rendered.assistant
  for (const model of rendered.models) c.models.add(model)
  for (const [field, value] of Object.entries(rendered.usage))
    c.usage[field] = round((c.usage[field] ?? 0) + value)
}

export function finish(c, nativeId, title = null) {
  c.ctx.conversations.push({
    key: c.key,
    tool: c.tool,
    root: c.root,
    nativeId,
    projectFolder: c.cwd,
    files: c.files,
    firstUserMessage: c.firstUserMessage,
    title: title ?? c.firstUserMessage,
    userMessages: c.user,
    assistantMessages: c.assistant,
    models: [...c.models],
    subagents: c.subagents,
    words: c.words,
    unreadWords: c.unreadWords,
    startedAt: c.start,
    updatedAt: c.state.at,
    usage: c.usage,
    appendedTurns: 0,
  })
}

export function rendered(models) {
  return {
    rows: [],
    firstUserMessage: null,
    user: 0,
    assistant: 0,
    models,
    usage: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
  }
}

export function toolFacts(root, files) {
  const listed = files
    .map((entry) => {
      try {
        const stats = statSync(entry.path)
        return {
          ...entry,
          bytes: stats.size,
          modifiedAt: Math.round(stats.mtimeMs),
        }
      } catch {
        return null
      }
    })
    .filter(Boolean)
  return {
    root,
    conversationFiles: listed
      .filter((f) => f.kind === 'conversation')
      .map((f) => f.path),
    subagentFiles: listed
      .filter((f) => f.kind === 'subagent')
      .map((f) => f.path),
    files: listed,
    totalBytes: listed.reduce((sum, f) => sum + f.bytes, 0),
    earliestFileDate: Math.min(...listed.map((f) => f.modifiedAt)),
  }
}
