/**
 * Typed client for streamed search sessions.
 *
 * Why this file exists:
 * - Search reaches every matching page through the Tantivy index. On a large
 *   archive the exact ranking and count can take seconds, so the backend
 *   streams provisional steps; routes use this client instead of raw commands.
 *
 * Main declarations:
 * - `searchClient`, and the wire types below.
 *
 * Source-of-truth notes:
 * - Contract: `docs/architecture/search.md` (Desktop, worker and dev IPC);
 *   shapes mirror `vault_core::search::{types, stream, indexer}`.
 */

import type { HistoryEntry } from '../types'
import type { AiSearchNote } from '../types/intelligence'
import { openSearchStream } from '../ipc/search-stream'
import { call } from './shared'

export interface SearchRequest {
  query?: string
  pageSize?: number
  sort?: 'relevance' | 'newest' | 'oldest' | null
  profileId?: string | null
  browserKind?: string | null
  domain?: string | null
  startTimeMs?: number | null
  endTimeMs?: number | null
  regex?: boolean
  fuzzy?: boolean
  starred?: boolean
}

/** How far the search index is behind the archive. */
export interface IndexerState {
  phase:
    | 'building'
    | 'updating'
    | 'ready'
    | 'rebuilding'
    | 'cancelled'
    | 'failed'
    | 'closed'
  /** During a build: pages indexed so far; later: replacements applied. */
  documentsProcessed: number
  /** During a build: pages to index. */
  documentsTotal: number
  appliedSeq: number
  targetSeq: number
  error: string | null
}

export interface SearchStarted {
  sessionId: string
  indexer: IndexerState
}

/** `exact` is false while the count is still running; `value` is a lower bound until then. */
export interface SearchCount {
  value: number
  exact: boolean
}

export interface SearchProgress {
  documentsProcessed: number
  segmentsCompleted: number
  segmentsTotal: number
  statisticsFieldsProcessed: number
  statisticsFieldsTotal: number
}

export interface SearchHit {
  pageId: number
  score: number
  visit: HistoryEntry & { visitCount?: number }
  reason?: string
}

/**
 * One step of one result page. A step replaces the earlier steps of the same
 * `page`. `provisional` means the order can still change; `coverageComplete`
 * false means the index hasn't caught up with the archive yet, so pages can be
 * missing — an empty step is then never "no results".
 */
export interface SearchStep {
  page: number
  items: SearchHit[]
  provisional: boolean
  progress: SearchProgress
  count: SearchCount
  visits: SearchCount
  countProgress: SearchProgress
  cursor: string | null
  indexer: IndexerState
  snapshotSeq: number
  coverageComplete: boolean
  searchNotes?: AiSearchNote[]
}

/** How complete the index was for a non-streamed answer (history pages, AI search). */
export interface SearchCoverage {
  indexer: IndexerState
  snapshotSeq: number
  coverageComplete: boolean
}

export type SearchEvent =
  | { type: 'indexer'; data: IndexerState }
  | { type: 'step'; data: SearchStep }
  | { type: 'error'; data: string }
  | { type: 'cancelled' }

export const searchClient = {
  start: openSearchStream,
  /** Cursors are single-use and belong to their session. */
  nextPage: (sessionId: string, cursor: string) =>
    call<void>('next_search_page', { sessionId, cursor }),
  indexerState: () => call<IndexerState>('search_indexer_state'),
}
