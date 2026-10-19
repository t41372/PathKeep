/**
 * Responsibilities: type the optional semantic search drill-in read.
 * Not responsible for: ranking, availability or navigation.
 * Failure modes: an unindexed search and an empty result get conflated.
 * Tests: semantic-index.spec.ts exercises the real command and rows.
 */
import { call } from './shared'

/** A search's all-time evidence accompanies its link. */
export interface SimilarSearch {
  query: string
  searches: number
  lastSearchedAt: number
}

/** The source can lack a vector even when the serving index is available. */
export type SimilarSearches =
  | { state: 'unavailable'; reason: string }
  | { state: 'notIndexed'; staticModel: boolean }
  | { state: 'ready'; staticModel: boolean; items: SimilarSearch[] }

/** Uses the shared desktop/dev bridge transport without changing its contract. */
export const similarSearchesClient = {
  get: (query: string, wordings: string[]) =>
    call<SimilarSearches>('get_similar_searches', { query, wordings }),
}
