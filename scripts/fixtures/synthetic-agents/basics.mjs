/**
 * Responsibilities: the helpers every part of the fixture shares: seeded
 * random streams, ids, local dates, JSONL and zstd encoding, and the made-up
 * words that make each conversation findable by search.
 * Not responsible for: any tool's format, the fake home's layout, or facts.
 */

import { createHash } from 'node:crypto'
import { zstdCompressSync } from 'node:zlib'

export const DAYS = 120

export function seeded(...parts) {
  const digest = createHash('sha256').update(parts.join('\u0000')).digest()
  let state = (digest.readUInt32LE(0) % 2147483646) + 1
  return () => {
    state = (state * 16807) % 2147483647
    return (state - 1) / 2147483646
  }
}

export const pick = (random, list) => list[Math.floor(random() * list.length)]

export const between = (random, low, high) =>
  low + Math.floor(random() * (high - low + 1))

export const ALNUM = 'ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789'

export const chars = (random, alphabet, length) =>
  Array.from({ length }, () => pick(random, alphabet)).join('')

export const hex = (random, length) => chars(random, '0123456789abcdef', length)

const uuidText = (h) =>
  `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20, 32)}`

const variant = (random) => pick(random, '89ab')

/** Claude Code names sessions with random UUIDs. */
export const uuid4 = (random) =>
  uuidText(
    `${hex(random, 12)}4${hex(random, 3)}${variant(random)}${hex(random, 15)}`,
  )

/** Codex and pi name threads with time-ordered UUIDs. */
export const uuid7 = (random, at) =>
  uuidText(
    `${at.toString(16).padStart(12, '0')}7${hex(random, 3)}${variant(random)}${hex(random, 15)}`,
  )

export const iso = (at) => new Date(at).toISOString()

/** One JSON record per line; a string is written as it is (a damaged line). */
export const jsonl = (rows) =>
  rows
    .map((row) => `${typeof row === 'string' ? row : JSON.stringify(row)}\n`)
    .join('')

export const zstd = (rows) => zstdCompressSync(Buffer.from(jsonl(rows)))

/** `days` before the local day `today` starts, at a local time. */
export function dayAt(today, days, hour, minute) {
  const day = new Date(today)
  day.setDate(day.getDate() - days)
  day.setHours(hour, minute, 0, 0)
  return day.getTime()
}

export function startOfDay(now) {
  const today = new Date(now)
  today.setHours(0, 0, 0, 0)
  return today.getTime()
}

/** Moves the conversation's clock on by a few seconds to minutes. */
export const later = (state, low, high) =>
  (state.at += between(state.random, low, high) * 1000)

/** A made-up word no other conversation uses, so search finds one place. */
export function newWord(taken, random) {
  for (;;) {
    let word = ''
    for (let index = 0; index < 4; index += 1)
      word += pick(random, 'bdfgklmnprstvz') + pick(random, 'aeiou')
    word += pick(random, 'nrlsk')
    if (!taken.has(word)) {
      taken.add(word)
      return word
    }
  }
}

export const round = (value) => Math.round(value * 1e6) / 1e6
