/**
 * @file secondary-sections.tsx
 * @description Preserves the public secondary-section import surface while delegating each card to its own focused module.
 * @module pages/intelligence/sections
 *
 * ## Responsibilities
 * - Keep `sections.tsx` importing secondary cards from one stable path.
 * - Re-export the secondary overview cards in the same order and names as before.
 * - Make the per-card ownership split explicit without changing route composition.
 *
 * ## Non-Responsibilities
 * - Does not fetch data, render card bodies, or own secondary-grid ordering.
 * - Does not contain filtering heuristics; those live with the secondary-section modules.
 * - Does not define new route grammar or public contracts.
 *
 * ## Dependencies
 * - Depends on the focused secondary-section modules under `./secondary-sections/`.
 *
 * ## Performance Notes
 * - This module is intentionally a zero-logic barrel so the split has no runtime overhead beyond normal ESM imports.
 */

export { StableSourcesSection } from './secondary-sections/stable-sources-section'
export { SearchEffectivenessSection } from './secondary-sections/search-effectiveness-section'
export { FrictionDetectionSection } from './secondary-sections/friction-detection-section'
export { ReopenedInvestigationsSection } from './secondary-sections/reopened-investigations-section'
export { DiscoveryTrendSection } from './secondary-sections/discovery-trend-section'
export { BreadthIndexSection } from './secondary-sections/breadth-index-section'
export { PathFlowsSection } from './secondary-sections/path-flows-section'
export { HabitsSection } from './secondary-sections/habits-section'
export { CompareSetsSection } from './secondary-sections/compare-sets-section'
export { MultiBrowserDiffSection } from './secondary-sections/multi-browser-diff-section'
export { ObservedInteractionsSection } from './secondary-sections/observed-interactions-section'
