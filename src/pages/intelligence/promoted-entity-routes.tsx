/**
 * Barrel export for split promoted intelligence entity routes.
 *
 * Why this file exists:
 * - M10 decomposes the old route mega-file without changing downstream import
 *   paths from `src/pages/intelligence`.
 */

export { CompareSetInsightsRoutePage } from './promoted-entity-routes/compare-set-route'
export { QueryFamilyInsightsRoutePage } from './promoted-entity-routes/query-family-route'
export { RefindPageInsightsRoutePage } from './promoted-entity-routes/refind-route'
export { SessionInsightsRoutePage } from './promoted-entity-routes/session-route'
export { TrailInsightsRoutePage } from './promoted-entity-routes/trail-route'
