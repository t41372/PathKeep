import type { HistoryQuery, HistoryQueryResponse } from '../types'
import { call } from './shared'

export const explorerClient = {
  queryHistory: (query: HistoryQuery) =>
    call<HistoryQueryResponse>('query_history', { query }),
}
