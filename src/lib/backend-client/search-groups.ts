/**
 * Responsibilities: type the optional Insights search-group read.
 * Not responsible for: clustering, freshness decisions or navigation.
 * Failure modes: an unfinished month reads as empty, or a profile silently changes monthly counts.
 * Tests: semantic-index.spec.ts exercises the desktop/dev command and the card.
 */
import type {
  CoreIntelligenceSectionMeta,
  DateRange,
} from '@/lib/core-intelligence/types'
import { call } from './shared'

/** Bounded display evidence; wording links are always the user's own searches. */
export interface SearchGroup {
  wordings: string[]
  distinctSearches: number
  searches: number
  trails: number
  days: string[]
  pagesOpened: number
  topPages: number[]
}
export interface SearchGroupWindow {
  range: DateRange
  updating: boolean
  groups: SearchGroup[]
}
export interface SearchGroupsCard {
  available: boolean
  staticModel: boolean
  monthly: boolean
  data: SearchGroupWindow[]
  meta: CoreIntelligenceSectionMeta | null
}

/** Unkept short ranges group on the backend worker, while the card paints skeletons. */
export const searchGroupsClient = {
  get: (request: { dateRange: DateRange; profileId: string | null }) =>
    call<SearchGroupsCard>('get_search_groups', { request }),
}
