/**
 * @file backend-preview-intelligence-commands.ts
 * @description Browser-preview Core Intelligence command owner for the compatibility backend facade.
 * @module lib/backend-preview-intelligence-commands
 *
 * ## Responsibilities
 * - Handle the preview-only Core Intelligence read fallbacks that currently live inside `backend.ts`.
 * - Keep browser-preview search-engine-rule mutations, local-host preview/build payloads, and rebuild acknowledgements aligned with the existing fixture contract.
 * - Reuse the shared preview state and helper modules instead of inventing a second intelligence fixture source.
 *
 * ## Not responsible for
 * - Wiring this handler into the top-level preview dispatcher; `backend.ts` still decides delegation order.
 * - Owning runtime digest, AI queue, or non-`get_*` explain surfaces outside the command list assigned to this module.
 * - Changing any desktop command semantics beyond the intelligence-related preview cases already shipped in `backend.ts`.
 *
 * ## Dependencies
 * - Depends on the shared preview command sentinel contract from `./backend-preview-command-result`.
 * - Reuses `MockBackendState`, `buildMockSearchQueries`, and `clearDerivedIntelligenceFixture` from the existing preview helper modules.
 * - Depends on typed Core Intelligence contracts from `./core-intelligence`.
 *
 * ## Performance notes
 * - These handlers stay synchronous and bounded to the in-memory preview fixture surface.
 * - All responses preserve the existing lightweight fallback payloads so browser preview does not trigger extra work.
 */

import {
  PREVIEW_COMMAND_UNHANDLED,
  type PreviewCommandResult,
} from './backend-preview-command-result'
import type { MockBackendState } from './backend-preview-state'
import { buildMockSearchQueries } from './backend-preview-search'
import { clearDerivedIntelligenceFixture } from './backend-preview-support'
import type {
  IntelligenceLocalHostRequest,
  SearchEngineRule,
  SearchEngineRuleInput,
} from './core-intelligence'

/**
 * Creates the empty date-range payload shared by preview-only intelligence fallback responses.
 *
 * Several read surfaces need the same blank window contract, so keeping it here avoids subtle drift
 * between cards that should all degrade the same way in browser preview mode.
 */
function emptyDateRange() {
  return { start: '', end: '' }
}

/**
 * Builds the zeroed digest summary returned when browser preview has no real deterministic read model behind a card.
 *
 * The structure matches the shipping digest contract so route components can render honest empty states
 * without adding route-local preview special cases.
 */
function emptyDigestSummary() {
  return {
    dateRange: emptyDateRange(),
    totalVisits: { value: 0, trend: 'flat' as const },
    totalSearches: { value: 0, trend: 'flat' as const },
    newDomains: { value: 0, trend: 'flat' as const },
    deepReadPages: { value: 0, trend: 'flat' as const },
    refindPages: { value: 0, trend: 'flat' as const },
  }
}

/**
 * Produces the standard degraded section meta wrapper for preview-only intelligence detail payloads.
 *
 * This keeps all detail fallbacks explicit about being degraded instead of leaving each handler branch to
 * handcraft its own metadata shell.
 */
function degradedSectionMeta(sectionId: string) {
  return {
    sectionId,
    generatedAt: null,
    window: {
      kind: 'date-range' as const,
      dateRange: emptyDateRange(),
    },
    moduleIds: [],
    sourceTables: [],
    includesEnrichment: false,
    state: 'degraded' as const,
    stateReason: null,
    notes: [],
  }
}

/**
 * Normalizes the optional local-host preview request back to the same shape used by the existing browser fixture.
 *
 * The preview host surfaces can be invoked without args in tests, so this helper centralizes the fallback request
 * instead of repeating defensive defaults in multiple command branches.
 */
function localHostRequest(
  args: Record<string, unknown> | undefined,
): Partial<IntelligenceLocalHostRequest> {
  return (
    (args?.request as Partial<IntelligenceLocalHostRequest> | undefined) ?? {
      dateRange: emptyDateRange(),
      profileId: null,
      locale: 'en',
    }
  )
}

