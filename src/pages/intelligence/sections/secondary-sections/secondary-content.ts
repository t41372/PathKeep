/**
 * @file secondary-content.ts
 * @description Decides, from cached overview payloads, whether a hide-when-empty secondary card will actually render anything.
 * @module pages/intelligence/sections/secondary-sections
 *
 * ## Responsibilities
 * - Mirror each hide-when-empty section's own visibility rule against its peeked cache result.
 * - Let the route shell drop a known-empty card's layout slot instead of leaving a blank cell.
 * - Stay conservative: only report "no content" when the section is provably going to render `null`.
 *
 * ## Non-Responsibilities
 * - Does not fetch data, call `peek*`, or know about `dateRange` / `profileId` scoping.
 * - Does not render UI, own grid ordering, or change a section's internal hide-when-empty logic.
 * - Does not cover always-rendering cards (breadth, compare sets, multi-browser, observed, habits, health),
 *   which show an honest empty state instead of disappearing and therefore always own a slot.
 *
 * ## Dependencies
 * - `lib/core-intelligence` for the section result envelope and entity types.
 * - `./heuristics` for the same low-signal filters the sections apply before deciding to hide.
 *
 * ## Performance Notes
 * - Operates on already-bounded cache snapshots, so each predicate is O(n) over a capped result list.
 * - Runs during staged overview renders, so it stays allocation-light and avoids re-deriving heavy state.
 */

import type {
  CoreIntelligenceSectionResult,
  DiscoveryTrend,
  FrictionSignal,
  PathFlow,
  ReopenedInvestigation,
  SearchEffectiveness,
} from '../../../../lib/core-intelligence'
import {
  isMeaningfulFrictionSignal,
  isMeaningfulPathFlow,
  isSearchBackedReopenedInvestigation,
} from './heuristics'

/**
 * Why this exists: a section only hides itself once its data is `ready` and the
 * filtered result is empty. Before that — no cache, or a `stale`/`disabled`/
 * `degraded` snapshot — we cannot prove emptiness, so the slot must stay so the
 * card can still mount and show data or its own empty/loading state.
 *
 * @param cached The peeked section result, or `null` when nothing is cached yet.
 * @returns `false` only when the ready payload provably yields no rows; `true` otherwise.
 */
function decideFromReady<T>(
  cached: CoreIntelligenceSectionResult<T> | null,
  hasRows: (data: T) => boolean,
): boolean {
  if (!cached || cached.meta.state !== 'ready') {
    return true
  }

  return hasRows(cached.data)
}

/**
 * Matches `SearchEffectivenessSection`: hidden when every engine/source/topic list is empty.
 */
export function hasSearchEffectivenessContent(
  cached: CoreIntelligenceSectionResult<SearchEffectiveness> | null,
): boolean {
  return decideFromReady(
    cached,
    (effectiveness) =>
      effectiveness.engineStats.length > 0 ||
      effectiveness.topResolvingSources.length > 0 ||
      effectiveness.hardestTopics.length > 0,
  )
}

/**
 * Matches `FrictionDetectionSection`: hidden when no meaningful friction signal survives filtering.
 */
export function hasFrictionContent(
  cached: CoreIntelligenceSectionResult<FrictionSignal[]> | null,
): boolean {
  return decideFromReady(cached, (signals) =>
    signals.some(isMeaningfulFrictionSignal),
  )
}

/**
 * Matches `ReopenedInvestigationsSection`: hidden when no search-backed investigation survives filtering.
 */
export function hasReopenedInvestigationsContent(
  cached: CoreIntelligenceSectionResult<ReopenedInvestigation[]> | null,
): boolean {
  return decideFromReady(cached, (items) =>
    items.some(isSearchBackedReopenedInvestigation),
  )
}

/**
 * Matches `DiscoveryTrendSection`: hidden when the trend has no weekly points.
 */
export function hasDiscoveryTrendContent(
  cached: CoreIntelligenceSectionResult<DiscoveryTrend> | null,
): boolean {
  return decideFromReady(cached, (trend) => trend.points.length > 0)
}

/**
 * Matches `PathFlowsSection`: hidden when no meaningful multi-step flow survives filtering.
 */
export function hasPathFlowsContent(
  cached: CoreIntelligenceSectionResult<PathFlow[]> | null,
): boolean {
  return decideFromReady(cached, (flows) => flows.some(isMeaningfulPathFlow))
}
