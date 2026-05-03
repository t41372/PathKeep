/**
 * @file jobs-runtime.test.tsx
 * @description Focused Jobs runtime surface suite extracted from the legacy Intelligence mega-test.
 * @module pages/intelligence-surfaces
 *
 * ## Responsibilities
 * - Protect the shipped Jobs route behavior around queue controls, retry actions, and runtime truth ownership.
 * - Keep the shell-owned runtime contract honest after the mega-suite split.
 * - Reuse the shared Intelligence surface harness instead of rebuilding route providers locally.
 *
 * ## Non-Responsibilities
 * - Does not cover Assistant, Settings, or Intelligence route assertions.
 * - Does not redefine the shared render/reset harness used by sibling split suites.
 * - Does not own local-host fixture builders or cross-surface route wiring tests.
 *
 * ## Dependencies
 * - Depends on the shared test harness in `test-helpers.tsx`.
 * - Uses typed backend client spies for retry/replay and runtime polling assertions.
 * - Renders the shipped `JobsPage` route component directly.
 *
 * ## Performance Notes
 * - Reuses seeded archive state from the shared harness so the split suite stays deterministic without extra fixture churn.
 */

import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import type { ShellTask } from '../../app/shell-tasks'
import { createNamespaceTranslator } from '../../lib/i18n'
import type {
  AiQueueStatus,
  IntelligenceRuntimeSnapshot,
} from '../../lib/types'
import { JobsPage } from '../jobs'
import {
  createShellValue,
  renderSurface,
  resetIntelligenceSurfaceHarness,
  seedArchiveState,
} from './test-helpers'

