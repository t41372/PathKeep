/**
 * @file assistant-and-shell.test.tsx
 * @description Focused Assistant and shared shell/intelligence digest suite extracted from the legacy Intelligence mega-test.
 * @module pages/intelligence-surfaces
 *
 * ## Responsibilities
 * - Protect the shipped Assistant queue, provider probe, and evidence citation behavior.
 * - Preserve shared shell-scoping and compact runtime digest assertions for Intelligence route surfaces.
 * - Keep localized archive-wide copy assertions intact after the mega-suite split.
 *
 * ## Non-Responsibilities
 * - Does not cover Jobs controls, Settings external outputs, or Explorer route flows.
 * - Does not redefine the shared route harness or local-host fixtures.
 * - Does not modify the legacy mega-test during extraction.
 *
 * ## Dependencies
 * - Depends on the shared Intelligence surface harness in `test-helpers.tsx`.
 * - Uses shipped `AssistantPage`, `IntelligencePage`, and `DomainDeepDiveRoutePage` route components.
 * - Relies on typed backend and Core Intelligence API spies to keep assertions aligned with production contracts.
 *
 * ## Performance Notes
 * - Reuses the common seeded archive harness so this split suite does not duplicate setup overhead across route tests.
 */

import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Route, Routes } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import * as coreIntelligenceApi from '../../lib/core-intelligence/api'
import { createNamespaceTranslator } from '../../lib/i18n'
import type {
  AiProviderConnectionTestReport,
  AiQueueStatus,
} from '../../lib/types'
import { AssistantPage } from '../assistant'
import { DomainDeepDiveRoutePage, IntelligencePage } from '../intelligence'
import {
  createEmptyRuntimeSnapshot,
  createShellValue,
  enableAi,
  renderSurface,
  resetIntelligenceSurfaceHarness,
  seedArchiveState,
  wrapSection,
} from './test-helpers'

