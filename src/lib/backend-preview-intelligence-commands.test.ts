import { afterEach, describe, expect, test, vi } from 'vitest'
import {
  PREVIEW_COMMAND_UNHANDLED,
  type PreviewCommandResult,
} from './backend-preview-command-result'
import { handlePreviewIntelligenceCommand } from './backend-preview-intelligence-commands'
import { createMockState } from './backend-preview-state'

describe('handlePreviewIntelligenceCommand', () => {
  afterEach(() => {
    vi.useRealTimers()
  })

  test('manages preview search-engine rules without mutating returned clones', () => {
    const state = createMockState()
    const originalLength = state.searchEngineRules.length

    const listed = handled(
      handlePreviewIntelligenceCommand<unknown[]>(
        'list_search_engine_rules',
        undefined,
        state,
      ),
    )
    expect(listed).toHaveLength(originalLength)
    listed.pop()
    expect(state.searchEngineRules).toHaveLength(originalLength)

    expect(
      handlePreviewIntelligenceCommand(
        'upsert_search_engine_rule',
        undefined,
        state,
      ),
    ).toHaveLength(originalLength)

    const upserted = handled(
      handlePreviewIntelligenceCommand<
        Array<{ ruleId: string; displayName: string; builtIn: boolean }>
      >(
        'upsert_search_engine_rule',
        {
          input: {
            ruleId: ' ',
            engineId: '',
            displayName: 'Preview Search',
            hostPattern: 'preview.example',
            pathPrefix: undefined,
            queryParamKey: 'q',
            enabled: true,
            note: undefined,
            exampleUrl: undefined,
          },
        },
        state,
      ),
    )
    expect(upserted.at(-1)).toMatchObject({
      ruleId: `custom:engine:${originalLength + 1}`,
      displayName: 'Preview Search',
      builtIn: false,
    })

    const afterInvalidDelete = handled(
      handlePreviewIntelligenceCommand<unknown[]>(
        'delete_search_engine_rule',
        {
          ruleId: 42,
        },
        state,
      ),
    )
    expect(afterInvalidDelete).toHaveLength(originalLength + 1)

    const afterCustomDelete = handled(
      handlePreviewIntelligenceCommand<Array<{ ruleId: string }>>(
        'delete_search_engine_rule',
        {
          ruleId: `custom:engine:${originalLength + 1}`,
        },
        state,
      ),
    )
    expect(afterCustomDelete.map((rule) => rule.ruleId)).not.toContain(
      `custom:engine:${originalLength + 1}`,
    )

    const afterBuiltInDelete = handled(
      handlePreviewIntelligenceCommand<Array<{ ruleId: string }>>(
        'delete_search_engine_rule',
        {
          ruleId: 'builtin:google',
        },
        state,
      ),
    )
    expect(afterBuiltInDelete.map((rule) => rule.ruleId)).toContain(
      'builtin:google',
    )
  })

  test('returns bounded preview payloads for empty deterministic intelligence reads', () => {
    const state = createMockState()

    for (const command of [
      'clear_derived_intelligence',
      'get_on_this_day',
      'get_top_sites',
      'get_refind_pages',
      'get_search_engine_ranking',
      'get_top_search_concepts',
      'get_stable_sources',
      'get_friction_signals',
      'get_reopened_investigations',
      'get_habit_patterns',
      'get_interrupted_habits',
      'get_path_flows',
      'get_compare_sets',
      'get_observed_interactions',
      'get_hub_pages',
      'get_digest_summary',
      'get_day_insights',
      'get_activity_mix',
      'get_discovery_trend',
      'get_breadth_index',
      'get_multi_browser_diff',
      'get_sessions',
      'get_search_trails',
      'get_query_families',
      'get_search_effectiveness',
    ]) {
      expect(
        handlePreviewIntelligenceCommand(command, undefined, state),
      ).not.toBe(PREVIEW_COMMAND_UNHANDLED)
    }
    expect(
      handlePreviewIntelligenceCommand(
        'get_activity_mix_trend',
        undefined,
        state,
      ),
    ).toEqual({ points: [] })
    expect(
      handlePreviewIntelligenceCommand('get_browsing_rhythm', undefined, state),
    ).toEqual({ cells: [], maxCount: 0 })
    expect(
      handled(
        handlePreviewIntelligenceCommand<{
          refindPages: { value: number; trend: string }
        }>('get_digest_summary', undefined, state),
      ).refindPages,
    ).toEqual({ value: 0, trend: 'flat' })
    expect(
      handlePreviewIntelligenceCommand('get_domain_trend', undefined, state),
    ).toEqual({ registrableDomain: '', points: [] })
    expect(
      handlePreviewIntelligenceCommand(
        'get_domain_deep_dive',
        undefined,
        state,
      ),
    ).toMatchObject({
      registrableDomain: '',
      topPages: [],
      visitTrend: [],
    })
    expect(
      handlePreviewIntelligenceCommand('get_session_detail', undefined, state),
    ).toEqual({ session: null, visits: [], trails: [] })
    expect(
      handlePreviewIntelligenceCommand('get_trail_detail', undefined, state),
    ).toEqual({ trail: null, members: [] })
    expect(
      handlePreviewIntelligenceCommand('get_navigation_path', undefined, state),
    ).toEqual({ targetVisitId: 0, steps: [] })
    expect(
      handlePreviewIntelligenceCommand('get_search_queries', undefined, state),
    ).toMatchObject({ total: 3 })
    expect(
      handlePreviewIntelligenceCommand(
        'get_search_queries',
        { request: { sort: 'family-frequency', pageSize: 1 } },
        state,
      ),
    ).toMatchObject({ pageSize: 1 })
  })

  test('wraps detail fallbacks with degraded section metadata', () => {
    const state = createMockState()

    expect(
      handled(
        handlePreviewIntelligenceCommand<{
          meta: { sectionId: string; state: string }
        }>('get_query_family_detail', undefined, state),
      ).meta,
    ).toMatchObject({
      sectionId: 'query-family-detail',
      state: 'degraded',
    })
    expect(
      handled(
        handlePreviewIntelligenceCommand<{
          meta: { sectionId: string; state: string }
        }>('get_refind_page_detail', undefined, state),
      ).meta,
    ).toMatchObject({
      sectionId: 'refind-page-detail',
      state: 'degraded',
    })
    expect(
      handled(
        handlePreviewIntelligenceCommand<{
          meta: { sectionId: string; state: string }
        }>('get_compare_set_detail', undefined, state),
      ).meta,
    ).toMatchObject({
      sectionId: 'compare-set-detail',
      state: 'degraded',
    })
  })

  test('builds trusted local-host preview payloads and rebuild acknowledgements', () => {
    const state = createMockState()
    vi.setSystemTime(new Date('2026-04-25T12:00:00Z'))

    const preview = handled(
      handlePreviewIntelligenceCommand<{
        bundle: {
          dateRange: { start: string; end: string }
          locale: string
          profileId: string | null
        }
        installedHost: unknown
      }>('preview_intelligence_local_host', undefined, state),
    )
    expect(preview.bundle).toMatchObject({
      dateRange: { start: '', end: '' },
      locale: 'en',
      profileId: null,
    })
    expect(preview.installedHost).toBeNull()

    const built = handled(
      handlePreviewIntelligenceCommand<{
        bundle: { dateRange: { start: string; end: string }; locale: string }
        installedHost: unknown
      }>(
        'build_intelligence_local_host',
        {
          request: {
            dateRange: { start: '2026-04-01', end: '2026-04-30' },
            locale: 'zh-TW',
            profileId: 'chrome:Default',
          },
        },
        state,
      ),
    )
    expect(built.bundle).toMatchObject({
      dateRange: { start: '2026-04-01', end: '2026-04-30' },
      locale: 'zh-TW',
    })
    expect(built.installedHost).not.toBeNull()

    const partial = handled(
      handlePreviewIntelligenceCommand<{
        bundle: {
          dateRange: { start: string; end: string }
          locale: string
          profileId: string | null
        }
      }>('preview_intelligence_local_host', { request: {} }, state),
    )
    expect(partial.bundle).toMatchObject({
      dateRange: { start: '', end: '' },
      locale: 'en',
      profileId: null,
    })

    vi.setSystemTime(new Date('2026-04-25T12:00:01Z'))
    expect(
      handlePreviewIntelligenceCommand<{
        state: string
        notes: string[]
      }>('run_core_intelligence_now', undefined, state),
    ).toMatchObject({
      state: 'running',
    })

    state.snapshot.config.ai.jobQueuePaused = true
    expect(
      handlePreviewIntelligenceCommand<{
        state: string
        notes: string[]
      }>('queue_core_intelligence_rebuild', undefined, state),
    ).toMatchObject({
      state: 'queued',
    })
  })

  test('throws for unavailable overview batching and leaves unknown commands unhandled', () => {
    const state = createMockState()

    expect(() =>
      handlePreviewIntelligenceCommand(
        'get_intelligence_primary_overview',
        undefined,
        state,
      ),
    ).toThrow(/overview batching/)
    expect(() =>
      handlePreviewIntelligenceCommand(
        'get_intelligence_secondary_overview',
        undefined,
        state,
      ),
    ).toThrow(/overview batching/)
    expect(
      handlePreviewIntelligenceCommand('not_a_real_command', undefined, state),
    ).toBe(PREVIEW_COMMAND_UNHANDLED)
  })
})

function handled<T>(result: PreviewCommandResult<T>): T {
  if (result === PREVIEW_COMMAND_UNHANDLED) {
    throw new Error('expected preview command to be handled')
  }
  return result
}
