/**
 * Records search and the reader's find, for the specs: a search (by words
 * or by meaning) or a find run through the backend the way the window runs
 * one, read to its last frame over the bridge
 * (`/previews/{streamId}/events`), to check the screen against; the
 * semantic index's status; and what find lights on the page (the CSS
 * highlights).
 */
import type { Page } from '@playwright/test'
import { resolveDesktopBridgeEnv } from '../../../scripts/pathkeep-dev-desktop-bridge.mjs'
import { backend } from './fixture'

export interface SearchFrame {
  type: string
  payload: {
    items?: { id: string; hits: number; snippets?: unknown[] }[]
    hits?: number
    conversations?: number
    searchedCount?: number
    filtered?: boolean
    code?: string
  }
}

async function readStream(command: string, request: object) {
  const { devIpcUrl } = resolveDesktopBridgeEnv(process.env)
  const started = await backend<{ streamId: string }>(command, { request })
  const response = await fetch(
    `${devIpcUrl}/previews/${encodeURIComponent(started.streamId)}/events`,
  )
  return (await response.text())
    .split('\n')
    .filter((line) => line.startsWith('data:'))
    .map((line) => JSON.parse(line.slice('data:'.length)) as SearchFrame)
}

/**
 * A Records search to its end: the cards in arrival order (a later copy of
 * a card replaces it in place) and the finished frame.
 */
export async function wireSearch(request: object) {
  const frames = await readStream('search_agent_conversations', request)
  const order: string[] = []
  const cards = new Map<string, { id: string; hits: number }>()
  for (const frame of frames)
    if (frame.type === 'groups')
      for (const card of frame.payload.items ?? []) {
        if (!cards.has(card.id)) order.push(card.id)
        cards.set(card.id, card)
      }
  const finished = frames.find((frame) => frame.type === 'finished')
  if (!finished)
    throw new Error(
      `The search did not finish: ${frames.at(-1)?.type} ${JSON.stringify(frames.at(-1)?.payload)}`,
    )
  return {
    order,
    cards,
    hits: finished.payload.hits!,
    conversations: finished.payload.conversations!,
    searched: finished.payload.searchedCount!,
    filtered: finished.payload.filtered!,
  }
}

/** One passage of a semantic search, as the backend streams it. */
export interface WirePassage {
  id: string
  title: string | null
  address: {
    conversationId: string
    sourceId: string
    ordinal: number
    publication: string
  }
  role: string
  text: string
  chunk: number
  score: number
}

/**
 * The passage's identity as Records writes it on the page
 * (`data-passage`): `<conversation>/<ordinal>:<source>/<chunk>`.
 */
export const passageKey = (passage: WirePassage) =>
  `${passage.address.conversationId}/${passage.address.ordinal}:${passage.address.sourceId}/${passage.chunk}`

/**
 * A search by meaning to its end: every passage in the order streamed
 * (closest first) and the finished frame. `request` is the full-text one;
 * `mode: 'semantic'` is added here.
 */
export async function wireSemantic(request: object) {
  const frames = await readStream('search_agent_conversations', {
    mode: 'semantic',
    ...request,
  })
  const started = frames.find((frame) => frame.type === 'started')
  const finished = frames.find((frame) => frame.type === 'finished')
  if (!started || !finished)
    throw new Error(
      `The semantic search did not finish: ${frames.at(-1)?.type} ${JSON.stringify(frames.at(-1)?.payload)}`,
    )
  const passages = frames.flatMap((frame) =>
    frame.type === 'passages'
      ? (frame.payload.items as unknown as WirePassage[])
      : [],
  )
  const end = finished.payload as unknown as {
    hits: number
    generation: string
    stale: boolean
  }
  return { passages, ...end }
}

/** `get_agent_semantic_index`: whether the index may exist, what serves and what builds. */
export function semanticStatus() {
  return backend<{
    enabled: boolean
    serving: { generation: string; provider: string; model: string } | null
    stale?: boolean
    job: { jobId: string; state: string; passages: number } | null
    reason?: string
  }>('get_agent_semantic_index', { request: {} })
}

/** Find in one conversation to its end: how many occurrences. */
export async function wireFind(id: string, query: string) {
  const frames = await readStream('find_in_agent_conversation', { id, query })
  const finished = frames.find((frame) => frame.type === 'finished')
  if (!finished) throw new Error(`Find did not finish in ${id}`)
  return finished.payload.hits!
}

/**
 * What find lights now: every lit text, and the current one with the
 * whole text of the part it is in.
 */
export function findHighlights(page: Page) {
  return page.evaluate(() => {
    const all = CSS.highlights.get('pathkeep-find')
    const current = CSS.highlights.get('pathkeep-find-current')
    const [range] = current ? [...current] : []
    const part = (range?.startContainer.parentElement ?? null)?.closest(
      '[data-part]',
    )
    return {
      lit: all ? [...all].map((item) => (item as Range).toString()) : [],
      current: range ? (range as Range).toString() : null,
      part: part?.textContent ?? null,
    }
  })
}