/**
 * Builds the trusted-local-host preview/build payload shared by both browser-preview host commands.
 *
 * The generated files and bundle JSON intentionally stay identical to the old `backend.ts` branch so tests and
 * Settings review UI keep seeing the exact same fixture surface after the dispatcher split.
 */
function localHostResponse(
  command: 'preview_intelligence_local_host' | 'build_intelligence_local_host',
  args: Record<string, unknown> | undefined,
) {
  const request = localHostRequest(args)
  const artifactRoot =
    '/tmp/pathkeep-preview/integrations/core-intelligence/browser-snippet-v1'
  const bundle = {
    bundleVersion: 'pathkeep.core-intelligence.local-host.v1',
    hostId: 'browser-snippet-v1',
    generatedAt: new Date().toISOString(),
    locale: request.locale ?? 'en',
    dateRange: {
      start: request.dateRange?.start ?? '',
      end: request.dateRange?.end ?? '',
    },
    profileId: request.profileId ?? null,
    embedCards: [
      {
        cardId: 'digest:visits',
        cardType: 'digest',
        title: 'Visits',
        eyebrow: `${request.dateRange?.start ?? ''} → ${
          request.dateRange?.end ?? ''
        }`,
        body: 'Preview fixture for the trusted local snippet host.',
        metricLabel: 'visit_count',
        metricValue: '42',
        href: null,
        internalOnly: false,
      },
    ],
    widgetSnapshot: {
      generatedAt: new Date().toISOString(),
      dateRange: {
        start: request.dateRange?.start ?? '',
        end: request.dateRange?.end ?? '',
      },
      digestSummary: {
        dateRange: {
          start: request.dateRange?.start ?? '',
          end: request.dateRange?.end ?? '',
        },
        totalVisits: { value: 42, trend: 'flat' as const },
        totalSearches: { value: 7, trend: 'flat' as const },
        newDomains: { value: 3, trend: 'flat' as const },
        deepReadPages: { value: 2, trend: 'flat' as const },
        refindPages: { value: 1, trend: 'flat' as const },
      },
      highlights: [],
      notes: ['Preview fixture for browser-only mode.'],
    },
    publicSnapshot: {
      generatedAt: new Date().toISOString(),
      dateRange: {
        start: request.dateRange?.start ?? '',
        end: request.dateRange?.end ?? '',
      },
      digestSummary: {
        dateRange: {
          start: request.dateRange?.start ?? '',
          end: request.dateRange?.end ?? '',
        },
        totalVisits: { value: 42, trend: 'flat' as const },
        totalSearches: { value: 7, trend: 'flat' as const },
        newDomains: { value: 3, trend: 'flat' as const },
        deepReadPages: { value: 2, trend: 'flat' as const },
        refindPages: { value: 1, trend: 'flat' as const },
      },
      topDomains: ['example.com'],
      searchEngines: [],
      discoveryTrend: { points: [], availableYears: [] },
      notes: ['Preview fixture for browser-only mode.'],
    },
    trustedOnlyCardIds: [],
    trustedOnlyCardCount: 0,
    boundaryNotes: [
      'Browser preview mode only simulates the trusted local host contract.',
    ],
  }

  return {
    artifactRoot,
    entryFilePath: `${artifactRoot}/index.html`,
    generatedFiles: [
      {
        relativePath:
          'integrations/core-intelligence/browser-snippet-v1/index.html',
        absolutePath: `${artifactRoot}/index.html`,
        purpose: 'Preview local browser snippet.',
        contents: '<!doctype html><title>PathKeep Preview</title>',
      },
      {
        relativePath:
          'integrations/core-intelligence/browser-snippet-v1/bundle.json',
        absolutePath: `${artifactRoot}/bundle.json`,
        purpose: 'Preview local browser snippet bundle.',
        contents: JSON.stringify(bundle, null, 2),
      },
    ],
    bundle,
    boundaryNotes: bundle.boundaryNotes,
    manualSteps: [
      'Review the generated files in Settings.',
      'Open the local snippet after creating it in the desktop build.',
    ],
    warnings: [],
    installedHost:
      command === 'build_intelligence_local_host'
        ? {
            artifactRoot,
            entryFilePath: `${artifactRoot}/index.html`,
            bundle,
          }
        : null,
  }
}

