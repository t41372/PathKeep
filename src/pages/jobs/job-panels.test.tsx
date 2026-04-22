/**
 * @file job-panels.test.tsx
 * @description Focused coverage for the extracted Jobs recent-activity panels.
 * @module pages/jobs
 *
 * ## Responsibilities
 * - Verify AI queue state labels and empty-state rendering stay truthful.
 * - Verify retry/cancel affordances and runtime progress details render in the extracted panel owners.
 *
 * ## Not responsible for
 * - Re-testing the full Jobs route shell or runtime-health section.
 * - Re-testing shell-owned polling and pause/resume flows.
 *
 * ## Dependencies
 * - Depends on the shipped Jobs and Settings translators.
 * - Uses direct panel rendering to keep the extracted owner boundaries cheap to test.
 *
 * ## Performance notes
 * - Focused render tests avoid mounting the whole Jobs route for local panel coverage.
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, test, vi } from 'vitest'
import { createNamespaceTranslator } from '../../lib/i18n'
import type { AiQueueJob, IntelligenceJobOverview } from '../../lib/types'
import { aiJobStateLabel } from './job-panel-helpers'
import { JobPanel, RuntimeJobPanel } from './job-panels'

const jobsT = createNamespaceTranslator('en', 'jobs')
const settingsT = createNamespaceTranslator('en', 'settings')

describe('jobs job panels', () => {
  test('maps AI queue states to shipped jobs labels and falls back for unknown states', () => {
    expect(aiJobStateLabel('queued', jobsT)).toBe(jobsT('jobStateQueued'))
    expect(aiJobStateLabel('running', jobsT)).toBe(jobsT('jobStateRunning'))
    expect(aiJobStateLabel('succeeded', jobsT)).toBe(jobsT('jobStateSucceeded'))
    expect(aiJobStateLabel('failed', jobsT)).toBe(jobsT('jobStateFailed'))
    expect(aiJobStateLabel('cancelled', jobsT)).toBe(jobsT('jobStateCancelled'))
    expect(aiJobStateLabel('paused', jobsT)).toBe(jobsT('jobStatePaused'))
    expect(aiJobStateLabel('stale', jobsT)).toBe(jobsT('jobStateStale'))
    expect(aiJobStateLabel('mystery', jobsT)).toBe('mystery')
  })

  test('renders empty AI and runtime panels when no jobs are available', () => {
    render(
      <>
        <JobPanel
          action={null}
          emptyLabel={jobsT('recentJobsEmpty')}
          jobs={[]}
          jobsT={jobsT}
          language="en"
          noDetailsLabel={jobsT('noErrorDetails')}
          onCancel={vi.fn()}
          onRetry={vi.fn()}
          title={jobsT('recentAiJobs')}
        />
        <RuntimeJobPanel
          action={null}
          emptyLabel={jobsT('recentJobsEmpty')}
          jobs={[]}
          jobsT={jobsT}
          language="en"
          onCancel={vi.fn()}
          onRetry={vi.fn()}
          settingsT={settingsT}
          title={jobsT('recentRuntimeJobs')}
        />
      </>,
    )

    expect(screen.getAllByText(jobsT('recentJobsEmpty')).length).toBe(2)
  })

  test('renders AI queue retry and cancel actions only for actionable states', async () => {
    const user = userEvent.setup()
    const onRetry = vi.fn().mockResolvedValue(undefined)
    const onCancel = vi.fn().mockResolvedValue(undefined)
    const jobs: AiQueueJob[] = [
      {
        id: 1,
        jobType: 'assistant',
        state: 'failed',
        priority: 10,
        attempt: 1,
        maxAttempts: 3,
        runId: null,
        summary: null,
        queuedAt: '2026-04-10T15:00:00Z',
        availableAt: '2026-04-10T15:00:00Z',
        startedAt: null,
        finishedAt: null,
        heartbeatAt: null,
        errorCode: null,
        errorMessage: null,
      },
      {
        id: 2,
        jobType: 'assistant',
        state: 'queued',
        priority: 10,
        attempt: 1,
        maxAttempts: 3,
        runId: null,
        summary: 'Queued summary',
        queuedAt: '2026-04-10T15:05:00Z',
        availableAt: '2026-04-10T15:05:00Z',
        startedAt: null,
        finishedAt: null,
        heartbeatAt: null,
        errorCode: null,
        errorMessage: null,
      },
    ]

    render(
      <JobPanel
        action={null}
        emptyLabel={jobsT('recentJobsEmpty')}
        jobs={jobs}
        jobsT={jobsT}
        language="en"
        noDetailsLabel={jobsT('noErrorDetails')}
        onCancel={onCancel}
        onRetry={onRetry}
        title={jobsT('recentAiJobs')}
      />,
    )

    expect(screen.getByText(jobsT('noErrorDetails'))).toBeVisible()
    expect(screen.getByText('Queued summary')).toBeVisible()

    const retryButtons = screen.getAllByRole('button', {
      name: jobsT('retryJob'),
    })
    const cancelButtons = screen.getAllByRole('button', {
      name: jobsT('cancelJob'),
    })

    expect(retryButtons).toHaveLength(1)
    expect(cancelButtons).toHaveLength(1)

    await user.click(retryButtons[0])
    await user.click(cancelButtons[0])

    expect(onRetry).toHaveBeenCalledWith(1)
    expect(onCancel).toHaveBeenCalledWith(2)
  })

  test('renders runtime progress, error details, and actionable buttons for runtime jobs', async () => {
    const user = userEvent.setup()
    const onRetry = vi.fn().mockResolvedValue(undefined)
    const onCancel = vi.fn().mockResolvedValue(undefined)
    const jobs: IntelligenceJobOverview[] = [
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
    ]

    render(
      <RuntimeJobPanel
        action={null}
        emptyLabel={jobsT('recentJobsEmpty')}
        jobs={jobs}
        jobsT={jobsT}
        language="en"
        onCancel={onCancel}
        onRetry={onRetry}
        settingsT={settingsT}
        title={jobsT('recentRuntimeJobs')}
      />,
    )

    expect(screen.getByText('47%')).toBeVisible()
    expect(screen.getAllByText('24,000 / 64,781 visits').length).toBe(2)
    expect(screen.getByText('429 from upstream host')).toBeVisible()
    expect(
      screen.getByText((content) => content.includes('Page content fetcher')),
    ).toBeVisible()

    const retryButton = screen.getByRole('button', { name: jobsT('retryJob') })
    const cancelButton = screen.getByRole('button', {
      name: jobsT('cancelJob'),
    })

    await user.click(retryButton)
    await user.click(cancelButton)

    expect(onRetry).toHaveBeenCalledWith(412)
    expect(onCancel).toHaveBeenCalledWith(411)
  })
})
