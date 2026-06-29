/**
 * @file index.test.tsx
 * @description Page-level integration tests for the Activity center (JobsPage).
 * @module pages/jobs
 *
 * ## Responsibilities
 * - Verify all gate states render correctly (loading, setup, locked, runtime-loading).
 * - Verify needs-attention, running-now, and background-features zones render correctly.
 * - Verify recent zone toggle behavior.
 * - Verify accessibility contract (aria roles, labelledby).
 *
 * ## Not responsible for
 * - Unit-testing adapter logic (see activity-adapter.test.ts).
 * - Testing gate gates handled by jobs-runtime.test.tsx (kept for compatibility).
 */

import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import type * as ModelDownloadModule from '../../lib/ipc/model-download'
import { useModelDownloadProgress } from '../../lib/ipc/model-download'
import { createNamespaceTranslator } from '../../lib/i18n'
import { I18nContext } from '../../lib/i18n/context'
import { ProfileScopeProvider } from '../../lib/profile-scope'
import { ShellDataContext } from '../../app/shell-data-context'
import type { ShellDataContextValue } from '../../app/shell-data-context'
import { JobsPage } from '.'
import {
  createI18nValue,
  createShellValue,
  renderSurface,
  resetIntelligenceSurfaceHarness,
  seedArchiveState,
} from '../intelligence-surfaces/test-helpers'

// Mock useModelDownloadProgress so model-download-driven rendering can be exercised without
// live Tauri events. The real function is used by default (importOriginal passthrough); individual
// tests opt-in to a custom return value via vi.mocked(...).mockReturnValueOnce(…).
vi.mock('../../lib/ipc/model-download', async (importOriginal) => {
  const actual = await importOriginal<typeof ModelDownloadModule>()
  return {
    ...actual,
    useModelDownloadProgress: vi.fn(actual.useModelDownloadProgress),
  }
})