/**
 * Routes the extracted browser-preview Core Intelligence commands that belong to one shared owner.
 *
 * This keeps the intelligence-specific preview fallbacks reusable for a future `backend.ts` cutover
 * while preserving the exact payloads, errors, and mutation behavior that the legacy switch already exposes.
 */
export function handlePreviewIntelligenceCommand<T>(
  command: string,
  args: Record<string, unknown> | undefined,
  state: MockBackendState,
): PreviewCommandResult<T> {
  switch (command) {
    case 'clear_derived_intelligence':
      return clearDerivedIntelligenceFixture(state) as T
    case 'get_on_this_day':
      return [] as T
    case 'get_top_sites':
    case 'get_refind_pages':
    case 'get_search_engine_ranking':
    case 'get_top_search_concepts':
    case 'get_stable_sources':
    case 'get_friction_signals':
    case 'get_reopened_investigations':
    case 'get_habit_patterns':
    case 'get_interrupted_habits':
    case 'get_path_flows':
    case 'get_compare_sets':
    case 'get_observed_interactions':
    case 'get_hub_pages':
      return [] as T
    case 'list_search_engine_rules':
      return structuredClone(state.searchEngineRules) as T
    case 'upsert_search_engine_rule': {
      const input = (args?.input as SearchEngineRuleInput | undefined) ?? null
      if (!input) {
        return structuredClone(state.searchEngineRules) as T
      }

      const ruleId =
        input.ruleId?.trim() ||
        `custom:${input.engineId || 'engine'}:${state.searchEngineRules.length + 1}`
      const nextRule: SearchEngineRule = {
        ruleId,
        engineId: input.engineId,
        displayName: input.displayName,
        hostPattern: input.hostPattern,
        pathPrefix: input.pathPrefix ?? null,
        queryParamKey: input.queryParamKey,
        enabled: input.enabled,
        note: input.note ?? null,
        exampleUrl: input.exampleUrl ?? null,
        builtIn: false,
      }
      state.searchEngineRules = [
        ...state.searchEngineRules.filter((rule) => rule.ruleId !== ruleId),
        nextRule,
      ]
      return structuredClone(state.searchEngineRules) as T
    }
    case 'delete_search_engine_rule': {
      const ruleId =
        args &&
        typeof args === 'object' &&
        'ruleId' in args &&
        typeof args.ruleId === 'string'
          ? args.ruleId
          : ''
      state.searchEngineRules = state.searchEngineRules.filter(
        (rule) => rule.ruleId !== ruleId || rule.builtIn,
      )
      return structuredClone(state.searchEngineRules) as T
    }
    case 'get_digest_summary':
      return emptyDigestSummary() as T
    case 'get_day_insights':
      return {
        date: '',
        digestSummary: emptyDigestSummary(),
        topSites: [],
        activityMix: { categories: [], changeVsPrevious: [] },
        refindPages: [],
        queryFamilies: {
          families: [],
          total: 0,
          page: 0,
          pageSize: 8,
        },
        hourlyActivity: Array.from({ length: 24 }, (_, hour) => ({
          hour,
          visitCount: 0,
        })),
        drilldown: {
          explorerDateRange: emptyDateRange(),
        },
      } as T
    case 'get_intelligence_primary_overview':
      throw new Error(
        'PathKeep intelligence overview batching is unavailable in browser preview mode.',
      )
    case 'get_intelligence_secondary_overview':
      throw new Error(
        'PathKeep intelligence overview batching is unavailable in browser preview mode.',
      )
    case 'get_activity_mix':
      return { categories: [], changeVsPrevious: [] } as T
    case 'get_activity_mix_trend':
      return { points: [] } as T
    case 'get_discovery_trend':
      return { points: [], availableYears: [] } as T
    case 'get_browsing_rhythm':
      return { cells: [], maxCount: 0 } as T
    case 'get_breadth_index':
      return { hhi: 0, breadthScore: 0, concentrationDomainCount: 0 } as T
    case 'get_multi_browser_diff':
      return {
        profiles: [],
        exclusiveDomains: [],
        sharedDomains: [],
        categoryDistributions: [],
      } as T
    case 'get_sessions':
    case 'get_search_trails':
    case 'get_query_families':
      return {
        sessions: [],
        trails: [],
        families: [],
        total: 0,
        page: 0,
        pageSize: 20,
      } as T
    case 'get_search_queries':
      return buildMockSearchQueries(
        (args?.request as Parameters<typeof buildMockSearchQueries>[0]) ??
          undefined,
      ) as T
    case 'get_query_family_detail':
      return {
        data: {
          family: {
            familyId: '',
            anchorQuery: '',
            memberCount: 0,
            searchEngine: '',
            queries: [],
            firstSeenAt: '',
            lastSeenAt: '',
          },
          relatedTrails: [],
        },
        meta: degradedSectionMeta('query-family-detail'),
      } as T
    case 'get_search_effectiveness':
      return {
        engineStats: [],
        topResolvingSources: [],
        hardestTopics: [],
      } as T
    case 'get_domain_trend':
      return { registrableDomain: '', points: [] } as T
    case 'get_domain_deep_dive':
      return {
        registrableDomain: '',
        displayName: null,
        domainCategory: 'unknown',
        totalVisits: 0,
        activeDays: 0,
        trailCount: 0,
        arrivalBreakdown: { search: 0, link: 0, typed: 0, other: 0 },
        topPages: [],
        topReferrers: [],
        topExits: [],
        visitTrend: [],
      } as T
    case 'get_refind_page_detail':
      return {
        data: {
          page: {
            canonicalUrl: '',
            url: '',
            title: null,
            registrableDomain: '',
            crossDayCount: 0,
            trailCount: 0,
            searchArrivalCount: 0,
            typedRevisitCount: 0,
            refindScore: 0,
            firstSeenAt: '',
            lastSeenAt: '',
          },
          explanation: {
            canonicalUrl: '',
            refindScore: 0,
            factors: [],
            visitIds: [],
          },
          recentDays: [],
          relatedTrails: [],
        },
        meta: degradedSectionMeta('refind-page-detail'),
      } as T
    case 'get_compare_set_detail':
      return {
        data: {
          compareSet: {
            compareSetId: '',
            trailId: '',
            searchQuery: '',
            pageCategory: '',
            pages: [],
          },
          trail: {
            trailId: '',
            sessionId: null,
            initialQuery: '',
            searchEngine: '',
            reformulationCount: 0,
            visitCount: 0,
            landingUrl: null,
            landingDomain: null,
            firstVisitMs: 0,
            lastVisitMs: 0,
            maxDepth: 0,
            queries: [],
          },
          session: null,
          recentDays: [],
        },
        meta: degradedSectionMeta('compare-set-detail'),
      } as T
    case 'get_session_detail':
      return { session: null, visits: [], trails: [] } as T
    case 'get_trail_detail':
      return { trail: null, members: [] } as T
    case 'get_navigation_path':
      return { targetVisitId: 0, steps: [] } as T
    case 'preview_intelligence_local_host':
    case 'build_intelligence_local_host':
      return localHostResponse(command, args) as T
    case 'run_core_intelligence_now':
    case 'queue_core_intelligence_rebuild': {
      const jobId = Date.now()
      return {
        jobId,
        state: state.snapshot.config.ai.jobQueuePaused ? 'queued' : 'running',
        notes: [
          state.snapshot.config.ai.jobQueuePaused
            ? `Queued Core Intelligence rebuild job ${jobId}. Resume background work to process it.`
            : `Queued Core Intelligence rebuild job ${jobId}. PathKeep is processing it in the background.`,
        ],
      } as T
    }
    default:
      return PREVIEW_COMMAND_UNHANDLED
  }
}