describe('intelligence surfaces', () => {
  beforeEach(() => {
    resetIntelligenceSurfaceHarness()
  })

  test('renders Jobs loading, setup, locked, runtime-loading, and runtime-error gates', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')
    const commonT = createNamespaceTranslator('en', 'common')

    const shellLoadingRender = renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue: {
        ...createShellValue(snapshot),
        snapshot: null,
        loading: true,
      },
      snapshot,
    })

    expect(screen.getByText(jobsT('loadingPage'))).toBeVisible()
    shellLoadingRender.unmount()

    const uninitializedSnapshot = structuredClone(snapshot)
    uninitializedSnapshot.config.initialized = false
    const setupRender = renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      snapshot: uninitializedSnapshot,
    })

    expect(screen.getByText(jobsT('setupTitle'))).toBeVisible()
    expect(
      screen.getByRole('link', { name: commonT('initializeFirst') }),
    ).toHaveAttribute('href', '/onboarding')
    setupRender.unmount()

    const lockedSnapshot = structuredClone(snapshot)
    lockedSnapshot.archiveStatus.unlocked = false
    const lockedRender = renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      snapshot: lockedSnapshot,
    })

    expect(screen.getByText(jobsT('lockedTitle'))).toBeVisible()
    expect(
      screen.getByRole('link', { name: commonT('reviewSecurity') }),
    ).toHaveAttribute('href', '/security')
    lockedRender.unmount()

    const runtimeLoadingRender = renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue: {
        ...createShellValue(snapshot),
        runtimeStatus: {
          aiQueue: null,
          intelligence: null,
          loading: false,
          error: null,
        },
      },
      snapshot,
    })

    expect(screen.getByText(jobsT('loadingPage'))).toBeVisible()
    runtimeLoadingRender.unmount()

    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue: null,
      intelligence: null,
      loading: false,
      error: 'runtime bridge unavailable',
    }
    shellValue.refreshAppData = vi.fn().mockResolvedValue(undefined)
    shellValue.refreshRuntimeStatus = vi
      .fn()
      .mockResolvedValue(shellValue.runtimeStatus)

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot,
    })

    expect(screen.getByText(jobsT('pageUnavailableTitle'))).toBeVisible()
    expect(screen.getByText('runtime bridge unavailable')).toBeVisible()
    await user.click(screen.getByRole('button', { name: jobsT('refresh') }))

    await waitFor(() => expect(shellValue.refreshAppData).toHaveBeenCalled())
    expect(shellValue.refreshRuntimeStatus).toHaveBeenCalled()
  })

  test('surfaces a paused queued backlog and resumes the queue through config', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')
    snapshot.config.ai.jobQueuePaused = true
    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue: queueFixture({ queued: 2, recentJobs: [] }),
      intelligence: runtimeFixture({
        queue: {
          ...runtimeFixture().queue,
          queued: 1,
        },
      }),
      loading: false,
      error: null,
    }
    shellValue.saveConfig = vi.fn().mockResolvedValue({
      ...snapshot,
      config: {
        ...snapshot.config,
        ai: { ...snapshot.config.ai, jobQueuePaused: false },
      },
    })

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot,
    })

    expect(await screen.findByText(jobsT('pausedTitle'))).toBeVisible()

    await user.click(screen.getByRole('button', { name: jobsT('resumeQueue') }))

    await waitFor(() =>
      expect(shellValue.saveConfig).toHaveBeenCalledWith(
        expect.objectContaining({
          ai: expect.objectContaining({ jobQueuePaused: false }),
        }),
      ),
    )
  })

  test('renders background jobs controls and lets the user pause or replay work', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')
    const queueStatus: AiQueueStatus = {
      paused: false,
      concurrency: 2,
      queued: 1,
      running: 1,
      failed: 1,
      recentJobs: [
        {
          id: 77,
          jobType: 'index-build',
          state: 'failed',
          priority: 10,
          attempt: 2,
          maxAttempts: 3,
          runId: null,
          summary: 'Provider quota window has not reset yet.',
          queuedAt: '2026-04-07T18:00:00Z',
          availableAt: '2026-04-07T18:00:00Z',
          startedAt: '2026-04-07T18:01:00Z',
          finishedAt: '2026-04-07T18:02:00Z',
          heartbeatAt: '2026-04-07T18:01:30Z',
          errorCode: 'rate-limited',
          errorMessage: '429',
        },
        {
          id: 78,
          jobType: 'assistant',
          state: 'queued',
          priority: 10,
          attempt: 1,
          maxAttempts: 3,
          runId: null,
          summary: null,
          queuedAt: '2026-04-07T18:03:00Z',
          availableAt: '2026-04-07T18:03:00Z',
          startedAt: null,
          finishedAt: null,
          heartbeatAt: null,
          errorCode: null,
          errorMessage: null,
        },
      ],
    }
    const runtimeSnapshot: IntelligenceRuntimeSnapshot = {
      queue: {
        queued: 1,
        running: 1,
        succeeded: 0,
        failed: 0,
        cancelled: 0,
        lastActivityAt: '2026-04-10T16:30:00Z',
      },
      plugins: [
        {
          pluginId: 'readable-content-refetch',
          sourceKind: 'network',
          enabled: true,
          storedRecords: 5,
          queuedJobs: 1,
          runningJobs: 0,
          failedJobs: 1,
          lastCompletedAt: '2026-04-10T16:20:00Z',
          lastError: '429 from upstream host',
        },
      ],
      modules: [
        {
          moduleId: 'sessions',
          enabled: true,
          version: 'ci-v1',
          status: 'stale',
          dependsOn: ['visit-derived-facts'],
          derivedTables: ['sessions'],
          lastRunId: 12,
          lastBuiltAt: '2026-04-10T16:25:00Z',
          lastInvalidatedAt: '2026-04-10T16:28:00Z',
          staleReason: 'New imports were added after the last rebuild.',
          notes: ['Session grouping stayed stable across the latest rebuild.'],
        },
      ],
      recentJobs: [
        {
          id: 411,
          jobType: 'deterministic-rebuild',
          pluginId: null,
          state: 'running',
          historyId: null,
          profileId: 'chrome:Default',
          url: null,
          title: 'chrome:Default · 30 days',
          attempt: 2,
          createdAt: '2026-04-10T15:35:00Z',
          startedAt: '2026-04-10T15:36:00Z',
          finishedAt: null,
          updatedAt: '2026-04-10T15:36:45Z',
          heartbeatAt: '2026-04-10T15:36:45Z',
          progressLabel: 'Scoring visits',
          progressDetail: '24,000 / 64,781 visits',
          progressCurrent: 24000,
          progressTotal: 64781,
          progressPercent: 46.8,
          lastError: null,
          retryable: false,
          cancellable: true,
        },
        {
          id: 412,
          jobType: 'enrichment-plugin',
          pluginId: 'readable-content-refetch',
          state: 'failed',
          historyId: 2,
          profileId: 'chrome:Default',
          url: 'https://example.com/article',
          title: 'Article',
          attempt: 2,
          createdAt: '2026-04-10T15:20:00Z',
          startedAt: '2026-04-10T15:21:00Z',
          finishedAt: '2026-04-10T15:22:00Z',
          updatedAt: '2026-04-10T15:22:00Z',
          heartbeatAt: null,
          progressLabel: null,
          progressDetail: null,
          progressCurrent: null,
          progressTotal: null,
          progressPercent: null,
          lastError: '429 from upstream host',
          retryable: true,
          cancellable: false,
        },
      ],
      notes: [
        'Recovered 1 interrupted deterministic rebuild job after restart.',
      ],
    }

    const replaySpy = vi
      .spyOn(backend, 'replayAiJob')
      .mockResolvedValue(queueStatus.recentJobs[0])
    const retrySpy = vi
      .spyOn(backend, 'retryIntelligenceJob')
      .mockResolvedValue(runtimeSnapshot)

    const pausedSnapshot = structuredClone(snapshot)
    pausedSnapshot.config.ai.jobQueuePaused = true
    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue: queueStatus,
      intelligence: runtimeSnapshot,
      loading: false,
      error: null,
    }
    shellValue.saveConfig = vi.fn().mockResolvedValue(pausedSnapshot)

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot,
    })

    expect(await screen.findByText(jobsT('failedTitle'))).toBeVisible()
    expect(screen.getByText(jobsT('statusEyebrow'))).toBeVisible()
    expect(screen.getByText(jobsT('runtimeSummaryTitle'))).toBeVisible()
    expect(screen.getByText(jobsT('runtimeHealthTitle'))).toBeVisible()
    expect(screen.getByText(jobsT('pluginsTitle'))).toBeVisible()
    expect(
      screen.getAllByText('Readable content fetcher').length,
    ).toBeGreaterThan(0)
    expect(screen.getByText('Sessions')).toBeVisible()
    expect(
      screen.getByText('New imports were added after the last rebuild.'),
    ).toBeVisible()
    expect(screen.getByText('Scoring visits')).toBeVisible()
    expect(
      screen.getAllByText('24,000 / 64,781 visits').length,
    ).toBeGreaterThan(0)
    expect(screen.getByText('47%')).toBeVisible()

    const recentActivityHeading = document.getElementById(
      'jobs-recent-activity',
    )
    expect(recentActivityHeading).toBeInstanceOf(HTMLElement)
    if (!(recentActivityHeading instanceof HTMLElement)) {
      throw new Error('expected recent activity heading')
    }
    const scrollIntoView = vi.fn()
    const focusRecentActivity = vi
      .spyOn(recentActivityHeading, 'focus')
      .mockImplementation(() => undefined)
    Object.defineProperty(recentActivityHeading, 'scrollIntoView', {
      configurable: true,
      value: scrollIntoView,
    })
    Object.defineProperty(window, 'matchMedia', {
      configurable: true,
      value: undefined,
    })
    await user.click(
      screen.getByRole('link', {
        name: jobsT('jumpToFailures', { count: 2 }),
      }),
    )
    expect(scrollIntoView).toHaveBeenLastCalledWith({
      behavior: 'smooth',
      block: 'start',
    })
    expect(recentActivityHeading).toHaveAttribute('tabindex', '-1')
    expect(focusRecentActivity).toHaveBeenCalledWith({ preventScroll: true })

    Object.defineProperty(window, 'matchMedia', {
      configurable: true,
      value: vi.fn().mockReturnValue({ matches: true }),
    })
    await user.click(
      screen.getByRole('link', {
        name: jobsT('jumpToFailures', { count: 2 }),
      }),
    )
    expect(scrollIntoView).toHaveBeenLastCalledWith({
      behavior: 'auto',
      block: 'start',
    })

    Object.defineProperty(window, 'matchMedia', {
      configurable: true,
      value: vi.fn().mockReturnValue({ matches: false }),
    })
    await user.click(
      screen.getByRole('link', {
        name: jobsT('jumpToFailures', { count: 2 }),
      }),
    )
    expect(scrollIntoView).toHaveBeenLastCalledWith({
      behavior: 'smooth',
      block: 'start',
    })

    vi.spyOn(document, 'getElementById').mockReturnValueOnce(null)
    await user.click(
      screen.getByRole('link', {
        name: jobsT('jumpToFailures', { count: 2 }),
      }),
    )

    await user.click(screen.getByRole('button', { name: jobsT('pauseQueue') }))
    await waitFor(() =>
      expect(shellValue.saveConfig).toHaveBeenCalledWith(
        expect.objectContaining({
          ai: expect.objectContaining({ jobQueuePaused: true }),
        }),
      ),
    )

    const aiPanel = screen.getByText(jobsT('recentAiJobs')).closest('.panel')
    expect(aiPanel).not.toBeNull()
    if (!(aiPanel instanceof HTMLElement)) {
      throw new Error('expected recent ai jobs panel')
    }
    await user.click(
      within(aiPanel).getAllByRole('button', { name: jobsT('retryJob') })[0],
    )
    expect(replaySpy).toHaveBeenCalledWith(77)

    const runtimePanel = screen
      .getByText(jobsT('recentRuntimeJobs'))
      .closest('.panel')
    expect(runtimePanel).not.toBeNull()
    if (!(runtimePanel instanceof HTMLElement)) {
      throw new Error('expected recent runtime jobs panel')
    }
    await user.click(
      within(runtimePanel).getByRole('button', { name: jobsT('retryJob') }),
    )
    expect(retrySpy).toHaveBeenCalledWith(412)
  })

  test('keeps failed backlog honest even when the latest runtime item is still running', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const shellValue = createShellValue(snapshot)
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
          queued: 3,
          running: 1,
          succeeded: 0,
          failed: 2,
          cancelled: 0,
          lastActivityAt: '2026-04-10T16:30:00Z',
        },
        plugins: [
          {
            pluginId: 'readable-content-refetch',
            sourceKind: 'network',
            enabled: true,
            storedRecords: 5,
            queuedJobs: 3,
            runningJobs: 1,
            failedJobs: 2,
            lastCompletedAt: '2026-04-10T16:20:00Z',
            lastError: 'unsupported-content',
          },
        ],
        modules: [],
        recentJobs: [
          {
            id: 990,
            jobType: 'enrichment-plugin',
            pluginId: 'readable-content-refetch',
            state: 'running',
            historyId: 2,
            profileId: 'chrome:Default',
            url: 'https://example.com/article',
            title: 'Article',
            attempt: 1,
            createdAt: '2026-04-10T15:20:00Z',
            startedAt: '2026-04-10T15:21:00Z',
            finishedAt: null,
            updatedAt: '2026-04-10T15:22:00Z',
            heartbeatAt: null,
            progressLabel: null,
            progressDetail: null,
            progressCurrent: null,
            progressTotal: null,
            progressPercent: null,
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

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot,
    })

    expect(
      await screen.findByText(jobsT('needsReviewBacklog', { count: 2 })),
    ).toBeVisible()
    expect(
      screen.getAllByText(jobsT('errorUnsupportedContent')).length,
    ).toBeGreaterThan(0)
  })

  test('reads jobs runtime truth from the shell source instead of page-local polling', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')
    const loadAiQueueStatusSpy = vi.spyOn(backend, 'loadAiQueueStatus')
    const loadRuntimeSpy = vi.spyOn(backend, 'loadIntelligenceRuntime')
    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue: {
        paused: false,
        concurrency: 1,
        queued: 2,
        running: 1,
        failed: 0,
        recentJobs: [],
      },
      intelligence: {
        queue: {
          queued: 1,
          running: 0,
          succeeded: 0,
          failed: 0,
          cancelled: 0,
          lastActivityAt: '2026-04-10T16:30:00Z',
        },
        plugins: [
          {
            pluginId: 'readable-content-refetch',
            sourceKind: 'network',
            enabled: true,
            storedRecords: 4,
            queuedJobs: 0,
            runningJobs: 0,
            failedJobs: 0,
            lastCompletedAt: '2026-04-10T16:20:00Z',
            lastError: null,
          },
        ],
        modules: [],
        recentJobs: [],
        notes: [],
      },
      loading: false,
      error: null,
    }
    shellValue.refreshRuntimeStatus = vi
      .fn()
      .mockResolvedValue(shellValue.runtimeStatus)

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot,
    })

    expect(screen.getByText(jobsT('runningTitle'))).toBeVisible()
    expect(
      screen.getAllByText(jobsT('contentFetchDeferredBody')).length,
    ).toBeGreaterThan(0)
    expect(screen.queryByText('jobs.contentFetchHealthyBody')).toBeNull()
    expect(loadAiQueueStatusSpy).not.toHaveBeenCalled()
    expect(loadRuntimeSpy).not.toHaveBeenCalled()

    await user.click(screen.getByRole('button', { name: jobsT('refresh') }))

    await waitFor(() =>
      expect(shellValue.refreshRuntimeStatus).toHaveBeenCalledTimes(1),
    )
    expect(loadAiQueueStatusSpy).not.toHaveBeenCalled()
    expect(loadRuntimeSpy).not.toHaveBeenCalled()
  })

  test('renders partial queue/runtime snapshots without hiding fallback counts', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')
    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue: null,
      intelligence: {
        queue: {
          queued: 0,
          running: 0,
          succeeded: 0,
          failed: 0,
          cancelled: 0,
          lastActivityAt: null,
        },
        plugins: [
          {
            pluginId: 'readable-content-refetch',
            sourceKind: 'network',
            enabled: true,
            storedRecords: 9,
            queuedJobs: 0,
            runningJobs: 2,
            failedJobs: 0,
            lastCompletedAt: null,
            lastError: null,
          },
        ],
        modules: [],
        recentJobs: [],
        notes: [],
      },
      loading: false,
      error: 'runtime degraded but usable',
    }

    const runtimeOnly = renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot,
    })

    await screen.findByText('runtime degraded but usable')
    expect(
      screen.getAllByText(jobsT('contentFetchDeferredBody')).length,
    ).toBeGreaterThan(0)
    expect(screen.getByText('runtime degraded but usable')).toBeVisible()
    runtimeOnly.unmount()

    const aiOnlyShellValue = createShellValue(snapshot)
    aiOnlyShellValue.runtimeStatus = {
      aiQueue: queueFixture({
        concurrency: 3,
        queued: 0,
        running: 0,
        failed: 0,
        recentJobs: [
          {
            id: 880,
            jobType: 'assistant',
            state: 'queued',
            priority: 10,
            attempt: 1,
            maxAttempts: 3,
            runId: null,
            summary: null,
            queuedAt: '2026-04-10T15:05:00Z',
            availableAt: '2026-04-10T15:05:00Z',
            startedAt: null,
            finishedAt: null,
            heartbeatAt: null,
            errorCode: null,
            errorMessage: null,
          },
        ],
      }),
      intelligence: null,
      loading: false,
      error: null,
    }

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue: aiOnlyShellValue,
      snapshot,
    })

    await screen.findByText(jobsT('recentAiJobs'))
    expect(
      screen.getAllByText(jobsT('contentFetchDeferredBody')).length,
    ).toBeGreaterThan(0)
    expect(
      screen.getAllByText(jobsT('sidebarIdleDetail')).length,
    ).toBeGreaterThan(0)
  })

  test('keeps the idle Jobs header focused on refresh and settings', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')
    const shellValue = createShellValue(snapshot)
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
          queued: 0,
          running: 0,
          succeeded: 0,
          failed: 0,
          cancelled: 0,
          lastActivityAt: null,
        },
        plugins: [],
        modules: [],
        recentJobs: [],
        notes: [],
      },
      loading: false,
      error: null,
    }

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot,
    })

    expect(await screen.findByText(jobsT('readyTitle'))).toBeVisible()
    expect(
      screen.queryByRole('button', { name: jobsT('pauseQueue') }),
    ).toBeNull()
    expect(
      screen.getByRole('link', { name: jobsT('openSettings') }),
    ).toBeVisible()
  })

  test('routes cancel actions and runtime mutation recovery through the Jobs shell', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')
    const aiQueue = queueFixture({
      queued: 1,
      recentJobs: [
        {
          id: 501,
          jobType: 'assistant',
          state: 'queued',
          priority: 10,
          attempt: 1,
          maxAttempts: 3,
          runId: null,
          summary: 'Queued assistant answer',
          queuedAt: '2026-04-10T15:05:00Z',
          availableAt: '2026-04-10T15:05:00Z',
          startedAt: null,
          finishedAt: null,
          heartbeatAt: null,
          errorCode: null,
          errorMessage: null,
        },
      ],
    })
    const runtime = runtimeFixture({
      recentJobs: [
        runtimeJobFixture({
          id: 601,
          state: 'failed',
          retryable: true,
          cancellable: false,
          lastError: 'stale derived row',
        }),
        runtimeJobFixture({
          id: 602,
          state: 'running',
          retryable: false,
          cancellable: true,
          lastError: null,
        }),
        runtimeJobFixture({
          id: 603,
          state: 'running',
          retryable: false,
          cancellable: true,
          lastError: null,
        }),
        runtimeJobFixture({
          id: 604,
          state: 'running',
          retryable: false,
          cancellable: true,
          lastError: null,
        }),
      ],
    })
    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue,
      intelligence: runtime,
      loading: false,
      error: null,
    }
    shellValue.refreshAppData = vi.fn().mockResolvedValue(undefined)
    shellValue.refreshRuntimeStatus = vi
      .fn()
      .mockResolvedValue(shellValue.runtimeStatus)
    const cancelAiSpy = vi
      .spyOn(backend, 'cancelAiJob')
      .mockResolvedValue({ ...aiQueue.recentJobs[0], state: 'cancelled' })
    const retryRuntimeSpy = vi
      .spyOn(backend, 'retryIntelligenceJob')
      .mockRejectedValueOnce(
        new Error(
          "Intelligence job 601 is in state 'succeeded' and cannot be retried.",
        ),
      )
      .mockRejectedValueOnce(new Error('retry exploded'))
    const cancelRuntimeSpy = vi
      .spyOn(backend, 'cancelIntelligenceJob')
      .mockRejectedValueOnce(
        new Error(
          'Intelligence job 602 cannot be cancelled after it finished.',
        ),
      )
      .mockResolvedValueOnce(runtime)
      .mockRejectedValueOnce(new Error('cancel exploded'))

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot,
    })

    const aiPanel = (await screen.findByText(jobsT('recentAiJobs'))).closest(
      '.panel',
    )
    expect(aiPanel).not.toBeNull()
    if (!(aiPanel instanceof HTMLElement)) {
      throw new Error('expected AI jobs panel')
    }
    await user.click(
      within(aiPanel).getByRole('button', { name: jobsT('cancelJob') }),
    )
    await waitFor(() => expect(cancelAiSpy).toHaveBeenCalledWith(501))

    const runtimePanel = screen
      .getByText(jobsT('recentRuntimeJobs'))
      .closest('.panel')
    expect(runtimePanel).not.toBeNull()
    if (!(runtimePanel instanceof HTMLElement)) {
      throw new Error('expected runtime jobs panel')
    }
    await user.click(
      within(runtimePanel).getByRole('button', { name: jobsT('retryJob') }),
    )
    await waitFor(() => expect(retryRuntimeSpy).toHaveBeenCalledWith(601))
    await waitFor(() =>
      expect(shellValue.refreshRuntimeStatus).toHaveBeenCalled(),
    )

    await user.click(
      within(runtimePanel).getByRole('button', { name: jobsT('retryJob') }),
    )
    await waitFor(() => expect(retryRuntimeSpy).toHaveBeenCalledTimes(2))
    expect(await screen.findByText('retry exploded')).toBeVisible()

    const cancelButtons = within(runtimePanel).getAllByRole('button', {
      name: jobsT('cancelJob'),
    })
    await user.click(cancelButtons[0])
    await waitFor(() => expect(cancelRuntimeSpy).toHaveBeenCalledWith(602))
    await waitFor(() =>
      expect(shellValue.refreshRuntimeStatus).toHaveBeenCalled(),
    )

    await user.click(cancelButtons[1])
    await waitFor(() => expect(cancelRuntimeSpy).toHaveBeenCalledWith(603))

    await user.click(cancelButtons[2])
    await waitFor(() => expect(cancelRuntimeSpy).toHaveBeenCalledWith(604))
    expect(await screen.findByText('cancel exploded')).toBeVisible()
  })

  test('uses the common fallback for non-Error runtime action failures', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')
    const commonT = createNamespaceTranslator('en', 'common')
    const runtime = runtimeFixture({
      recentJobs: [
        runtimeJobFixture({
          id: 701,
          state: 'failed',
          retryable: true,
          cancellable: false,
        }),
        runtimeJobFixture({
          id: 702,
          state: 'running',
          retryable: false,
          cancellable: true,
        }),
      ],
    })
    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue: queueFixture(),
      intelligence: runtime,
      loading: false,
      error: null,
    }
    vi.spyOn(backend, 'retryIntelligenceJob').mockRejectedValueOnce(
      'retry fallback',
    )
    vi.spyOn(backend, 'cancelIntelligenceJob').mockRejectedValueOnce(
      'cancel fallback',
    )

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot,
    })

    const runtimePanel = (
      await screen.findByText(jobsT('recentRuntimeJobs'))
    ).closest('.panel')
    expect(runtimePanel).not.toBeNull()
    if (!(runtimePanel instanceof HTMLElement)) {
      throw new Error('expected runtime jobs panel')
    }
    await user.click(
      within(runtimePanel).getByRole('button', { name: jobsT('retryJob') }),
    )
    expect(await screen.findByText(commonT('notAvailable'))).toBeVisible()

    await user.click(
      within(runtimePanel).getByRole('button', { name: jobsT('cancelJob') }),
    )
    expect(await screen.findByText(commonT('notAvailable'))).toBeVisible()
  })

  test('renders active and stale archive write tasks in Jobs', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')
    snapshot.recentRuns = [
      {
        id: 93,
        startedAt: '2026-04-27T09:00:00.000Z',
        finishedAt: null,
        status: 'running',
        runType: 'import',
        trigger: 'manual',
        profileScope: [],
        manifestHash: null,
        profilesProcessed: 0,
        newVisits: 0,
        newUrls: 0,
        newDownloads: 0,
      },
      {
        id: 94,
        startedAt: '2026-04-27T08:00:00.000Z',
        finishedAt: null,
        status: 'running',
        runType: 'backup',
        trigger: 'manual',
        manifestHash: null,
        profilesProcessed: 0,
        newVisits: 0,
        newUrls: 0,
        newDownloads: 0,
      },
      {
        id: 95,
        startedAt: '2026-04-27T07:00:00.000Z',
        finishedAt: '2026-04-27T07:01:00.000Z',
        status: 'succeeded',
        trigger: 'manual',
        manifestHash: null,
        profilesProcessed: 1,
        newVisits: 1,
        newUrls: 1,
        newDownloads: 0,
      },
    ]
    const shellValue = createShellValue(snapshot)
    shellValue.archiveTasks = [
      archiveTaskFixture({
        id: 'task-backup',
        kind: 'backup',
        title: 'Backup Chrome',
        state: 'queued',
        progressLabel: '2 / 4 records',
        progressValue: 50,
        processedRecords: 2,
        totalRecords: 4,
      }),
    ]

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot,
    })

    expect(screen.getByText(jobsT('archiveTasksTitle'))).toBeVisible()
    expect(screen.getByRole('heading', { name: 'Backup Chrome' })).toBeVisible()
    expect(screen.getAllByText('2 / 4 records')).toHaveLength(2)
    expect(
      screen.getAllByRole('heading', { name: jobsT('archiveTaskStaleTitle') }),
    ).toHaveLength(2)
  })
})

