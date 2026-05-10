/**
 * @file backend-preview-showcase.test.ts
 * @description Unit coverage for the synthetic browser-preview showcase dataset switch.
 * @module lib/backend-preview-showcase.test
 *
 * ## Responsibilities
 * - Verify the browser-preview dataset flag activates modeled showcase totals without requiring desktop data.
 * - Protect showcase-only Core Intelligence command branches used by public static previews.
 *
 * ## Not responsible for
 * - Revalidating desktop/Tauri command truth; those paths are covered by desktop-contract tests.
 * - Testing the full visual route rendering of the showcase dataset.
 *
 * ## Dependencies
 * - Uses Vitest globals to emulate the Vite build-time dataset define.
 * - Reuses browser-preview state and support helpers so coverage follows the same runtime entry points as the app.
 *
 * ## Performance notes
 * - Tests stay bounded to small synthetic fixtures and do not read local archive files.
 */

import { afterEach, describe, expect, test, vi } from 'vitest'
import {
  PREVIEW_COMMAND_UNHANDLED,
  type PreviewCommandResult,
} from './backend-preview-command-result'
import { handlePreviewIntelligenceCommand } from './backend-preview-intelligence-commands'
import { createMockState } from './backend-preview-state'
import { buildMockDashboardSnapshot } from './backend-preview-support'