describe('Activity center (JobsPage)', () => {
  beforeEach(() => {
    resetIntelligenceSurfaceHarness()
  })

  // ── 1. Loading skeleton ────────────────────────────────────────────────────

  test('loading skeleton renders data-testid="activity-page-skeleton"', async () => {
    const { snapshot } = await seedArchiveState()

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue: {
        ...createShellValue(snapshot),
        snapshot: null,
        loading: true,
      },
      snapshot,
    })

    expect(screen.getByTestId('activity-page-skeleton')).toBeInTheDocument()
  })

  // ── 2. Not-initialized gate ────────────────────────────────────────────────

  test('not-initialized renders EmptyState with setupTitle visible', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const uninitializedSnapshot = structuredClone(snapshot)
    uninitializedSnapshot.config.initialized = false

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      snapshot: uninitializedSnapshot,
    })

    expect(screen.getByText(jobsT('setupTitle'))).toBeVisible()
  })

  // ── 3. Archive-locked gate ─────────────────────────────────────────────────

  test('archive-locked renders PermissionGate with lockedTitle visible', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const lockedSnapshot = structuredClone(snapshot)
    lockedSnapshot.archiveStatus.unlocked = false

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      snapshot: lockedSnapshot,
    })

    expect(screen.getByText(jobsT('lockedTitle'))).toBeVisible()
  })

  // ── 4. All-caught-up state ─────────────────────────────────────────────────

  test('all-caught-up: no needs-attention region, no running region, background features render', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const shellValue = createShellValue(snapshot)

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot,
    })

    expect(
      screen.queryByRole('region', { name: jobsT('needsAttentionTitle') }),
    ).toBeNull()
    expect(
      screen.queryByRole('region', { name: jobsT('runningNowTitle') }),
    ).toBeNull()
    expect(
      screen.getByRole('region', { name: jobsT('backgroundFeaturesTitle') }),
    ).toBeInTheDocument()
  })

  // ── 5. Needs-attention zone for failed AI job ──────────────────────────────

  test('needs-attention zone renders for failed AI job with correct task name and Retry button', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const replaySpy = vi
      .spyOn(backend, 'replayAiJob')
      .mockResolvedValue(
        {} as ReturnType<typeof backend.replayAiJob> extends Promise<infer T>
          ? T
          : never,
      )

    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue: {
        paused: false,
        concurrency: 1,
        queued: 0,
        running: 0,
        failed: 1,
        indexQueued: 0,
        indexRunning: 0,
        recentJobs: [
          {
            id: 77,
            jobType: 'index-build',
            state: 'failed',
            priority: 10,
            attempt: 2,
            maxAttempts: 3,
            runId: null,
            summary: null,
            queuedAt: '2026-04-07T18:00:00Z',
            availableAt: '2026-04-07T18:00:00Z',
            startedAt: '2026-04-07T18:01:00Z',
            finishedAt: '2026-04-07T18:02:00Z',
            heartbeatAt: null,
            errorCode: 'rate-limited',
            errorMessage: '429',
          },
        ],
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

    const region = screen.getByRole('region', {
      name: jobsT('needsAttentionTitle'),
    })
    expect(region).toBeInTheDocument()
    expect(screen.getByText(jobsT('taskIndexBuild'))).toBeVisible()

    const retryBtn = screen.getByRole('button', {
      name: new RegExp(jobsT('actionRetry')),
    })
    await user.click(retryBtn)
    await waitFor(() => expect(replaySpy).toHaveBeenCalledWith(77))
  })

  // ── 6. Needs-attention for stale archive run ───────────────────────────────

  test('needs-attention zone renders for stale archive run with "Open Import" link', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const snapshotWithStale = structuredClone(snapshot)
    snapshotWithStale.recentRuns = [
      {
        id: 99,
        startedAt: '2026-04-07T08:00:00Z',
        finishedAt: null,
        status: 'running',
        runType: 'import',
        profilesProcessed: 0,
        newVisits: 0,
        newUrls: 0,
        newDownloads: 0,
      },
    ]

    const shellValue = createShellValue(snapshotWithStale)

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot: snapshotWithStale,
    })

    expect(
      screen.getByRole('region', { name: jobsT('needsAttentionTitle') }),
    ).toBeInTheDocument()
    const openImportLink = screen.getByRole('link', {
      name: new RegExp(jobsT('actionOpenImport')),
    })
    expect(openImportLink).toHaveAttribute('href', '/import')
  })

  // ── 7. Running-now zone with progress bar ─────────────────────────────────

  test('running-now zone renders with progress bar for index-build (aria-valuenow set)', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue: {
        paused: false,
        concurrency: 1,
        queued: 0,
        running: 1,
        failed: 0,
        indexQueued: 0,
        indexRunning: 1,
        recentJobs: [
          {
            id: 55,
            jobType: 'index-build',
            state: 'running',
            priority: 10,
            attempt: 1,
            maxAttempts: 3,
            runId: null,
            summary: null,
            queuedAt: '2026-04-07T10:00:00Z',
            availableAt: '2026-04-07T10:00:00Z',
            startedAt: '2026-04-07T10:01:00Z',
            finishedAt: null,
            heartbeatAt: null,
            errorCode: null,
            errorMessage: null,
            progressScanned: 5000,
            progressScanTarget: 10000,
            progressEmbedded: 4800,
          },
        ],
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

    const region = screen.getByRole('region', {
      name: jobsT('runningNowTitle'),
    })
    expect(region).toBeInTheDocument()

    const progressBar = screen.getByRole('progressbar')
    expect(progressBar).toHaveAttribute('aria-valuenow', '0.5')
  })

  // ── 8. Indeterminate progress when progressScanTarget=0 ───────────────────

  test('running-now zone renders indeterminate bar when progressScanTarget=0 (aria-busy=true)', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue: {
        paused: false,
        concurrency: 1,
        queued: 0,
        running: 1,
        failed: 0,
        indexQueued: 0,
        indexRunning: 1,
        recentJobs: [
          {
            id: 56,
            jobType: 'index-build',
            state: 'running',
            priority: 10,
            attempt: 1,
            maxAttempts: 3,
            runId: null,
            summary: null,
            queuedAt: '2026-04-07T10:00:00Z',
            availableAt: '2026-04-07T10:00:00Z',
            startedAt: '2026-04-07T10:01:00Z',
            finishedAt: null,
            heartbeatAt: null,
            errorCode: null,
            errorMessage: null,
            progressScanned: 0,
            progressScanTarget: 0,
            progressEmbedded: 0,
          },
        ],
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

    const region = screen.getByRole('region', {
      name: jobsT('runningNowTitle'),
    })
    expect(region).toBeInTheDocument()

    // Indeterminate: no progressbar role, but aria-busy on the container
    const busyEl = region.querySelector('[aria-busy="true"]')
    expect(busyEl).not.toBeNull()
  })

  // ── 9. Interruption badge for running index-build ─────────────────────────

  test('interruption badge shows for running index-build', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue: {
        paused: false,
        concurrency: 1,
        queued: 0,
        running: 1,
        failed: 0,
        indexQueued: 0,
        indexRunning: 1,
        recentJobs: [
          {
            id: 57,
            jobType: 'index-build',
            state: 'running',
            priority: 10,
            attempt: 1,
            maxAttempts: 3,
            runId: null,
            summary: null,
            queuedAt: '2026-04-07T10:00:00Z',
            availableAt: '2026-04-07T10:00:00Z',
            startedAt: '2026-04-07T10:01:00Z',
            finishedAt: null,
            heartbeatAt: null,
            errorCode: null,
            errorMessage: null,
          },
        ],
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

    expect(screen.getByText(jobsT('badgeSafeToClose'))).toBeVisible()
  })

  // ── 10. Background features chips ─────────────────────────────────────────

  test('background features chips render (3 chips: Smart-search, Site content, Analysis)', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const shellValue = createShellValue(snapshot)

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot,
    })

    expect(screen.getByText(jobsT('chipSmartSearchLabel'))).toBeVisible()
    expect(screen.getByText(jobsT('chipSiteContentLabel'))).toBeVisible()
    expect(screen.getByText(jobsT('chipAnalysisLabel'))).toBeVisible()
  })

  // ── 11. Smart-search chip shows Off when AI disabled ──────────────────────

  test('smart-search chip shows Off (chipSmartSearchOff) when AI disabled', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    // aiStatus is null or disabled by default in seedArchiveState
    const snapshotNoAi = structuredClone(snapshot)
    if (snapshotNoAi.aiStatus) {
      snapshotNoAi.aiStatus.enabled = false
    }

    const shellValue = createShellValue(snapshotNoAi)

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot: snapshotNoAi,
    })

    expect(screen.getByText(jobsT('chipSmartSearchOff'))).toBeVisible()
  })

  // ── 12. Smart-search chip shows indexed count when ready ──────────────────

  test('smart-search chip shows indexed count when ready', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const snapshotWithAi = structuredClone(snapshot)
    // Construct a clean AiIndexStatus — do NOT spread the seeded snapshot.aiStatus because
    // createMockState seeds a queued assistant job, making queuedJobs=1 in the initial state,
    // which would cause the SmartSearch chip to show "building" instead of "ready".
    snapshotWithAi.aiStatus = {
      enabled: true,
      assistantEnabled: false,
      mcpEnabled: false,
      skillEnabled: false,
      state: 'ready',
      ready: true,
      indexedItems: 1234,
      lastIndexedAt: '2026-04-07T10:00:00Z',
      queuePaused: false,
      queueConcurrency: 1,
      queuedJobs: 0,
      runningJobs: 0,
      failedJobs: 0,
      recentJobs: [],
      semanticSidecarBytes: 0,
      semanticMetadataBytes: 0,
      estimatedEmbeddingTokens: 0,
    }

    const shellValue = createShellValue(snapshotWithAi)

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot: snapshotWithAi,
    })

    expect(
      screen.getByText(jobsT('chipSmartSearchIndexed', { count: 1234 })),
    ).toBeVisible()
  })

  // ── 13. Recent zone collapsed by default; toggle shows/hides ──────────────

  test('recent zone collapsed by default; toggle shows/hides items', async () => {
    const user = userEvent.setup()
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
        indexQueued: 0,
        indexRunning: 0,
        recentJobs: [
          {
            id: 60,
            jobType: 'index-build',
            state: 'succeeded',
            priority: 10,
            attempt: 1,
            maxAttempts: 3,
            runId: null,
            summary: null,
            queuedAt: '2026-04-07T09:00:00Z',
            availableAt: '2026-04-07T09:00:00Z',
            startedAt: '2026-04-07T09:01:00Z',
            finishedAt: '2026-04-07T09:30:00Z',
            heartbeatAt: null,
            errorCode: null,
            errorMessage: null,
          },
        ],
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

    // Toggle button should exist
    const toggleBtn = screen.getByRole('button', {
      name: jobsT('showRecentToggle', { count: 1 }),
    })
    expect(toggleBtn).toBeVisible()

    // Task name NOT visible initially (collapsed)
    expect(screen.queryByText(jobsT('taskIndexBuild'))).toBeNull()

    // Expand
    await user.click(toggleBtn)
    expect(await screen.findByText(jobsT('taskIndexBuild'))).toBeVisible()

    // Collapse
    const hideBtn = screen.getByRole('button', {
      name: jobsT('hideRecentToggle'),
    })
    await user.click(hideBtn)
    expect(screen.queryByText(jobsT('taskIndexBuild'))).toBeNull()
  })

  // ── 14. Paused queue callout ───────────────────────────────────────────────

  test('paused queue callout renders when paused+queued>0', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const pausedSnapshot = structuredClone(snapshot)
    pausedSnapshot.config.ai.jobQueuePaused = true

    const shellValue = createShellValue(pausedSnapshot)
    shellValue.runtimeStatus = {
      aiQueue: {
        paused: true,
        concurrency: 1,
        queued: 3,
        running: 0,
        failed: 0,
        indexQueued: 3,
        indexRunning: 0,
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
      snapshot: pausedSnapshot,
    })

    expect(
      screen.getByText(jobsT('pausedQueueCallout', { count: 3 })),
    ).toBeVisible()
    expect(screen.getByText(jobsT('pausedQueueBody'))).toBeVisible()
  })

  // ── 15. Runtime error renders inline StatusCallout ────────────────────────

  test('runtime error renders inline StatusCallout (pageUnavailableTitle visible)', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue: null,
      intelligence: null,
      loading: false,
      error: 'runtime bridge unavailable',
    }

    // Override the runtimeLoading detection: since error is set and aiQueue/runtime are null,
    // we need a non-null aiQueue or runtime so we don't hit the loading gate.
    // The new page shows runtime error inline rather than a full error state.
    shellValue.runtimeStatus = {
      aiQueue: {
        paused: false,
        concurrency: 1,
        queued: 0,
        running: 0,
        failed: 0,
        indexQueued: 0,
        indexRunning: 0,
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
      error: 'runtime bridge unavailable',
    }

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot,
    })

    expect(screen.getByText(jobsT('pageUnavailableTitle'))).toBeVisible()
    expect(screen.getByText('runtime bridge unavailable')).toBeVisible()
  })

  // ── 16. Accessibility contract ─────────────────────────────────────────────

  test('a11y: main has aria-labelledby="activity-page-heading", zones have role="region"', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue: {
        paused: false,
        concurrency: 1,
        queued: 0,
        running: 1,
        failed: 1,
        indexQueued: 0,
        indexRunning: 0,
        recentJobs: [
          {
            id: 70,
            jobType: 'index-build',
            state: 'running',
            priority: 10,
            attempt: 1,
            maxAttempts: 3,
            runId: null,
            summary: null,
            queuedAt: '2026-04-07T10:00:00Z',
            availableAt: '2026-04-07T10:00:00Z',
            startedAt: '2026-04-07T10:01:00Z',
            finishedAt: null,
            heartbeatAt: null,
            errorCode: null,
            errorMessage: null,
          },
          {
            id: 71,
            jobType: 'index-build',
            state: 'failed',
            priority: 10,
            attempt: 2,
            maxAttempts: 3,
            runId: null,
            summary: null,
            queuedAt: '2026-04-07T09:00:00Z',
            availableAt: '2026-04-07T09:00:00Z',
            startedAt: '2026-04-07T09:01:00Z',
            finishedAt: '2026-04-07T09:02:00Z',
            heartbeatAt: null,
            errorCode: null,
            errorMessage: null,
          },
        ],
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

    // main element has aria-labelledby
    const main = screen.getByRole('main')
    expect(main).toHaveAttribute('aria-labelledby', 'activity-page-heading')

    // needs-attention region
    expect(
      screen.getByRole('region', { name: jobsT('needsAttentionTitle') }),
    ).toBeInTheDocument()

    // running region
    expect(
      screen.getByRole('region', { name: jobsT('runningNowTitle') }),
    ).toBeInTheDocument()
  })

  // ── 17. Cancel running AI job ──────────────────────────────────────────────

  test('cancel button on running AI job calls backend.cancelAiJob', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const cancelSpy = vi
      .spyOn(backend, 'cancelAiJob')
      .mockResolvedValue(
        {} as ReturnType<typeof backend.cancelAiJob> extends Promise<infer T>
          ? T
          : never,
      )

    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue: {
        paused: false,
        concurrency: 1,
        queued: 0,
        running: 1,
        failed: 0,
        indexQueued: 0,
        indexRunning: 1,
        recentJobs: [
          {
            id: 88,
            jobType: 'index-build',
            state: 'running',
            priority: 10,
            attempt: 1,
            maxAttempts: 3,
            runId: null,
            summary: null,
            queuedAt: '2026-04-07T10:00:00Z',
            availableAt: '2026-04-07T10:00:00Z',
            startedAt: '2026-04-07T10:01:00Z',
            finishedAt: null,
            heartbeatAt: null,
            errorCode: null,
            errorMessage: null,
          },
        ],
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

    const cancelBtn = screen.getByRole('button', {
      name: `${jobsT('actionCancel')} ${jobsT('taskIndexBuild')}`,
    })
    await user.click(cancelBtn)
    await waitFor(() => expect(cancelSpy).toHaveBeenCalledWith(88))
  })

  // ── 18. Retry runtime job ─────────────────────────────────────────────────

  test('retry button on failed runtime job calls backend.retryIntelligenceJob', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const retrySpy = vi
      .spyOn(backend, 'retryIntelligenceJob')
      .mockResolvedValue(
        {} as ReturnType<typeof backend.retryIntelligenceJob> extends Promise<
          infer T
        >
          ? T
          : never,
      )

    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue: {
        paused: false,
        concurrency: 1,
        queued: 0,
        running: 0,
        failed: 0,
        indexQueued: 0,
        indexRunning: 0,
        recentJobs: [],
      },
      intelligence: {
        queue: {
          queued: 0,
          running: 0,
          succeeded: 0,
          failed: 1,
          cancelled: 0,
          lastActivityAt: null,
        },
        plugins: [],
        modules: [],
        recentJobs: [
          {
            id: 42,
            jobType: 'content-fetch',
            state: 'failed',
            attempt: 1,
            createdAt: '2026-04-07T10:00:00Z',
            startedAt: '2026-04-07T10:01:00Z',
            finishedAt: '2026-04-07T10:02:00Z',
            updatedAt: '2026-04-07T10:02:00Z',
            retryable: true,
            cancellable: false,
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

    const retryBtn = screen.getByRole('button', {
      name: new RegExp(jobsT('actionRetry')),
    })
    await user.click(retryBtn)
    await waitFor(() => expect(retrySpy).toHaveBeenCalledWith(42))
  })

  // ── 18b. Cancel running, cancellable runtime job ──────────────────────────

  test('cancel button on a cancellable running runtime job calls backend.cancelIntelligenceJob', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const cancelSpy = vi
      .spyOn(backend, 'cancelIntelligenceJob')
      .mockResolvedValue(
        {} as ReturnType<typeof backend.cancelIntelligenceJob> extends Promise<
          infer T
        >
          ? T
          : never,
      )

    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue: {
        paused: false,
        concurrency: 1,
        queued: 0,
        running: 0,
        failed: 0,
        indexQueued: 0,
        indexRunning: 0,
        recentJobs: [],
      },
      intelligence: {
        queue: {
          queued: 0,
          running: 1,
          succeeded: 0,
          failed: 0,
          cancelled: 0,
          lastActivityAt: null,
        },
        plugins: [],
        modules: [],
        recentJobs: [
          {
            id: 51,
            jobType: 'content-fetch',
            pluginId: 'readable-content-refetch',
            state: 'running',
            attempt: 1,
            createdAt: '2026-04-07T10:00:00Z',
            startedAt: '2026-04-07T10:01:00Z',
            finishedAt: null,
            updatedAt: '2026-04-07T10:01:30Z',
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

    // The Cancel control carries the task name in its accessible label.
    const cancelBtn = screen.getByRole('button', {
      name: `${jobsT('actionCancel')} ${jobsT('taskContentFetch')}`,
    })
    await user.click(cancelBtn)
    await waitFor(() => expect(cancelSpy).toHaveBeenCalledWith(51))
  })

  // ── 18c. Non-cancellable running runtime job hides the Cancel control ──────

  test('a non-cancellable running runtime job does not render a Cancel control', async () => {
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
        indexQueued: 0,
        indexRunning: 0,
        recentJobs: [],
      },
      intelligence: {
        queue: {
          queued: 0,
          running: 1,
          succeeded: 0,
          failed: 0,
          cancelled: 0,
          lastActivityAt: null,
        },
        plugins: [],
        modules: [],
        recentJobs: [
          {
            id: 52,
            jobType: 'deterministic-rebuild',
            state: 'running',
            attempt: 1,
            createdAt: '2026-04-07T10:00:00Z',
            startedAt: '2026-04-07T10:01:00Z',
            finishedAt: null,
            updatedAt: '2026-04-07T10:01:30Z',
            retryable: false,
            cancellable: false,
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

    // The row is in the running zone, but a non-cancellable job offers no Cancel button.
    expect(
      screen.getByRole('region', { name: jobsT('runningNowTitle') }),
    ).toBeInTheDocument()
    expect(
      screen.queryByRole('button', {
        name: `${jobsT('actionCancel')} ${jobsT('taskDeterministicRebuild')}`,
      }),
    ).toBeNull()
  })

  // ── 18d. Runtime cancel "needs refresh" error silently refreshes ──────────

  test('cancelIntelligenceJob "needs refresh" error silently refreshes', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    vi.spyOn(backend, 'cancelIntelligenceJob').mockRejectedValueOnce(
      new Error('job cannot be cancelled from running state'),
    )

    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue: {
        paused: false,
        concurrency: 1,
        queued: 0,
        running: 0,
        failed: 0,
        indexQueued: 0,
        indexRunning: 0,
        recentJobs: [],
      },
      intelligence: {
        queue: {
          queued: 0,
          running: 1,
          succeeded: 0,
          failed: 0,
          cancelled: 0,
          lastActivityAt: null,
        },
        plugins: [],
        modules: [],
        recentJobs: [
          {
            id: 61,
            jobType: 'content-fetch',
            pluginId: 'readable-content-refetch',
            state: 'running',
            attempt: 1,
            createdAt: '2026-04-07T10:00:00Z',
            startedAt: '2026-04-07T10:01:00Z',
            finishedAt: null,
            updatedAt: '2026-04-07T10:01:30Z',
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

    await user.click(
      screen.getByRole('button', {
        name: `${jobsT('actionCancel')} ${jobsT('taskContentFetch')}`,
      }),
    )
    await waitFor(() =>
      expect(vi.mocked(shellValue.refreshAppData)).toHaveBeenCalled(),
    )
    expect(screen.queryByText(jobsT('pageUnavailableTitle'))).toBeNull()
  })

  // ── 18e. Runtime cancel generic error shows pageUnavailableTitle ──────────

  test('cancelIntelligenceJob generic error shows pageUnavailableTitle', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    vi.spyOn(backend, 'cancelIntelligenceJob').mockRejectedValueOnce(
      new Error('server error 503'),
    )

    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue: {
        paused: false,
        concurrency: 1,
        queued: 0,
        running: 0,
        failed: 0,
        indexQueued: 0,
        indexRunning: 0,
        recentJobs: [],
      },
      intelligence: {
        queue: {
          queued: 0,
          running: 1,
          succeeded: 0,
          failed: 0,
          cancelled: 0,
          lastActivityAt: null,
        },
        plugins: [],
        modules: [],
        recentJobs: [
          {
            id: 62,
            jobType: 'content-fetch',
            pluginId: 'readable-content-refetch',
            state: 'running',
            attempt: 1,
            createdAt: '2026-04-07T10:00:00Z',
            startedAt: '2026-04-07T10:01:00Z',
            finishedAt: null,
            updatedAt: '2026-04-07T10:01:30Z',
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

    await user.click(
      screen.getByRole('button', {
        name: `${jobsT('actionCancel')} ${jobsT('taskContentFetch')}`,
      }),
    )
    await waitFor(() =>
      expect(screen.getByText(jobsT('pageUnavailableTitle'))).toBeVisible(),
    )
  })

  // ── 19. Stale backup action fires backend.runBackupNow ────────────────────

  test('needs-attention zone renders stale backup with Retry backup button that calls runBackupNow', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const runBackupSpy = vi
      .spyOn(backend, 'runBackupNow')
      .mockResolvedValue(
        {} as ReturnType<typeof backend.runBackupNow> extends Promise<infer T>
          ? T
          : never,
      )

    const snapshotWithStaleBackup = structuredClone(snapshot)
    snapshotWithStaleBackup.recentRuns = [
      {
        id: 77,
        startedAt: '2026-04-07T08:00:00Z',
        finishedAt: '2026-04-07T08:30:00Z',
        status: 'running',
        runType: 'backup',
        profilesProcessed: 1,
        newVisits: 50,
        newUrls: 30,
        newDownloads: 0,
      },
    ]

    const shellValue = createShellValue(snapshotWithStaleBackup)

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot: snapshotWithStaleBackup,
    })

    // The stale backup card must render a button (not a link) that fires runBackupNow.
    const retryBtn = screen.getByRole('button', {
      name: new RegExp(jobsT('actionRetryBackup')),
    })
    expect(retryBtn).toBeInTheDocument()
    await user.click(retryBtn)
    await waitFor(() => expect(runBackupSpy).toHaveBeenCalled())
  })

  // ── 20. Pause/resume toggle click exercises handlePauseChange ─────────────

  test('pause toggle button click calls saveConfig and flips queue state', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue: {
        paused: false,
        concurrency: 1,
        queued: 3,
        running: 1,
        failed: 0,
        indexQueued: 3,
        indexRunning: 1,
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

    const saveConfigSpy = vi.fn().mockResolvedValue(undefined)
    shellValue.saveConfig = saveConfigSpy

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot,
    })

    // The "Pause" button appears when running + queued > 0
    const pauseBtn = screen.getByRole('button', { name: jobsT('actionPause') })
    await user.click(pauseBtn)
    await waitFor(() => expect(saveConfigSpy).toHaveBeenCalled())
  })

  // ── 21. SmartSearchChip building state ────────────────────────────────────

  test('smart-search chip shows Building when queuedJobs > 0', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const snapshotBuilding = structuredClone(snapshot)
    snapshotBuilding.aiStatus = {
      enabled: true,
      assistantEnabled: false,
      mcpEnabled: false,
      skillEnabled: false,
      state: 'building',
      ready: false,
      indexedItems: 500,
      lastIndexedAt: null,
      queuePaused: false,
      queueConcurrency: 1,
      queuedJobs: 2,
      runningJobs: 0,
      failedJobs: 0,
      recentJobs: [],
      semanticSidecarBytes: 0,
      semanticMetadataBytes: 0,
      estimatedEmbeddingTokens: 0,
    }

    const shellValue = createShellValue(snapshotBuilding)

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot: snapshotBuilding,
    })

    expect(screen.getByText(jobsT('chipSmartSearchBuilding'))).toBeVisible()
  })

  // ── 22. SmartSearchChip degraded state ────────────────────────────────────

  test('smart-search chip shows degraded state', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const snapshotDegraded = structuredClone(snapshot)
    snapshotDegraded.aiStatus = {
      enabled: true,
      assistantEnabled: false,
      mcpEnabled: false,
      skillEnabled: false,
      state: 'degraded',
      ready: false,
      indexedItems: 900,
      lastIndexedAt: null,
      queuePaused: false,
      queueConcurrency: 1,
      queuedJobs: 0,
      runningJobs: 0,
      failedJobs: 0,
      recentJobs: [],
      semanticSidecarBytes: 0,
      semanticMetadataBytes: 0,
      estimatedEmbeddingTokens: 0,
    }

    const shellValue = createShellValue(snapshotDegraded)

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot: snapshotDegraded,
    })

    expect(screen.getByText(jobsT('chipStateDegraded'))).toBeVisible()
  })

  // ── 23. SmartSearchChip failed state ─────────────────────────────────────

  test('smart-search chip shows failed state', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const snapshotFailed = structuredClone(snapshot)
    snapshotFailed.aiStatus = {
      enabled: true,
      assistantEnabled: false,
      mcpEnabled: false,
      skillEnabled: false,
      state: 'failed',
      ready: false,
      indexedItems: 0,
      lastIndexedAt: null,
      queuePaused: false,
      queueConcurrency: 1,
      queuedJobs: 0,
      runningJobs: 0,
      failedJobs: 1,
      recentJobs: [],
      semanticSidecarBytes: 0,
      semanticMetadataBytes: 0,
      estimatedEmbeddingTokens: 0,
    }

    const shellValue = createShellValue(snapshotFailed)

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot: snapshotFailed,
    })

    expect(screen.getByText(jobsT('chipSmartSearchFailed'))).toBeVisible()
  })

  // ── 24. SmartSearchChip idle (enabled, 0 items) ───────────────────────────

  test('smart-search chip shows idle when enabled but no indexed items', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const snapshotIdle = structuredClone(snapshot)
    snapshotIdle.aiStatus = {
      enabled: true,
      assistantEnabled: false,
      mcpEnabled: false,
      skillEnabled: false,
      state: 'ready',
      ready: true,
      indexedItems: 0,
      lastIndexedAt: null,
      queuePaused: false,
      queueConcurrency: 1,
      queuedJobs: 0,
      runningJobs: 0,
      failedJobs: 0,
      recentJobs: [],
      semanticSidecarBytes: 0,
      semanticMetadataBytes: 0,
      estimatedEmbeddingTokens: 0,
    }

    const shellValue = createShellValue(snapshotIdle)

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot: snapshotIdle,
    })

    expect(screen.getByText(jobsT('chipSmartSearchEmpty'))).toBeVisible()
  })

  // ── 25. AnalysisChip degraded state ─────────────────────────────────────

  test('analysis chip shows degraded when any module is not ready', async () => {
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
        indexQueued: 0,
        indexRunning: 0,
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
        modules: [
          {
            moduleId: 'site-visits',
            enabled: true,
            version: '1.0',
            status: 'stale',
            dependsOn: [],
            derivedTables: [],
            notes: [],
          },
        ],
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

    expect(
      screen.getByText(jobsT('chipAnalysisAttention', { count: 1 })),
    ).toBeVisible()
  })

  // ── 26. SiteContentChip with queued jobs ─────────────────────────────────

  test('site-content chip shows queued state when queuedJobs > 0', async () => {
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
        indexQueued: 0,
        indexRunning: 0,
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
        plugins: [
          {
            pluginId: 'readable-content-refetch',
            sourceKind: 'html',
            enabled: true,
            storedRecords: 500,
            queuedJobs: 5,
            runningJobs: 0,
            failedJobs: 0,
          },
        ],
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

    expect(
      screen.getByText(jobsT('chipSiteContentQueued', { count: 5 })),
    ).toBeVisible()
  })

  // ── 27. Recent zone with different outcome states and resultLink ──────────

  test('recent zone shows failed/cancelled/stale outcomes and resultLink when expanded', async () => {
    const user = userEvent.setup()
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
        indexQueued: 0,
        indexRunning: 0,
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
        recentJobs: [
          {
            id: 201,
            jobType: 'content-fetch',
            state: 'failed',
            attempt: 1,
            createdAt: '2026-04-07T10:00:00Z',
            startedAt: '2026-04-07T10:01:00Z',
            finishedAt: '2026-04-07T10:02:00Z',
            updatedAt: '2026-04-07T10:02:00Z',
            retryable: false,
            cancellable: false,
          },
          {
            id: 202,
            jobType: 'content-fetch',
            state: 'cancelled',
            attempt: 1,
            createdAt: '2026-04-07T09:00:00Z',
            startedAt: '2026-04-07T09:01:00Z',
            finishedAt: '2026-04-07T09:02:00Z',
            updatedAt: '2026-04-07T09:02:00Z',
            retryable: false,
            cancellable: false,
          },
        ],
        notes: [],
      },
      loading: false,
      error: null,
    }
    // Also add a stale archive task with a resultLink so we exercise that path
    shellValue.archiveTasks = [
      {
        id: 'archive-stale-1',
        kind: 'import',
        state: 'stale',
        title: 'Import',
        detail: 'Stale import',
        startedAt: '2026-04-07T08:00:00Z',
        updatedAt: '2026-04-07T08:01:00Z',
        finishedAt: null,
        logEntries: [],
        resultLink: '/audit?run=10',
      },
    ]

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot,
    })

    // Expand recent zone
    const toggleBtn = screen.getByRole('button', {
      name: new RegExp(
        jobsT('showRecentToggle', { count: 3 }).replace(
          /[.*+?^${}()|[\]\\]/g,
          '\\$&',
        ),
      ),
    })
    await user.click(toggleBtn)

    // outcomeKey for failed and cancelled items
    expect(screen.getAllByText(jobsT('outcomeFailed'))[0]).toBeVisible()
    expect(screen.getByText(jobsT('outcomeCancelled'))).toBeVisible()
    expect(screen.getByText(jobsT('outcomeInterrupted'))).toBeVisible()

    // resultLink → "View result →" link
    const viewLink = screen.getByRole('link', { name: 'View result →' })
    expect(viewLink).toHaveAttribute('href', '/audit?run=10')
  })

  // ── 28. headerSummary conditions ─────────────────────────────────────────

  test('headerSummary shows failed+running when both present', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue: {
        paused: false,
        concurrency: 1,
        queued: 0,
        running: 1,
        failed: 2,
        indexQueued: 0,
        indexRunning: 1,
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

    expect(
      screen.getByText(jobsT('headerSummaryFailed', { failed: 2, running: 1 })),
    ).toBeVisible()
  })

  test('headerSummary shows failed-only when only failures present', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue: {
        paused: false,
        concurrency: 1,
        queued: 0,
        running: 0,
        failed: 3,
        indexQueued: 0,
        indexRunning: 0,
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

    expect(
      screen.getByText(jobsT('headerSummaryFailedIdle', { failed: 3 })),
    ).toBeVisible()
  })

  test('headerSummary shows running+waiting when both present', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue: {
        paused: false,
        concurrency: 1,
        queued: 2,
        running: 1,
        failed: 0,
        indexQueued: 2,
        indexRunning: 1,
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

    expect(
      screen.getByText(
        jobsT('headerSummaryRunningWaiting', { running: 1, queued: 2 }),
      ),
    ).toBeVisible()
  })

  test('headerSummary shows running-only when running but no queue', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue: {
        paused: false,
        concurrency: 1,
        queued: 0,
        running: 1,
        failed: 0,
        indexQueued: 0,
        indexRunning: 1,
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

    expect(
      screen.getByText(jobsT('headerSummaryRunning', { running: 1 })),
    ).toBeVisible()
  })

  test('headerSummary shows queued-waiting when queued but nothing running yet (unpaused)', async () => {
    // Integration-review fix: a queued-but-not-running, unpaused job (retry-backoff or a
    // concurrency-limited build) must NOT read "All caught up" while the Running-now zone shows
    // the queued rows.
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue: {
        paused: false,
        concurrency: 1,
        queued: 2,
        running: 0,
        failed: 0,
        indexQueued: 2,
        indexRunning: 0,
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

    expect(
      screen.getByText(jobsT('headerSummaryQueued', { queued: 2 })),
    ).toBeVisible()
    expect(screen.queryByText(jobsT('headerSummaryNoActivity'))).toBeNull()
  })

  test('headerSummary shows all-caught-up with last activity time', async () => {
    const { snapshot } = await seedArchiveState()
    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue: {
        paused: false,
        concurrency: 1,
        queued: 0,
        running: 0,
        failed: 0,
        indexQueued: 0,
        indexRunning: 0,
        recentJobs: [],
      },
      intelligence: {
        queue: {
          queued: 0,
          running: 0,
          succeeded: 0,
          failed: 0,
          cancelled: 0,
          lastActivityAt: '2026-04-07T09:00:00Z',
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

    expect(
      screen.getByText((text) => text.includes('All caught up')),
    ).toBeVisible()
  })

  test('headerSummary shows no-activity when all queues are empty', async () => {
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
        indexQueued: 0,
        indexRunning: 0,
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

    expect(screen.getByText(jobsT('headerSummaryNoActivity'))).toBeVisible()
  })

  // ── 29. staleArchiveRunTask with finishedAt set ───────────────────────────

  test('staleArchiveRunTask uses finishedAt when present', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const snapshotWithFinished = structuredClone(snapshot)
    snapshotWithFinished.recentRuns = [
      {
        id: 55,
        startedAt: '2026-04-07T08:00:00Z',
        finishedAt: '2026-04-07T08:45:00Z',
        status: 'running',
        runType: 'import',
        profilesProcessed: 1,
        newVisits: 100,
        newUrls: 50,
        newDownloads: 0,
      },
    ]

    const shellValue = createShellValue(snapshotWithFinished)

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot: snapshotWithFinished,
    })

    // The stale archive import should be visible in needs-attention
    expect(
      screen.getByRole('region', { name: jobsT('needsAttentionTitle') }),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('link', { name: new RegExp(jobsT('actionOpenImport')) }),
    ).toBeInTheDocument()
  })

  // ── 30. Error paths in mutation handlers ─────────────────────────────────

  describe('mutation handler error paths', () => {
    function makeRuntimeStatus(opts: {
      jobId: number
      state: 'failed' | 'running'
      jobType: string
    }) {
      const isRunning = opts.state === 'running'
      return {
        aiQueue: {
          paused: false,
          concurrency: 1,
          queued: 0,
          running: isRunning ? 1 : 0,
          failed: isRunning ? 0 : 1,
          indexQueued: 0,
          indexRunning: isRunning ? 1 : 0,
          recentJobs: [
            {
              id: opts.jobId,
              jobType: opts.jobType,
              state: opts.state,
              priority: 10,
              attempt: 1,
              maxAttempts: 3,
              runId: null,
              summary: null,
              queuedAt: '2026-04-07T10:00:00Z',
              availableAt: '2026-04-07T10:00:00Z',
              startedAt: '2026-04-07T10:01:00Z',
              finishedAt: null,
              heartbeatAt: null,
              errorCode: null,
              errorMessage: null,
            },
          ],
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
    }

    test('replayAiJob "needs refresh" error silently refreshes (no page error shown)', async () => {
      const user = userEvent.setup()
      const { snapshot } = await seedArchiveState()

      vi.spyOn(backend, 'replayAiJob').mockRejectedValueOnce(
        new Error('can be replayed from a failed state only'),
      )

      const shellValue = createShellValue(snapshot)
      shellValue.runtimeStatus = makeRuntimeStatus({
        jobId: 77,
        state: 'failed',
        jobType: 'index-build',
      })

      renderSurface(<JobsPage />, {
        language: 'en',
        route: '/jobs',
        shellValue,
        snapshot,
      })

      const retryBtn = screen.getByRole('button', {
        name: new RegExp(
          createNamespaceTranslator('en', 'jobs')('actionRetry'),
        ),
      })
      await user.click(retryBtn)
      await waitFor(() =>
        expect(vi.mocked(shellValue.refreshAppData)).toHaveBeenCalled(),
      )

      // "Needs refresh" path: no pageUnavailableTitle shown
      expect(
        screen.queryByText(
          createNamespaceTranslator('en', 'jobs')('pageUnavailableTitle'),
        ),
      ).toBeNull()
    })

    test('replayAiJob generic error shows pageUnavailableTitle', async () => {
      const user = userEvent.setup()
      const { snapshot } = await seedArchiveState()
      const jobsT = createNamespaceTranslator('en', 'jobs')

      vi.spyOn(backend, 'replayAiJob').mockRejectedValueOnce(
        new Error('network timeout'),
      )

      const shellValue = createShellValue(snapshot)
      shellValue.runtimeStatus = makeRuntimeStatus({
        jobId: 78,
        state: 'failed',
        jobType: 'index-build',
      })

      renderSurface(<JobsPage />, {
        language: 'en',
        route: '/jobs',
        shellValue,
        snapshot,
      })

      const retryBtn = screen.getByRole('button', {
        name: new RegExp(jobsT('actionRetry')),
      })
      await user.click(retryBtn)
      await waitFor(() =>
        expect(screen.getByText(jobsT('pageUnavailableTitle'))).toBeVisible(),
      )
    })

    test('cancelAiJob "needs refresh" error silently refreshes', async () => {
      const user = userEvent.setup()
      const { snapshot } = await seedArchiveState()

      vi.spyOn(backend, 'cancelAiJob').mockRejectedValueOnce(
        new Error('job cannot be cancelled from running state'),
      )

      const shellValue = createShellValue(snapshot)
      shellValue.runtimeStatus = makeRuntimeStatus({
        jobId: 88,
        state: 'running',
        jobType: 'index-build',
      })

      renderSurface(<JobsPage />, {
        language: 'en',
        route: '/jobs',
        shellValue,
        snapshot,
      })

      const cancelBtn = screen.getByRole('button', {
        name: `${createNamespaceTranslator('en', 'jobs')('actionCancel')} ${createNamespaceTranslator('en', 'jobs')('taskIndexBuild')}`,
      })
      await user.click(cancelBtn)
      await waitFor(() =>
        expect(vi.mocked(shellValue.refreshAppData)).toHaveBeenCalled(),
      )

      expect(
        screen.queryByText(
          createNamespaceTranslator('en', 'jobs')('pageUnavailableTitle'),
        ),
      ).toBeNull()
    })

    test('cancelAiJob generic error shows pageUnavailableTitle', async () => {
      const user = userEvent.setup()
      const { snapshot } = await seedArchiveState()
      const jobsT = createNamespaceTranslator('en', 'jobs')

      vi.spyOn(backend, 'cancelAiJob').mockRejectedValueOnce(
        new Error('upstream cancel failed'),
      )

      const shellValue = createShellValue(snapshot)
      shellValue.runtimeStatus = makeRuntimeStatus({
        jobId: 89,
        state: 'running',
        jobType: 'index-build',
      })

      renderSurface(<JobsPage />, {
        language: 'en',
        route: '/jobs',
        shellValue,
        snapshot,
      })

      const cancelBtn = screen.getByRole('button', {
        name: `${jobsT('actionCancel')} ${jobsT('taskIndexBuild')}`,
      })
      await user.click(cancelBtn)
      await waitFor(() =>
        expect(screen.getByText(jobsT('pageUnavailableTitle'))).toBeVisible(),
      )
    })

    test('retryIntelligenceJob "needs refresh" error silently refreshes', async () => {
      const user = userEvent.setup()
      const { snapshot } = await seedArchiveState()

      vi.spyOn(backend, 'retryIntelligenceJob').mockRejectedValueOnce(
        new Error('job cannot be retried'),
      )

      const shellValue = createShellValue(snapshot)
      shellValue.runtimeStatus = {
        ...makeRuntimeStatus({
          jobId: 42,
          state: 'failed',
          jobType: 'content-fetch',
        }),
        aiQueue: {
          paused: false,
          concurrency: 1,
          queued: 0,
          running: 0,
          failed: 0,
          indexQueued: 0,
          indexRunning: 0,
          recentJobs: [],
        },
        intelligence: {
          queue: {
            queued: 0,
            running: 0,
            succeeded: 0,
            failed: 1,
            cancelled: 0,
            lastActivityAt: null,
          },
          plugins: [],
          modules: [],
          recentJobs: [
            {
              id: 42,
              jobType: 'content-fetch',
              state: 'failed',
              attempt: 1,
              createdAt: '2026-04-07T10:00:00Z',
              startedAt: '2026-04-07T10:01:00Z',
              finishedAt: '2026-04-07T10:02:00Z',
              updatedAt: '2026-04-07T10:02:00Z',
              retryable: true,
              cancellable: false,
            },
          ],
          notes: [],
        },
      }

      renderSurface(<JobsPage />, {
        language: 'en',
        route: '/jobs',
        shellValue,
        snapshot,
      })

      const retryBtn = screen.getByRole('button', {
        name: new RegExp(
          createNamespaceTranslator('en', 'jobs')('actionRetry'),
        ),
      })
      await user.click(retryBtn)
      await waitFor(() =>
        expect(vi.mocked(shellValue.refreshAppData)).toHaveBeenCalled(),
      )
    })

    test('retryIntelligenceJob generic error shows pageUnavailableTitle', async () => {
      const user = userEvent.setup()
      const { snapshot } = await seedArchiveState()
      const jobsT = createNamespaceTranslator('en', 'jobs')

      vi.spyOn(backend, 'retryIntelligenceJob').mockRejectedValueOnce(
        new Error('server error 503'),
      )

      const shellValue = createShellValue(snapshot)
      shellValue.runtimeStatus = {
        aiQueue: {
          paused: false,
          concurrency: 1,
          queued: 0,
          running: 0,
          failed: 0,
          indexQueued: 0,
          indexRunning: 0,
          recentJobs: [],
        },
        intelligence: {
          queue: {
            queued: 0,
            running: 0,
            succeeded: 0,
            failed: 1,
            cancelled: 0,
            lastActivityAt: null,
          },
          plugins: [],
          modules: [],
          recentJobs: [
            {
              id: 43,
              jobType: 'content-fetch',
              state: 'failed',
              attempt: 1,
              createdAt: '2026-04-07T10:00:00Z',
              startedAt: '2026-04-07T10:01:00Z',
              finishedAt: '2026-04-07T10:02:00Z',
              updatedAt: '2026-04-07T10:02:00Z',
              retryable: true,
              cancellable: false,
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

      const retryBtn = screen.getByRole('button', {
        name: new RegExp(jobsT('actionRetry')),
      })
      await user.click(retryBtn)
      await waitFor(() =>
        expect(screen.getByText(jobsT('pageUnavailableTitle'))).toBeVisible(),
      )
    })
  })

  // ── 30. Model download in RunningNowZone (indeterminate) ──────────────────

  describe('model download in running-now zone', () => {
    afterEach(() => {
      vi.mocked(useModelDownloadProgress).mockReset()
    })

    test('model download indeterminate row renders when phase=downloading with totalBytes=0', async () => {
      const { snapshot } = await seedArchiveState()
      const jobsT = createNamespaceTranslator('en', 'jobs')

      // Override the hook to return the downloading phase with zero bytes (indeterminate).
      vi.mocked(useModelDownloadProgress).mockReturnValueOnce({
        phase: 'downloading',
        downloadedBytes: 0,
        totalBytes: 0,
        currentFile: 'model.safetensors',
        error: null,
      })

      const shellValue = createShellValue(snapshot)
      shellValue.runtimeStatus = {
        aiQueue: {
          paused: false,
          concurrency: 1,
          queued: 0,
          running: 0,
          failed: 0,
          indexQueued: 0,
          indexRunning: 0,
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

      // RunningNowZone shows the model download row
      const region = screen.getByRole('region', {
        name: jobsT('runningNowTitle'),
      })
      expect(region).toBeInTheDocument()
      expect(screen.getByText(jobsT('taskModelDownload'))).toBeVisible()

      // Indeterminate: no progressbar role, but aria-busy on the container
      const busyEl = region.querySelector('[aria-busy="true"]')
      expect(busyEl).not.toBeNull()
    })

    test('model download determinate row shows byte count label (exercises formatBytes MB + GB)', async () => {
      const { snapshot } = await seedArchiveState()
      const jobsT = createNamespaceTranslator('en', 'jobs')

      // downloadedBytes = 1.5 MB (MB branch), totalBytes = 2 GB (GB branch)
      vi.mocked(useModelDownloadProgress).mockReturnValueOnce({
        phase: 'downloading',
        downloadedBytes: 1572864, // 1.5 MB → formatBytes → "1.5 MB"
        totalBytes: 2147483648, // 2 GB → formatBytes → "2.00 GB"
        currentFile: 'model.safetensors',
        error: null,
      })

      const shellValue = createShellValue(snapshot)
      shellValue.runtimeStatus = {
        aiQueue: {
          paused: false,
          concurrency: 1,
          queued: 0,
          running: 0,
          failed: 0,
          indexQueued: 0,
          indexRunning: 0,
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

      // Determinate: progressbar role is rendered with aria-valuenow
      const progressBar = screen.getByRole('progressbar')
      expect(progressBar).toHaveAttribute('aria-valuenow')

      // progressDownloadLabel is rendered with byte values
      expect(
        screen.getByText(
          jobsT('progressDownloadLabel', {
            downloaded: '1.5 MB',
            total: '2.00 GB',
          }),
        ),
      ).toBeVisible()
    })

    test('model download row shows B and KB ranges in formatBytes', async () => {
      const { snapshot } = await seedArchiveState()
      const jobsT = createNamespaceTranslator('en', 'jobs')

      // downloadedBytes = 500 B, totalBytes = 51200 (50 KB)
      vi.mocked(useModelDownloadProgress).mockReturnValueOnce({
        phase: 'downloading',
        downloadedBytes: 500, // < 1024 → "500 B"
        totalBytes: 51200, // 50 KB → "50.0 KB"
        currentFile: null,
        error: null,
      })

      const shellValue = createShellValue(snapshot)
      shellValue.runtimeStatus = {
        aiQueue: {
          paused: false,
          concurrency: 1,
          queued: 0,
          running: 0,
          failed: 0,
          indexQueued: 0,
          indexRunning: 0,
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

      expect(
        screen.getByText(
          jobsT('progressDownloadLabel', {
            downloaded: '500 B',
            total: '50.0 KB',
          }),
        ),
      ).toBeVisible()
    })
  })

  // ── 31. isStaleArchiveRun null runType branch ──────────────────────────────

  test('isStaleArchiveRun ignores recentRuns with null runType', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const snapshotWithNullType = structuredClone(snapshot)
    snapshotWithNullType.recentRuns = [
      {
        id: 99,
        startedAt: '2026-04-07T08:00:00Z',
        finishedAt: null,
        status: 'running',
        // runType intentionally omitted to hit the ?? '' branch
        profilesProcessed: 0,
        newVisits: 0,
        newUrls: 0,
        newDownloads: 0,
      },
    ]

    const shellValue = createShellValue(snapshotWithNullType)

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot: snapshotWithNullType,
    })

    // No needs-attention zone — the run was filtered out because runType is undefined
    expect(
      screen.queryByRole('region', { name: jobsT('needsAttentionTitle') }),
    ).toBeNull()
  })

  // ── 32. staleArchiveRunTask with profileScope ─────────────────────────────

  test('staleArchiveRunTask populates sourceLabel from profileScope', async () => {
    const { snapshot } = await seedArchiveState()

    const snapshotWithScope = structuredClone(snapshot)
    snapshotWithScope.recentRuns = [
      {
        id: 88,
        startedAt: '2026-04-07T08:00:00Z',
        finishedAt: null,
        status: 'running',
        runType: 'backup',
        profileScope: ['chrome', 'safari'],
        profilesProcessed: 1,
        newVisits: 50,
        newUrls: 30,
        newDownloads: 0,
      },
    ]

    const shellValue = createShellValue(snapshotWithScope)

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot: snapshotWithScope,
    })

    // The needs-attention zone renders the stale backup
    const jobsT = createNamespaceTranslator('en', 'jobs')
    expect(
      screen.getByRole('region', { name: jobsT('needsAttentionTitle') }),
    ).toBeInTheDocument()
  })

  // ── 33. Resume queue (handlePauseChange false branch) ─────────────────────

  test('resume button on paused queue calls saveConfig (covers resumeQueue branch)', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    // Queue is already paused
    const snapshotPaused = structuredClone(snapshot)
    snapshotPaused.config.ai.jobQueuePaused = true

    const shellValue = createShellValue(snapshotPaused)
    shellValue.runtimeStatus = {
      aiQueue: {
        paused: true,
        concurrency: 1,
        queued: 3,
        running: 0,
        failed: 0,
        indexQueued: 3,
        indexRunning: 0,
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

    const saveConfigSpy = vi.fn().mockResolvedValue(undefined)
    shellValue.saveConfig = saveConfigSpy

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot: snapshotPaused,
    })

    // Click the Resume button (queuePaused=true → button says "Resume")
    const resumeBtn = screen.getByRole('button', {
      name: jobsT('actionResume'),
    })
    await user.click(resumeBtn)
    await waitFor(() => expect(saveConfigSpy).toHaveBeenCalled())
  })

  // ── 34. SmartSearchChip unknown state with indexedItems > 0 ───────────────

  test('smart-search chip shows idle for unknown state with indexedItems > 0', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const snapshotUnknown = structuredClone(snapshot)
    snapshotUnknown.aiStatus = {
      ...snapshotUnknown.aiStatus,
      enabled: true,
      state: 'stale',
      ready: false,
      indexedItems: 200,
      queuedJobs: 0,
      runningJobs: 0,
    }

    const shellValue = createShellValue(snapshotUnknown)

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot: snapshotUnknown,
    })

    // Hits the else branch (unknown state + indexedItems > 0 → shows chipSmartSearchEmpty)
    expect(screen.getByText(jobsT('chipSmartSearchEmpty'))).toBeVisible()
  })

  // ── 35. SiteContentChip stored count (plugin exists, queuedJobs = 0) ──────

  test('site-content chip shows stored count when plugin has no queued jobs', async () => {
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
        indexQueued: 0,
        indexRunning: 0,
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
        plugins: [
          {
            pluginId: 'readable-content-refetch',
            sourceKind: 'html',
            enabled: true,
            storedRecords: 750,
            queuedJobs: 0,
            runningJobs: 0,
            failedJobs: 0,
          },
        ],
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

    // Plugin exists with queuedJobs=0 → shows stored count
    expect(
      screen.getByText(jobsT('chipSiteContentStored', { count: 750 })),
    ).toBeVisible()
  })

  // ── 36. AnalysisChip with null runtime (covers ?? 0 fallback) ─────────────

  test('analysis chip renders with attentionCount=0 when runtime is null', async () => {
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
        indexQueued: 0,
        indexRunning: 0,
        recentJobs: [],
      },
      intelligence: null,
      loading: false,
      error: null,
    }

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot,
    })

    // runtime=null → attentionCount = 0 → chipAnalysisReady
    expect(screen.getByText(jobsT('chipAnalysisReady'))).toBeVisible()
  })

  // ── 37. NeedsAttentionZone: failed backup shows Retry backup button + cause ─

  test('needs-attention zone shows backup+failed task with Retry backup button and cause text', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const runBackupSpy = vi
      .spyOn(backend, 'runBackupNow')
      .mockResolvedValue(
        {} as ReturnType<typeof backend.runBackupNow> extends Promise<infer T>
          ? T
          : never,
      )

    const shellValue = createShellValue(snapshot)
    shellValue.archiveTasks = [
      {
        id: 'backup-failed-1',
        kind: 'backup',
        state: 'failed',
        title: 'Backup',
        detail: 'Backup failed',
        startedAt: '2026-04-07T08:00:00Z',
        updatedAt: '2026-04-07T08:01:00Z',
        finishedAt: '2026-04-07T08:01:00Z',
        logEntries: [],
        error: 'disk full',
        resultLink: null,
      },
    ]

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot,
    })

    // Backup+failed must now have a Retry backup button that wires to runBackupNow.
    expect(
      screen.getByRole('region', { name: jobsT('needsAttentionTitle') }),
    ).toBeInTheDocument()
    const retryBtn = screen.getByRole('button', {
      name: new RegExp(jobsT('actionRetryBackup')),
    })
    expect(retryBtn).toBeInTheDocument()
    // activity.cause is set from task.error → cause text appears
    expect(screen.getByText('disk full')).toBeVisible()
    await user.click(retryBtn)
    await waitFor(() => expect(runBackupSpy).toHaveBeenCalled())
  })

  // ── 38. H2: index-build running shows progressEmbeddedLabel ───────────────

  test('running index-build shows embedded count label (H2 per-kind label)', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue: {
        paused: false,
        concurrency: 1,
        queued: 0,
        running: 1,
        failed: 0,
        indexQueued: 0,
        indexRunning: 1,
        recentJobs: [
          {
            id: 100,
            jobType: 'index-build',
            state: 'running',
            priority: 10,
            attempt: 1,
            maxAttempts: 3,
            runId: null,
            summary: null,
            queuedAt: '2026-04-07T10:00:00Z',
            availableAt: '2026-04-07T10:00:00Z',
            startedAt: '2026-04-07T10:01:00Z',
            finishedAt: null,
            heartbeatAt: null,
            errorCode: null,
            errorMessage: null,
            progressScanned: 8000,
            progressScanTarget: 10000,
            progressEmbedded: 7500,
          },
        ],
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

    // Must show "{count} pages embedded", NOT "{count} pages embedded pages embedded"
    expect(
      screen.getByText(jobsT('progressEmbeddedLabel', { count: '7500' })),
    ).toBeVisible()
  })

  // ── 39. H2: import running shows progressRecordsLabel ──────────────────────

  test('running import shows records count label (H2 per-kind label)', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const shellValue = createShellValue(snapshot)
    shellValue.archiveTasks = [
      {
        id: 'import-running-1',
        kind: 'import',
        state: 'running',
        title: 'Import',
        detail: 'Running',
        startedAt: '2026-04-07T10:00:00Z',
        updatedAt: '2026-04-07T10:01:00Z',
        finishedAt: null,
        logEntries: [],
        processedRecords: 42,
        totalRecords: 100,
      },
    ]
    shellValue.runtimeStatus = {
      aiQueue: {
        paused: false,
        concurrency: 1,
        queued: 0,
        running: 0,
        failed: 0,
        indexQueued: 0,
        indexRunning: 0,
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

    expect(
      screen.getByText(
        jobsT('progressRecordsLabel', { processed: 42, total: 100 }),
      ),
    ).toBeVisible()
  })

  // ── 40. H2: content-fetch shows verbatim runtime label ──────────────────────

  test('running content-fetch shows verbatim runtime progressLabel (H2 per-kind label)', async () => {
    const { snapshot } = await seedArchiveState()

    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue: {
        paused: false,
        concurrency: 1,
        queued: 0,
        running: 0,
        failed: 0,
        indexQueued: 0,
        indexRunning: 0,
        recentJobs: [],
      },
      intelligence: {
        queue: {
          queued: 0,
          running: 1,
          succeeded: 0,
          failed: 0,
          cancelled: 0,
          lastActivityAt: null,
        },
        plugins: [],
        modules: [],
        recentJobs: [
          {
            id: 201,
            jobType: 'content-fetch',
            pluginId: 'readable-content-refetch',
            state: 'running',
            attempt: 1,
            createdAt: '2026-04-07T10:00:00Z',
            startedAt: '2026-04-07T10:01:00Z',
            finishedAt: null,
            updatedAt: '2026-04-07T10:01:30Z',
            retryable: false,
            cancellable: true,
            progressLabel: '42 / 100 fetched',
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

    // The verbatim runtime label must appear as-is, NOT wrapped in progressEmbeddedLabel.
    expect(screen.getByText('42 / 100 fetched')).toBeVisible()
  })

  // ── 41. M1: download-only state: running count > 0, no "safe to close" ─────

  test('M1: model download running alone shows running count without "safe to close"', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    vi.mocked(useModelDownloadProgress).mockReturnValueOnce({
      phase: 'downloading',
      downloadedBytes: 500000,
      totalBytes: 2000000,
      currentFile: 'model.bin',
      error: null,
    })

    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue: {
        paused: false,
        concurrency: 1,
        queued: 0,
        running: 0,
        failed: 0,
        indexQueued: 0,
        indexRunning: 0,
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

    // Must NOT say "All caught up" when download is active.
    expect(screen.queryByText(jobsT('headerSummaryNoActivity'))).toBeNull()

    // Must say "{N} running" without "safe to close".
    expect(
      screen.getByText(jobsT('headerSummaryRunningNotSafe', { running: 1 })),
    ).toBeVisible()
  })

  // ── 42. M1: download + index-build: running count 2, no "safe to close" ────

  test('M1: download + index-build both running shows combined count without "safe to close"', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    vi.mocked(useModelDownloadProgress).mockReturnValueOnce({
      phase: 'downloading',
      downloadedBytes: 100,
      totalBytes: 1000,
      currentFile: 'model.bin',
      error: null,
    })

    const shellValue = createShellValue(snapshot)
    shellValue.runtimeStatus = {
      aiQueue: {
        paused: false,
        concurrency: 1,
        queued: 0,
        running: 1,
        failed: 0,
        indexQueued: 0,
        indexRunning: 1,
        recentJobs: [
          {
            id: 501,
            jobType: 'index-build',
            state: 'running',
            priority: 10,
            attempt: 1,
            maxAttempts: 3,
            runId: null,
            summary: null,
            queuedAt: '2026-04-07T10:00:00Z',
            availableAt: '2026-04-07T10:00:00Z',
            startedAt: '2026-04-07T10:01:00Z',
            finishedAt: null,
            heartbeatAt: null,
            errorCode: null,
            errorMessage: null,
          },
        ],
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

    // 2 running total (index-build + download), download is not safe to close.
    expect(
      screen.getByText(jobsT('headerSummaryRunningNotSafe', { running: 2 })),
    ).toBeVisible()
  })

  // ── 43a. NeedsAttentionZone: failed import shows Open Import link ─────────

  test('failed import archive task shows Open Import link in attention zone', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    const shellValue = createShellValue(snapshot)
    shellValue.archiveTasks = [
      {
        id: 'import-failed-1',
        kind: 'import',
        state: 'failed',
        title: 'Import',
        detail: 'Import failed',
        startedAt: '2026-04-07T08:00:00Z',
        updatedAt: '2026-04-07T08:01:00Z',
        finishedAt: '2026-04-07T08:01:00Z',
        logEntries: [],
        error: 'permission denied',
        resultLink: null,
      },
    ]

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot,
    })

    // Failed import must show the "Open Import" link (covers the failed branch of the import condition).
    expect(
      screen.getByRole('link', { name: new RegExp(jobsT('actionOpenImport')) }),
    ).toBeInTheDocument()
  })

  // ── 43b. NeedsAttentionZone renderPrimaryAction returns null for unhandled kind/state ──

  test('stale index-build in attention zone renders no primary action (line 166)', async () => {
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
        indexQueued: 0,
        indexRunning: 0,
        recentJobs: [
          {
            // A stale index-build: kind='index-build', state='stale'
            // None of the renderPrimaryAction conditions match → return null
            id: 999,
            jobType: 'index-build',
            state: 'stale',
            priority: 10,
            attempt: 1,
            maxAttempts: 3,
            runId: null,
            summary: null,
            queuedAt: '2026-04-07T09:00:00Z',
            availableAt: '2026-04-07T09:00:00Z',
            startedAt: '2026-04-07T09:01:00Z',
            finishedAt: '2026-04-07T09:05:00Z',
            heartbeatAt: null,
            errorCode: null,
            errorMessage: null,
          },
        ],
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

    // The needs-attention zone shows the stale index-build…
    const zone = screen.getByRole('region', {
      name: jobsT('needsAttentionTitle'),
    })
    expect(zone).toBeInTheDocument()
    // …but no Retry button or action link (renderPrimaryAction → return null)
    expect(
      zone.querySelector(
        'button[class="btn-secondary"], a[class="btn-secondary"]',
      ),
    ).toBeNull()
  })

  // ── 43. handleRetryBackup error path (line 364) ───────────────────────────

  test('runBackupNow rejection shows pageUnavailableTitle (retry error path)', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    vi.spyOn(backend, 'runBackupNow').mockRejectedValueOnce(
      new Error('disk full'),
    )

    const snapshotWithStale = structuredClone(snapshot)
    snapshotWithStale.recentRuns = [
      {
        id: 77,
        startedAt: '2026-04-07T08:00:00Z',
        finishedAt: '2026-04-07T08:30:00Z',
        status: 'running',
        runType: 'backup',
        profilesProcessed: 1,
        newVisits: 50,
        newUrls: 30,
        newDownloads: 0,
      },
    ]

    const shellValue = createShellValue(snapshotWithStale)

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot: snapshotWithStale,
    })

    const retryBtn = screen.getByRole('button', {
      name: new RegExp(jobsT('actionRetryBackup')),
    })
    await user.click(retryBtn)
    await waitFor(() =>
      expect(screen.getByText(jobsT('pageUnavailableTitle'))).toBeVisible(),
    )
  })

  // ── 44 & 45. aria-live announcer via re-render (lines 198, 205-214) ────────

  describe('aria-live announcer re-render paths', () => {
    // Build the full provider tree so tests can call rerender() with a new
    // shellValue and exercise the useEffect branches that require two renders.
    function buildTree(sv: ShellDataContextValue) {
      return (
        <MemoryRouter initialEntries={['/jobs']}>
          <I18nContext.Provider value={createI18nValue('en')}>
            <ProfileScopeProvider>
              <ShellDataContext.Provider value={sv}>
                <JobsPage />
              </ShellDataContext.Provider>
            </ProfileScopeProvider>
          </I18nContext.Provider>
        </MemoryRouter>
      )
    }

    test('announcer fires when running count drops to zero (line 193)', async () => {
      const { snapshot } = await seedArchiveState()
      const jobsT = createNamespaceTranslator('en', 'jobs')

      const baseStatus = {
        aiQueue: {
          paused: false,
          concurrency: 1,
          queued: 0,
          running: 0,
          failed: 0,
          indexQueued: 0,
          indexRunning: 0,
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

      // First render: 1 running index-build (sets prevRunningCount = 1).
      const sv1 = createShellValue(snapshot)
      sv1.runtimeStatus = {
        ...baseStatus,
        aiQueue: {
          ...baseStatus.aiQueue,
          running: 1,
          indexRunning: 1,
          recentJobs: [
            {
              id: 902,
              jobType: 'index-build',
              state: 'running',
              priority: 10,
              attempt: 1,
              maxAttempts: 3,
              runId: null,
              summary: null,
              queuedAt: '2026-04-07T10:00:00Z',
              availableAt: '2026-04-07T10:00:00Z',
              startedAt: '2026-04-07T10:01:00Z',
              finishedAt: null,
              heartbeatAt: null,
              errorCode: null,
              errorMessage: null,
            },
          ],
        },
      }
      const { rerender } = render(buildTree(sv1))

      // Second render: no running tasks → currentRunning = 0 < prevRunningCount = 1 → line 193.
      const sv2 = createShellValue(snapshot)
      sv2.runtimeStatus = { ...baseStatus }
      rerender(buildTree(sv2))

      await waitFor(() =>
        expect(screen.getByTestId('activity-live-announcer').textContent).toBe(
          jobsT('headerSummaryNoActivity'),
        ),
      )
    })

    test('announcer fires when attention count increases (line 198)', async () => {
      const { snapshot } = await seedArchiveState()
      const jobsT = createNamespaceTranslator('en', 'jobs')

      // First render: no attention items.
      const sv1 = createShellValue(snapshot)
      const { rerender } = render(buildTree(sv1))

      // Second render: add a failed backup task so needsAttention grows from 0 → 1.
      const sv2 = createShellValue(snapshot)
      sv2.archiveTasks = [
        {
          id: 'backup-failed-ann',
          kind: 'backup',
          state: 'failed',
          title: 'Backup',
          detail: 'Backup failed',
          startedAt: '2026-04-07T08:00:00Z',
          updatedAt: '2026-04-07T08:01:00Z',
          finishedAt: '2026-04-07T08:01:00Z',
          logEntries: [],
          error: 'disk full',
          resultLink: null,
        },
      ]

      rerender(buildTree(sv2))

      await waitFor(() =>
        expect(screen.getByTestId('activity-live-announcer').textContent).toBe(
          jobsT('headerSummaryFailedIdle', { failed: 1 }),
        ),
      )
    })

    test('announcer fires milestone when running activity crosses 25% (lines 205-214)', async () => {
      const { snapshot } = await seedArchiveState()
      const jobsT = createNamespaceTranslator('en', 'jobs')

      const baseStatus = {
        aiQueue: {
          paused: false,
          concurrency: 1,
          queued: 0,
          running: 0,
          failed: 0,
          indexQueued: 0,
          indexRunning: 0,
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

      // First render: no running activities (effect skips, sets prevRunningCount = 0).
      const sv1 = createShellValue(snapshot)
      sv1.runtimeStatus = { ...baseStatus }
      const { rerender } = render(buildTree(sv1))

      // Second render: running index-build at 50% progress — crosses 25% milestone.
      const sv2 = createShellValue(snapshot)
      sv2.runtimeStatus = {
        ...baseStatus,
        aiQueue: {
          ...baseStatus.aiQueue,
          running: 1,
          indexRunning: 1,
          recentJobs: [
            {
              id: 901,
              jobType: 'index-build',
              state: 'running',
              priority: 10,
              attempt: 1,
              maxAttempts: 3,
              runId: null,
              summary: null,
              queuedAt: '2026-04-07T10:00:00Z',
              availableAt: '2026-04-07T10:00:00Z',
              startedAt: '2026-04-07T10:01:00Z',
              finishedAt: null,
              heartbeatAt: null,
              errorCode: null,
              errorMessage: null,
              progressScanned: 5000,
              progressScanTarget: 10000, // value = 0.50 → crosses 25% and 50%
              progressEmbedded: 4500,
            },
          ],
        },
      }

      rerender(buildTree(sv2))

      // The last milestone hit is 50%; that's the msg that ends up in the announcer.
      await waitFor(() =>
        expect(screen.getByTestId('activity-live-announcer').textContent).toBe(
          `${jobsT('taskIndexBuild')}: 50%`,
        ),
      )
    })
  })
})
