import type { AiSearchResponse, HistoryQueryResponse } from '../../lib/types'

export type ExplorerMode = 'keyword' | 'semantic' | 'hybrid'

export type Translator = (
  key: string,
  vars?: Record<string, string | number>,
) => string

export interface ExplorerQueryState {
  requestKey: string | null
  results: HistoryQueryResponse | null
  error: string | null
}

export interface SemanticQueryState {
  requestKey: string | null
  results: AiSearchResponse | null
  error: string | null
}

export interface RecentSearchEntry {
  label?: string
  params: {
    q?: string | null
    mode?: ExplorerMode | null
    domain?: string | null
    profileId?: string | null
    browserKind?: string | null
    start?: string | null
    end?: string | null
    regex?: '1' | null
    sort?: 'newest' | 'oldest'
  }
}
