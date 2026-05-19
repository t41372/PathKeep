/**
 * Paper-redesign Search surface mounted via `?layout=paper&surface=search`.
 * Wraps PaperSearchView and adapts the existing Explorer URL state
 * (`queryInput` / `mode` / `regexMode`) into the paper hero's three-mode
 * + day-grouped-result shape.
 *
 * ## Responsibilities
 * - Translate explorer mode + regex flag to the paper search-mode ternary.
 * - Build day groups from the visible history results.
 * - Forward query / mode / submit / select / see-in-context interactions
 *   back to the route via lightweight callbacks.
 *
 * ## Not responsible for
 * - Fetching results — the route still owns the queryState + history hooks.
 * - URL parsing — the route hands typed callbacks for each side effect.
 */

import type { HistoryEntry } from '@/lib/types/archive'
import {
  PaperSearchView,
  type PaperSearchResultEntry,
} from '@/components/explorer-paper'
import { buildPaperSearchViewCopy } from './paper-explorer-copy'
import {
  buildPaperSearchDayGroups,
  explorerStateFromPaperSearchMode,
  paperSearchModeFromExplorerState,
} from './paper-search-helpers'
import { getDomainAbbr, getDomainColor } from './paper/domain-color'
import type { ExplorerMode } from './types'

export interface PaperSearchPanelProps {
  /** Active query text. */
  query: string
  /** Current explorer-side mode + regex flag. */
  mode: ExplorerMode
  regexMode: boolean
  /** History entries currently visible to the route. */
  entries: readonly HistoryEntry[]
  totalResults: number
  language: string
  explorerT: (key: string, vars?: Record<string, string | number>) => string
  /** Apply text input changes to URL + local state. */
  onQueryChange: (next: string) => void
  /** Apply explorer mode + regex flag from the paper hero. */
  onModeChange: (next: { mode: ExplorerMode; regexMode: boolean }) => void
  /** Submit handler (Enter in the hero). */
  onSubmit: (query: string) => void
  /** Selected entry id update — number coerced from the paper id. */
  onSelectEntry: (id: number) => void
  /** Jump back to PaperExplorerView centred on the result's day. */
  onSeeInContext: (entry: PaperSearchResultEntry, dayDate: string) => void
}

export function PaperSearchPanel({
  query,
  mode,
  regexMode,
  entries,
  totalResults,
  language,
  explorerT,
  onQueryChange,
  onModeChange,
  onSubmit,
  onSelectEntry,
  onSeeInContext,
}: PaperSearchPanelProps) {
  return (
    <PaperSearchView
      query={query}
      mode={paperSearchModeFromExplorerState(mode, regexMode)}
      activeFilters={[]}
      groups={buildPaperSearchDayGroups(entries, { language })}
      totalResults={totalResults}
      resolveDomainColor={getDomainColor}
      resolveDomainAbbr={getDomainAbbr}
      onQueryChange={onQueryChange}
      onModeChange={(nextMode) =>
        onModeChange(explorerStateFromPaperSearchMode(nextMode))
      }
      onRemoveFilter={() => {
        /* Paper hero filter chips are wired in a later pass. */
      }}
      onSubmit={onSubmit}
      onSelectEntry={(entry) => onSelectEntry(Number(entry.id))}
      onSeeInContext={onSeeInContext}
      copy={buildPaperSearchViewCopy(explorerT)}
      testId="explorer-paper-search-view"
    />
  )
}