describe('browser-preview showcase dataset', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
  })

  test('activates modeled dashboard totals without seeding queue noise', () => {
    vi.stubGlobal('__PATHKEEP_BROWSER_PREVIEW_DATASET__', 'showcase')

    const state = createMockState()
    state.snapshot.config.initialized = true
    const dashboard = buildMockDashboardSnapshot(state)

    expect(state.queueJobs).toEqual([])
    expect(state.showcaseTotals).toMatchObject({
      modeledTotalVisits: 348_000,
      modeledTotalUrls: 172_000,
      modeledProfiles: 4,
    })
    expect(dashboard).toMatchObject({
      totalProfiles: 4,
      totalUrls: 172_000,
      totalVisits: 348_000,
      storage: expect.objectContaining({
        archiveDatabaseBytes: 777_990_144,
        sourceEvidenceDatabaseBytes: 572_641_280,
        intelligenceBlobBytes: 18_582_912,
      }),
    })
  })

  test('serves showcase Core Intelligence reads instead of empty fallbacks', () => {
    vi.stubGlobal('__PATHKEEP_BROWSER_PREVIEW_DATASET__', 'showcase')
    const state = createMockState()

    const primary = handled(
      handlePreviewIntelligenceCommand<{
        digestSummary: { data: { totalVisits: { value: number } } }
        topSites: { data: Array<{ registrableDomain: string }> }
      }>('get_intelligence_primary_overview', undefined, state),
    )
    expect(primary.digestSummary.data.totalVisits.value).toBe(6071)
    expect(primary.topSites.data[0]?.registrableDomain).toBe('github.com')

    const searchEngines = handled(
      handlePreviewIntelligenceCommand<{
        data: Array<{ searchEngine: string }>
      }>('get_search_engine_ranking', undefined, state),
    )
    expect(searchEngines.data[0]?.searchEngine).toBe('google')

    const onThisDay = handled(
      handlePreviewIntelligenceCommand<{
        data: Array<{ totalVisits: number }>
      }>('get_on_this_day', undefined, state),
    )
    expect(onThisDay.data[0]?.totalVisits).toBeGreaterThan(0)

    const topSites = handled(
      handlePreviewIntelligenceCommand<{
        data: Array<{ registrableDomain: string }>
      }>('get_top_sites', undefined, state),
    )
    expect(topSites.data[0]?.registrableDomain).toBe('github.com')

    const refindPages = handled(
      handlePreviewIntelligenceCommand<{
        data: Array<{ registrableDomain: string }>
      }>('get_refind_pages', undefined, state),
    )
    expect(refindPages.data[0]?.registrableDomain).toBeTruthy()

    const concepts = handled(
      handlePreviewIntelligenceCommand<{
        data: Array<{ term: string }>
      }>('get_top_search_concepts', undefined, state),
    )
    expect(concepts.data[0]?.term).toBe('sqlite')

    const interruptedHabits = handled(
      handlePreviewIntelligenceCommand<{
        data: Array<unknown>
      }>('get_interrupted_habits', undefined, state),
    )
    expect(interruptedHabits.data).toEqual([])

    const activityMix = handled(
      handlePreviewIntelligenceCommand<{
        data: { categories: Array<{ domainCategory: string }> }
      }>('get_activity_mix', undefined, state),
    )
    expect(activityMix.data.categories[0]?.domainCategory).toBe('docs')

    const digest = handled(
      handlePreviewIntelligenceCommand<{
        data: { totalSearches: { value: number } }
      }>('get_digest_summary', undefined, state),
    )
    expect(digest.data.totalSearches.value).toBeGreaterThan(0)

    const dayInsights = handled(
      handlePreviewIntelligenceCommand<{
        data: { digestSummary: { totalVisits: { value: number } } }
      }>('get_day_insights', undefined, state),
    )
    expect(dayInsights.data.digestSummary.totalVisits.value).toBeGreaterThan(0)

    const secondary = handled(
      handlePreviewIntelligenceCommand<{
        stableSources: { data: Array<{ registrableDomain: string }> }
      }>('get_intelligence_secondary_overview', undefined, state),
    )
    expect(secondary.stableSources.data[0]?.registrableDomain).toBeTruthy()

    const stableSources = handled(
      handlePreviewIntelligenceCommand<{
        data: Array<{ registrableDomain: string }>
      }>('get_stable_sources', undefined, state),
    )
    expect(stableSources.data[0]?.registrableDomain).toBe('sqlite.org')

    const friction = handled(
      handlePreviewIntelligenceCommand<{ data: Array<unknown> }>(
        'get_friction_signals',
        undefined,
        state,
      ),
    )
    expect(friction.data).toEqual([])

    const reopened = handled(
      handlePreviewIntelligenceCommand<{
        data: Array<{ investigationId: string }>
      }>('get_reopened_investigations', undefined, state),
    )
    expect(reopened.data[0]?.investigationId).toBe('reopen-local-first')

    const habits = handled(
      handlePreviewIntelligenceCommand<{
        data: Array<{ registrableDomain: string }>
      }>('get_habit_patterns', undefined, state),
    )
    expect(habits.data[0]?.registrableDomain).toBe('github.com')

    const pathFlows = handled(
      handlePreviewIntelligenceCommand<{
        data: Array<{ flowId: string }>
      }>('get_path_flows', undefined, state),
    )
    expect(pathFlows.data[0]?.flowId).toBe('flow-search-docs-github')

    const compareSets = handled(
      handlePreviewIntelligenceCommand<{
        data: Array<{ compareSetId: string }>
      }>('get_compare_sets', undefined, state),
    )
    expect(compareSets.data[0]?.compareSetId).toBe('compare-preview-hosting')

    const observed = handled(
      handlePreviewIntelligenceCommand<{
        data: Array<unknown>
      }>('get_observed_interactions', undefined, state),
    )
    expect(observed.data).toEqual([])

    const discoveryTrendDay = handled(
      handlePreviewIntelligenceCommand<{
        data: { points: Array<{ dateKey: string }> }
      }>('get_discovery_trend', { request: { granularity: 'day' } }, state),
    )
    expect(discoveryTrendDay.data.points.length).toBeGreaterThan(0)

    const discoveryTrendWeek = handled(
      handlePreviewIntelligenceCommand<{
        data: { points: Array<{ dateKey: string }> }
      }>('get_discovery_trend', undefined, state),
    )
    expect(discoveryTrendWeek.data.points.length).toBeGreaterThan(0)

    const effectiveness = handled(
      handlePreviewIntelligenceCommand<{
        data: { engineStats: Array<{ searchEngine: string }> }
      }>('get_search_effectiveness', undefined, state),
    )
    expect(effectiveness.data.engineStats[0]?.searchEngine).toBe('google')

    const trend = handled(
      handlePreviewIntelligenceCommand<{
        registrableDomain: string
        points: Array<{ visitCount: number }>
      }>(
        'get_domain_trend',
        { request: { registrableDomain: 'sqlite.org' } },
        state,
      ),
    )
    expect(trend.registrableDomain).toBe('sqlite.org')
    expect(trend.points.length).toBeGreaterThan(0)

    const rhythm = handled(
      handlePreviewIntelligenceCommand<{
        cells: Array<{ visitCount: number }>
        maxCount: number
      }>('get_browsing_rhythm', undefined, state),
    )
    expect(rhythm.cells.length).toBeGreaterThan(0)
    expect(rhythm.maxCount).toBeGreaterThan(0)

    const breadth = handled(
      handlePreviewIntelligenceCommand<{ data: { breadthScore: number } }>(
        'get_breadth_index',
        undefined,
        state,
      ),
    )
    expect(breadth.data.breadthScore).toBeGreaterThan(0)

    const browserDiff = handled(
      handlePreviewIntelligenceCommand<{
        data: { profiles: Array<{ profileId: string }> }
      }>('get_multi_browser_diff', undefined, state),
    )
    expect(browserDiff.data.profiles.length).toBeGreaterThan(0)

    const families = handled(
      handlePreviewIntelligenceCommand<{
        data: { families: Array<{ anchorQuery: string }> }
      }>('get_query_families', undefined, state),
    )
    expect(families.data.families[0]?.anchorQuery).toContain('local')

    const queries = handled(
      handlePreviewIntelligenceCommand<{
        rows: Array<{ normalizedQuery: string }>
        total: number
      }>(
        'get_search_queries',
        { request: { query: 'sqlite', pageSize: 2 } },
        state,
      ),
    )
    expect(queries.total).toBeGreaterThan(0)
    expect(queries.rows[0]?.normalizedQuery).toContain('sqlite')
    expect(
      handled(
        handlePreviewIntelligenceCommand<{ total: number }>(
          'get_search_queries',
          undefined,
          state,
        ),
      ).total,
    ).toBeGreaterThanOrEqual(queries.total)

    const deepDive = handled(
      handlePreviewIntelligenceCommand<{
        data: {
          registrableDomain: string
          topPages: Array<{ path: string }>
        }
      }>(
        'get_domain_deep_dive',
        { request: { registrableDomain: 'tauri.app' } },
        state,
      ),
    )
    expect(deepDive.data.registrableDomain).toBe('tauri.app')
    expect(deepDive.data.topPages[0]?.path).toBe('/start/')
  })
})

function handled<T>(result: PreviewCommandResult<T>): T {
  if (result === PREVIEW_COMMAND_UNHANDLED) {
    throw new Error('expected preview command to be handled')
  }
  return result
}
