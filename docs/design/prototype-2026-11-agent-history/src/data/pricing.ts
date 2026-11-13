/**
 * Public list prices, per million tokens, for estimating what a
 * conversation would cost at API rates when the tool didn't record a cost.
 * Ships with the app and is updated with it; a model missing here (a local
 * model, a new release) is shown as "no price", never as zero.
 * The prices below are invented for the prototype.
 */
import type { Chat } from './types'

type Price = { input: number; cacheWrite: number; cacheRead: number; output: number }

export const PRICE_TABLE_VERSION = '2026-11'

const table: Record<string, Price | null> = {
  'claude-opus-4-6': { input: 15, cacheWrite: 18.75, cacheRead: 1.5, output: 75 },
  'claude-opus-4-7': { input: 15, cacheWrite: 18.75, cacheRead: 1.5, output: 75 },
  'claude-opus-4-8': { input: 15, cacheWrite: 18.75, cacheRead: 1.5, output: 75 },
  'claude-opus-5': { input: 5, cacheWrite: 6.25, cacheRead: 0.5, output: 25 },
  'claude-opus-5-5': { input: 5, cacheWrite: 6.25, cacheRead: 0.5, output: 25 },
  'claude-fable-5': { input: 3, cacheWrite: 3.75, cacheRead: 0.3, output: 15 },
  'claude-fable-5-1': { input: 3, cacheWrite: 3.75, cacheRead: 0.3, output: 15 },
  'claude-fable-5-2': { input: 3, cacheWrite: 3.75, cacheRead: 0.3, output: 15 },
  'claude-sonnet-5-5': { input: 3, cacheWrite: 3.75, cacheRead: 0.3, output: 15 },
  'gpt-6-sol': { input: 1.25, cacheWrite: 0, cacheRead: 0.125, output: 10 },
  'gpt-6-astra': { input: 2, cacheWrite: 0, cacheRead: 0.2, output: 12 },
  'gpt-6.1-sol': { input: 1.25, cacheWrite: 0, cacheRead: 0.125, output: 10 },
  'kimi-k3': { input: 0.6, cacheWrite: 0.6, cacheRead: 0.15, output: 2.5 },
  'deepseek-v4': { input: 0.27, cacheWrite: 0.27, cacheRead: 0.07, output: 1.1 },
  // Runs on your own machine: no list price.
  'qwen3.8-27b': null,
}

/** Estimated USD at list prices, or null when the model has no price or usage wasn't recorded. */
export function estimateCost(chat: Chat): number | null {
  if (!chat.tokens) return null
  const price = table[chat.models[0]]
  if (!price) return null
  const t = chat.tokens
  return (t.input * price.input + t.cacheWrite * price.cacheWrite + t.cacheRead * price.cacheRead + t.output * price.output) / 1e6
}

/** Recorded cost where the tool wrote one, otherwise the estimate. */
export function costOf(chats: Iterable<Chat>) {
  let recorded = 0
  let recordedChats = 0
  let estimated = 0
  let estimatedChats = 0
  let unpriced = 0
  for (const chat of chats) {
    if (chat.recordedCostUsd !== undefined) {
      recorded += chat.recordedCostUsd
      recordedChats++
      continue
    }
    const e = estimateCost(chat)
    if (e === null) unpriced++
    else {
      estimated += e
      estimatedChats++
    }
  }
  return { recorded, recordedChats, estimated, estimatedChats, unpriced, total: recorded + estimated }
}
