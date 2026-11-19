/**
 * What a recording run collects in the page: every command the app sent
 * through the real bridge with its answer, and every search with the events
 * it streamed. `record.ts` reads it out at the end of the walk.
 */
import type {
  SearchEvent,
  SearchRequest,
  SearchStarted,
} from '@/lib/backend-client/search'
import type { RecordedCall, RecordedSearch } from '../backend'

export interface RecordLog {
  calls: RecordedCall[]
  searches: RecordedSearch[]
}

declare global {
  var pathkeepDemoLog: RecordLog | undefined
}

export const log: RecordLog = (globalThis.pathkeepDemoLog ??= {
  calls: [],
  searches: [],
})

export function logSearch(
  request: SearchRequest,
  started: SearchStarted,
): SearchEvent[] {
  const events: SearchEvent[] = []
  log.searches.push({ request, started, events })
  return events
}
