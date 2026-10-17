/**
 * Responsibilities: type the optional related-page read for History.
 * Not responsible for: ranking, index availability or selection state.
 * Failure modes: unavailable, updating and empty results get conflated.
 * Tests: semantic-index.spec.ts uses the real command and panel.
 */
import { call } from './shared'

/** Carries the archived visit identity so selection needs no second search. */
export interface RelatedPage {
  historyId: number
  url: string
  title: string | null
  domain: string
  profileId: string
  lastVisitTime: number
  score: number
}

/** Separates an optional feature from a page that has no vector yet. */
export type RelatedPages =
  | { state: 'unavailable'; reason: string }
  | { state: 'updating' | 'notIndexed'; staticModel: boolean }
  | { state: 'ready'; staticModel: boolean; items: RelatedPage[] }

/** Uses the same desktop/dev-bridge contract as the other archive reads. */
export const relatedPagesClient = {
  get: (url: string, visitId?: number) =>
    call<RelatedPages>('get_related_pages', { url, visitId: visitId ?? null }),
}