function archiveTaskFixture(overrides: Partial<ShellTask> = {}): ShellTask {
  return {
    id: 'task-import',
    kind: 'import',
    state: 'running',
    title: 'Import Google Takeout',
    detail: 'Writing archive records',
    startedAt: '2026-04-27T10:00:00.000Z',
    updatedAt: '2026-04-27T10:01:00.000Z',
    finishedAt: null,
    progressValue: null,
    logEntries: [],
    ...overrides,
  }
}

function queueFixture(overrides: Partial<AiQueueStatus> = {}): AiQueueStatus {
  return {
    paused: false,
    concurrency: 1,
    queued: 0,
    running: 0,
    failed: 0,
    recentJobs: [],
    ...overrides,
  }
}

function runtimeFixture(
  overrides: Partial<IntelligenceRuntimeSnapshot> = {},
): IntelligenceRuntimeSnapshot {
  return {
    queue: {
      queued: 0,
      running: 0,
      succeeded: 0,
      failed: 0,
      cancelled: 0,
      lastActivityAt: null,
    },
    plugins: [],
    modules: [],
    recentJobs: [],
    notes: [],
    ...overrides,
  }
}

function runtimeJobFixture(
  overrides: Partial<IntelligenceRuntimeSnapshot['recentJobs'][number]> = {},
): IntelligenceRuntimeSnapshot['recentJobs'][number] {
  return {
    id: 601,
    jobType: 'deterministic-rebuild',
    pluginId: null,
    state: 'failed',
    historyId: null,
    profileId: 'chrome:Default',
    url: null,
    title: 'chrome:Default',
    attempt: 1,
    createdAt: '2026-04-10T15:20:00Z',
    startedAt: '2026-04-10T15:21:00Z',
    finishedAt: null,
    updatedAt: '2026-04-10T15:22:00Z',
    heartbeatAt: null,
    progressLabel: null,
    progressDetail: null,
    progressCurrent: null,
    progressTotal: null,
    progressPercent: null,
    lastError: null,
    retryable: true,
    cancellable: false,
    ...overrides,
  }
}