describe('intelligence surfaces', () => {
  beforeEach(() => {
    resetIntelligenceSurfaceHarness()
  })

  test('renders assistant queue state, provider probe, and answer citations', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const assistantT = createNamespaceTranslator('zh-CN', 'assistant')

    enableAi(snapshot)
    window.localStorage.setItem('pathkeep.profile-scope', 'chrome:Default')

    const queueStatus: AiQueueStatus = {
      paused: false,
      concurrency: 1,
      queued: 1,
      running: 1,
      failed: 0,
      recentJobs: [
        {
          id: 77,
          jobType: 'assistant',
          state: 'queued',
          priority: 10,
          attempt: 1,
          maxAttempts: 3,
          runId: null,
          summary: null,
          queuedAt: '2026-04-07T18:00:00Z',
          availableAt: '2026-04-07T18:00:00Z',
          startedAt: null,
          finishedAt: null,
          heartbeatAt: null,
          errorCode: null,
          errorMessage: null,
        },
      ],
    }
    const providerProbe: AiProviderConnectionTestReport = {
      providerId: 'llm-local',
      purpose: 'llm',
      model: 'qwen3:8b',
      ok: true,
      latencyMs: 210,
      capabilities: {
        supportsChat: true,
        supportsEmbeddings: false,
        supportsStreaming: true,
        supportsToolUse: false,
        supportsStructuredOutput: true,
      },
      warnings: [],
      message: 'Provider responded with a healthy chat completion.',
      actionHint: 'Safe to use for evidence-backed answers.',
      retryHint: null,
      errorCode: null,
    }

    vi.spyOn(backend, 'testAiProviderConnection').mockResolvedValue(
      providerProbe,
    )
    vi.spyOn(backend, 'askAiAssistant').mockResolvedValue({
      state: 'completed',
      answer: '语义搜索质量趋势稳定，证据契约文档被多次访问。',
      jobId: 91,
      runId: 12,
      providerId: 'llm-local',
      embeddingProviderId: 'embed-local',
      citations: [
        {
          historyId: 301,
          profileId: 'chrome:Default',
          url: 'https://example.com/semantic-quality',
          title: 'Semantic quality notes',
          visitedAt: '2026-04-06T15:20:00Z',
          score: 0.92,
        },
      ],
      notes: ['Answer kept inside the archive evidence boundary.'],
    })

    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue: queueStatus,
      intelligence: createEmptyRuntimeSnapshot(),
      loading: false,
      error: null,
    }

    renderSurface(<AssistantPage />, {
      language: 'zh-CN',
      route: '/assistant',
      shellValue,
      snapshot,
    })

    expect(await screen.findByText(assistantT('scopedViewTitle'))).toBeVisible()
    expect(await screen.findByText(assistantT('runningContext'))).toBeVisible()
    expect(
      await screen.findByText(assistantT('queuedJobLabel', { id: 77 })),
    ).toBeVisible()

    await user.click(
      screen.getByRole('button', { name: assistantT('testProvider') }),
    )
    expect(
      await screen.findByText(assistantT('providerReachable')),
    ).toBeVisible()

    const input = await screen.findByPlaceholderText(
      assistantT('inputPlaceholder'),
    )
    expect(
      screen.getByRole('button', { name: assistantT('sendAction') }),
    ).toBeVisible()
    await user.type(input, '总结最近的证据{enter}')

    expect(
      await screen.findByText('语义搜索质量趋势稳定，证据契约文档被多次访问。'),
    ).toBeVisible()
    expect(
      await screen.findByText(assistantT('evidenceLabel', { count: 1 })),
    ).toBeVisible()

    window.localStorage.removeItem('pathkeep.profile-scope')
  })

  test('inherits shared intelligence scope and lets explicit profileId override it', async () => {
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')
    const summary = {
      dateRange: { start: '2026-04-01', end: '2026-04-07' },
      totalVisits: { value: 12, trend: 'flat' as const },
      totalSearches: { value: 3, trend: 'flat' as const },
      newDomains: { value: 2, trend: 'flat' as const },
      deepReadPages: { value: 1, trend: 'flat' as const },
      refindPages: { value: 1, trend: 'flat' as const },
    }
    const digestSpy = vi
      .spyOn(coreIntelligenceApi, 'getDigestSummary')
      .mockResolvedValue(wrapSection('digest-summary', summary))

    window.localStorage.setItem('pathkeep.profile-scope', 'chrome:Default')

    try {
      const first = renderSurface(<IntelligencePage />, {
        route: '/intelligence',
        snapshot,
      })

      expect(await screen.findByTestId('intelligence-page')).toBeVisible()
      await waitFor(() =>
        expect(digestSpy).toHaveBeenCalledWith(
          expect.anything(),
          'chrome:Default',
        ),
      )
      expect(
        await screen.findByText(
          intelligenceT('scopedViewBody', { profile: 'Default' }),
        ),
      ).toBeVisible()

      first.unmount()
      digestSpy.mockClear()

      renderSurface(<IntelligencePage />, {
        route: '/intelligence?profileId=firefox:Research',
        snapshot,
      })

      await waitFor(() =>
        expect(digestSpy).toHaveBeenCalledWith(
          expect.anything(),
          'firefox:Research',
        ),
      )
      expect(
        await screen.findByText(
          intelligenceT('scopedViewBody', { profile: 'Research' }),
        ),
      ).toBeVisible()
    } finally {
      window.localStorage.removeItem('pathkeep.profile-scope')
    }
  })

  test('shows a compact runtime digest without a full-width settings banner', async () => {
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')
    const shellValue = createShellValue(snapshot, null)
    shellValue.runtimeStatus = {
      aiQueue: {
        paused: false,
        concurrency: 1,
        queued: 0,
        running: 0,
        failed: 0,
        recentJobs: [],
      },
      intelligence: {
        queue: {
          queued: 1,
          running: 1,
          succeeded: 0,
          failed: 0,
          cancelled: 0,
          lastActivityAt: '2026-04-17T09:40:00Z',
        },
        plugins: [],
        modules: [],
        recentJobs: [
          {
            id: 812,
            jobType: 'deterministic-rebuild',
            pluginId: null,
            state: 'running',
            historyId: null,
            profileId: 'chrome:Default',
            url: null,
            title: 'chrome:Default · 30 days',
            attempt: 1,
            createdAt: '2026-04-17T09:35:00Z',
            startedAt: '2026-04-17T09:36:00Z',
            finishedAt: null,
            updatedAt: '2026-04-17T09:40:00Z',
            heartbeatAt: '2026-04-17T09:40:00Z',
            progressLabel: 'Scoring visits',
            progressDetail: '24,000 / 64,781 visits',
            progressCurrent: 24000,
            progressTotal: 64781,
            progressPercent: 46.8,
            lastError: null,
            retryable: false,
            cancellable: true,
          },
        ],
        notes: [],
      },
      loading: false,
      error: null,
    }

    renderSurface(<IntelligencePage />, {
      route: '/intelligence',
      shellValue,
      snapshot,
    })

    const digest = await screen.findByTestId('intelligence-runtime-digest')
    expect(
      within(digest).getByText(intelligenceT('runtimeDigestTitle')),
    ).toBeVisible()
    expect(
      within(digest).getByText(
        intelligenceT('runtimeDigestRunningTitle', { count: 1 }),
      ),
    ).toBeVisible()
    expect(within(digest).getByText('24,000 / 64,781 visits')).toBeVisible()
    expect(within(digest).getByRole('link', { name: 'Jobs' })).toHaveAttribute(
      'href',
      '/jobs',
    )
    expect(
      screen.queryByText(intelligenceT('externalOutputsReviewTitle')),
    ).not.toBeInTheDocument()
  })

  test('keeps intelligence digest icons decorative instead of exposing raw ids', async () => {
    const { snapshot, dashboard } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')

    renderSurface(<IntelligencePage />, {
      dashboard,
      route: '/intelligence',
      snapshot,
    })

    expect(
      await screen.findByRole('heading', {
        name: intelligenceT('runtimeDigestTitle'),
      }),
    ).toBeVisible()
    expect(screen.queryByText('bar_chart')).not.toBeInTheDocument()
    expect(screen.queryByText('auto_stories')).not.toBeInTheDocument()
    expect(screen.queryByText('sync')).not.toBeInTheDocument()
  })

  test('renders archive-wide copy and decoded domain paths without raw keys in zh-TW', async () => {
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('zh-TW', 'intelligence')

    vi.spyOn(coreIntelligenceApi, 'getDigestSummary').mockResolvedValue(
      wrapSection('digest-summary', {
        dateRange: { start: '2026-04-01', end: '2026-04-07' },
        totalVisits: { value: 12, trend: 'flat' as const },
        totalSearches: { value: 3, trend: 'flat' as const },
        newDomains: { value: 2, trend: 'flat' as const },
        deepReadPages: { value: 1, trend: 'flat' as const },
        refindPages: { value: 1, trend: 'flat' as const },
      }),
    )
    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
      createEmptyRuntimeSnapshot(),
    )
    vi.spyOn(coreIntelligenceApi, 'getOnThisDay').mockResolvedValue(
      wrapSection('on-this-day', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getTopSites').mockResolvedValue(
      wrapSection('top-sites', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getSearchEngineRanking').mockResolvedValue(
      wrapSection('engine-ranking', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getTopSearchConcepts').mockResolvedValue(
      wrapSection('search-concepts', [
        {
          term: 'sqlite',
          frequency: 4,
          engines: ['google'],
        },
      ]),
    )
    vi.spyOn(coreIntelligenceApi, 'getQueryFamilies').mockResolvedValue(
      wrapSection('query-families', {
        page: 0,
        pageSize: 20,
        total: 0,
        families: [],
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getRefindPages').mockResolvedValue(
      wrapSection('refind-pages', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getActivityMix').mockResolvedValue(
      wrapSection('activity-mix', {
        categories: [
          { domainCategory: 'community', visitCount: 5, share: 0.25 },
          { domainCategory: 'search', visitCount: 15, share: 0.75 },
        ],
        changeVsPrevious: [],
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getBrowsingRhythm').mockResolvedValue(
      wrapSection('browsing-rhythm', {
        cells: [],
        maxCount: 0,
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getStableSources').mockResolvedValue(
      wrapSection('stable-sources', [
        {
          registrableDomain: 'wikipedia.org',
          displayName: 'wikipedia.org',
          sourceRole: 'landing',
          trailCount: 0,
          stableLandingCount: 1,
          effectivenessScore: 0.1,
        },
      ]),
    )
    vi.spyOn(coreIntelligenceApi, 'getSearchEffectiveness').mockResolvedValue(
      wrapSection('search-effectiveness', {
        engineStats: [],
        topResolvingSources: [],
        hardestTopics: [],
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getFrictionSignals').mockResolvedValue(
      wrapSection('friction-signals', [
        {
          registrableDomain: 'example.com',
          url: 'https://example.com/article',
          evidenceType: 'weak',
          signalKind: 'single_bounce',
          occurrenceCount: 1,
          description: 'Came back to search once.',
        },
      ]),
    )
    vi.spyOn(
      coreIntelligenceApi,
      'getReopenedInvestigations',
    ).mockResolvedValue(
      wrapSection('reopened-investigations', [
        {
          investigationId: 'query::chatgpt',
          anchorType: 'query_family',
          anchorId: 'chatgpt',
          anchorLabel: 'ChatGPT',
          occurrenceCount: 3,
          distinctDays: 3,
          firstSeenAt: '2026-04-01',
          lastSeenAt: '2026-04-07',
        },
      ]),
    )
    vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockResolvedValue(
      wrapSection('discovery-trend', { points: [], availableYears: [] }),
    )
    vi.spyOn(coreIntelligenceApi, 'getBreadthIndex').mockResolvedValue(
      wrapSection('breadth-index', {
        breadthScore: 42,
        hhi: 0.42,
        concentrationDomainCount: 5,
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getPathFlows').mockResolvedValue(
      wrapSection('path-flows', [
        {
          flowId: 'chatgpt-hop',
          flowPattern: 'chat.openai.com → chatgpt.com',
          stepCount: 2,
          occurrenceCount: 6,
          lastSeenAt: '2026-04-07T10:00:00Z',
          steps: [
            {
              index: 0,
              label: 'chat.openai.com',
              registrableDomain: 'openai.com',
            },
            {
              index: 1,
              label: 'chatgpt.com',
              registrableDomain: 'chatgpt.com',
            },
          ],
        },
      ]),
    )
    vi.spyOn(coreIntelligenceApi, 'getHabitPatterns').mockResolvedValue(
      wrapSection('habit-patterns', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getInterruptedHabits').mockResolvedValue(
      wrapSection('interrupted-habits', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getCompareSets').mockResolvedValue(
      wrapSection('compare-sets', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getMultiBrowserDiff').mockResolvedValue(
      wrapSection('multi-browser-diff', {
        profiles: [],
        sharedDomains: [],
        exclusiveDomains: [],
        categoryDistributions: [],
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getObservedInteractions').mockResolvedValue(
      wrapSection('observed-interactions', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getDomainDeepDive').mockResolvedValue(
      wrapSection('domain-deep-dive', {
        registrableDomain: 'github.com',
        displayName: 'GitHub',
        domainCategory: 'community',
        totalVisits: 38,
        activeDays: 7,
        trailCount: 4,
        arrivalBreakdown: { search: 10, link: 12, typed: 8, other: 8 },
        topPages: [
          {
            path: '/wiki/%E5%93%88%E5%B8%83%E6%96%AF%E5%A0%A1%E5%90%9B%E4%B8%BB%E5%9C%8B',
            visitCount: 12,
          },
        ],
        topReferrers: [],
        topExits: [],
        visitTrend: [],
      }),
    )

    const first = renderSurface(<IntelligencePage />, {
      language: 'zh-TW',
      route: '/intelligence',
      snapshot,
    })

    expect(
      await screen.findByText(intelligenceT('activityMixHelp')),
    ).toBeVisible()
    expect(
      (await screen.findAllByText(intelligenceT('archiveWideBadge'))).length,
    ).toBeGreaterThan(0)
    expect(
      screen.queryByText(intelligenceT('archiveWideBody')),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByText(intelligenceT('externalOutputsReviewBody')),
    ).not.toBeInTheDocument()
    expect(
      screen.getAllByText(intelligenceT('category_community')).length,
    ).toBeGreaterThan(0)
    expect(screen.getByText(intelligenceT('activityMixHelp'))).toBeVisible()
    expect(
      screen.queryByText(intelligenceT('stableSourcesTitle')),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByText(intelligenceT('searchEffectivenessTitle')),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByText(intelligenceT('frictionTitle')),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByText(intelligenceT('reopenedTitle')),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByText('intelligence.archiveWideBadge'),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByText('intelligence.archiveWideBody'),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByText('Intelligence.category_community'),
    ).not.toBeInTheDocument()

    first.unmount()

    renderSurface(
      <Routes>
        <Route
          path="/intelligence/domain/:domain"
          element={<DomainDeepDiveRoutePage />}
        />
      </Routes>,
      {
        language: 'zh-TW',
        route: '/intelligence/domain/github.com?range=month',
        snapshot,
      },
    )

    expect(await screen.findByText('/wiki/哈布斯堡君主國')).toBeVisible()
    expect(
      screen.queryByText(/%E5%93%88%E5%B8%83%E6%96%AF/i),
    ).not.toBeInTheDocument()
  })
})
