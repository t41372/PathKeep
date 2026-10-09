/**
 * Opening Insights reads batches, not one request per card (the accepted
 * contract in docs/features/intelligence.md): the primary overview, then the
 * secondary. Each batch's sections go into the query cache under the keys the
 * cards read, so a card whose section came in a batch shows it without a
 * request of its own. A card waits for its batch before reading by itself,
 * which it then does only when its section was missing or failed. For all
 * time the backend answers both from the snapshot it keeps after each Insights
 * build; for other ranges it computes them (one year on a 387k-visit archive:
 * 74 ms and 47 ms).
 *
 * Responsible for: the two batch reads, filling the cache, telling cards when
 * their batch has settled. Not responsible for: what a card shows.
 */
import {
  useQuery,
  useQueryClient,
  type QueryClient,
} from '@tanstack/react-query'
import { intelligenceClient } from '@/lib/backend-client/intelligence'
import type {
  CoreIntelligencePrimaryOverview,
  CoreIntelligenceSecondaryOverview,
} from '@/lib/core-intelligence/types'
import type { Band } from './bands'
import { insightsKey, request } from './queries'
import type { Scope } from './range'

function fillPrimary(
  client: QueryClient,
  scope: Scope,
  overview: CoreIntelligencePrimaryOverview,
) {
  const put = (name: string, value: unknown) =>
    client.setQueryData(insightsKey(scope, name), value)
  put('digest', overview.digestSummary)
  put('top-sites', overview.topSites)
  put('refind', overview.refindPages)
  put('search-engines', overview.searchEngineRanking)
  put('search-concepts', overview.topSearchConcepts)
  put('query-families', overview.queryFamilies)
  put('new-sites', overview.discoveryTrendDay)
  put('habits', overview.habitPatterns)
  put('interrupted-habits', overview.interruptedHabits)
}

function fillSecondary(
  client: QueryClient,
  scope: Scope,
  overview: CoreIntelligenceSecondaryOverview,
) {
  const put = (name: string, value: unknown) =>
    client.setQueryData(insightsKey(scope, name), value)
  put('threads', overview.reopenedInvestigations)
  put('breadth', overview.breadthIndex)
  put('paths', overview.pathFlows)
  put('browsers', overview.multiBrowserDiff)
  put('effectiveness', overview.searchEffectiveness)
  put('friction', overview.frictionSignals)
  put('compare-sets', overview.compareSets)
  put('search-landings', overview.stableSources)
}

/** Reads both batches for `scope` and reports which have settled. */
export function useOverviewBands(scope: Scope): Record<Band, boolean> {
  const client = useQueryClient()
  const primary = useQuery({
    queryKey: insightsKey(scope, 'overview', 'primary'),
    queryFn: async () => {
      const overview = await intelligenceClient.getPrimaryOverview(
        request(scope),
      )
      fillPrimary(client, scope, overview)
      return overview.totalDurationMs
    },
  })
  const primarySettled = !primary.isPending
  const secondary = useQuery({
    queryKey: insightsKey(scope, 'overview', 'secondary'),
    queryFn: async () => {
      const overview = await intelligenceClient.getSecondaryOverview(
        request(scope),
      )
      fillSecondary(client, scope, overview)
      return overview.totalDurationMs
    },
    enabled: primarySettled,
  })
  return { primary: primarySettled, secondary: !secondary.isPending }
}
