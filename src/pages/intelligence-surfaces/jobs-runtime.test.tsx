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
    expect(screen.getByText('Derived-data queue')).toBeVisible()
    expect(screen.getByText(jobsT('runtimeHealthTitle'))).toBeVisible()
    expect(screen.getByText(jobsT('pluginsTitle'))).toBeVisible()
    expect(screen.getAllByText('Page content fetcher').length).toBeGreaterThan(
      0,
    )
    expect(screen.getByText('Sessions')).toBeVisible()
    expect(
      screen.getByText('New imports were added after the last rebuild.'),
    ).toBeVisible()
    expect(screen.getByText('Scoring visits')).toBeVisible()
    expect(
      screen.getAllByText('24,000 / 64,781 visits').length,
    ).toBeGreaterThan(0)
    expect(screen.getByText('47%')).toBeVisible()

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
        plugins: [],
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
    expect(loadAiQueueStatusSpy).not.toHaveBeenCalled()
    expect(loadRuntimeSpy).not.toHaveBeenCalled()

    await user.click(screen.getByRole('button', { name: jobsT('refresh') }))

    await waitFor(() =>
      expect(shellValue.refreshRuntimeStatus).toHaveBeenCalledTimes(1),
    )
    expect(loadAiQueueStatusSpy).not.toHaveBeenCalled()
    expect(loadRuntimeSpy).not.toHaveBeenCalled()
  })
})
