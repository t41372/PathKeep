/**
 * Typed shapes for searching saved conversations (Records search, by words
 * or by meaning) and for finding inside one conversation (the reader's find
 * bar). All answer at once with a stream id and then stream frames; the
 * transport is `src/lib/ipc/agent-read-stream.ts`.
 *
 * Responsible for: the request and frame shapes. Not responsible for:
 * delivering frames or holding results (`features/agent-search`,
 * `features/agent-reader/use-find.ts`).
 *
 * Source of truth: `docs/architecture/desktop-command-surface.md`
 * (Records search and reader find; "Agent conversation AI access" for the
 * semantic mode). Text ranges are UTF-8 byte offsets; `utf8ToIndex` in
 * `src/lib/utf8-range.ts` turns them into string indexes.
 */
import type { AgentToolId } from './data-kinds'
import type { ChatFlag, ChatProject, ChatRange } from './agent-chats'

/** What is searched besides titles, which always are. */
export type SearchRole = 'user' | 'agent' | 'tool' | 'thinking'
export const SEARCH_ROLES: readonly SearchRole[] = [
  'user',
  'agent',
  'tool',
  'thinking',
]

/** The list's filters a search keeps: never a sort, group or cursor. */
export interface SearchFilters {
  harnesses?: AgentToolId[]
  projectId?: string
  range?: ChatRange
  day?: string
  timeZone?: string
  flags?: ChatFlag[]
}

export interface SearchRequest {
  /** Words (all in one record), a "phrase", or a /regular expression/. */
  query: string
  roles: SearchRole[]
  filters: SearchFilters
}

export interface FindRequest {
  id: string
  query: string
}

/** Where an occurrence is: a record of the searched conversation or a subagent's. */
export interface MatchAddress {
  conversationId: string
  publication: string
  sourceId: string
  ordinal: number
  nativeId?: string | null
}

/** A snippet on a search card, or one occurrence in find. */
export interface SearchMatch {
  /** `title` for a subagent's title in find. */
  role: SearchRole | 'title'
  /** Whitespace collapsed and clipped around the match. */
  text: string
  /** Byte range of the match in `text`. */
  highlight: [number, number]
  /** Byte range in the whole readable part (or the title). */
  range: [number, number]
  part: number | null
  /** 1-based, in the conversation's order. */
  occurrence: number
  address: MatchAddress | null
  subagentId: string | null
  /** Subagents from the searched conversation down to the one that has it. */
  path: string[]
  /** The fold to open: a tool call, thinking, or a group of subagents. */
  fold: 'tool' | 'thinking' | 'subagent' | null
}

/** One matching conversation on the results list; later copies replace it. */
export interface SearchCard {
  id: string
  title: string | null
  harness: AgentToolId
  project: ChatProject | null
  lastActivityAt: number | null
  onlyInPathKeep: boolean
  archived: boolean
  /** Every match in it and its subagents, so far. */
  hits: number
  titleHit: boolean
  /** Up to three. */
  snippets: SearchMatch[]
}

export interface StreamStarted {
  streamId: string
  viewToken: string
}

type Frame<Type extends string, Payload> = {
  streamId: string
  sequence: string
  type: Type
  payload: Payload
}

export type SearchFrame =
  | Frame<
      'started',
      {
        total: number
        indexLag: number
        indexedThrough: number | null
        filtered: boolean
      }
    >
  | Frame<'groups', { items: SearchCard[] }>
  | Frame<'matches', { items: SearchMatch[]; hits: number }>
  | Frame<
      'progress',
      {
        scanned: number
        total: number | null
        conversations: number
        hits: number
      }
    >
  | Frame<
      'finished',
      {
        searchedCount: number
        hits: number
        conversations: number
        filtered: boolean
        indexLag: number
        complete: boolean
      }
    >
  | Frame<'cancelled' | 'error', { code: string; message?: string }>

/**
 * A search by meaning: the same words-box text, kinds of text and filters,
 * answered from the semantic index with passages instead of cards. The
 * backend refuses a limit or a cursor here: every passage that is close at
 * all streams.
 */
export interface SemanticSearchRequest extends SearchRequest {
  mode: 'semantic'
}

/**
 * One passage close in meaning to the search: up to 1,024 characters of
 * one record's readable part, with the conversation it belongs to. It has
 * no highlight; nothing in it need match the words literally.
 */
export interface SemanticPassage {
  /** The conversation on the list: the searched one, also for a subagent's record. */
  id: string
  title: string | null
  harness: AgentToolId
  lastActivityAt: number | null
  archived: boolean
  onlyInPathKeep: boolean
  /** The record; its `conversationId` differs from `id` for a subagent's. */
  address: MatchAddress
  role: SearchRole
  /** Which readable part of the record. */
  part: number
  /** Where the passage starts in that part, in Unicode scalars. */
  start: number
  text: string
  /** Which passage of the record, counted from its start. */
  chunk: number
  /** Cosine similarity to the search, above 0; the stream is highest first. */
  score: number
}

export type SemanticSearchFrame =
  | Frame<'started', { mode: 'semantic' }>
  | Frame<'passages', { items: SemanticPassage[] }>
  | Frame<
      'finished',
      {
        hits: number
        generation: string
        /** Conversations changed after the index this answered from was built. */
        stale: boolean
      }
    >
  | Frame<'cancelled' | 'error', { code: string; message?: string }>
