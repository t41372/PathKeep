/**
 * @file job-panels.test.tsx
 * @description Focused render coverage for the Jobs route job-list panels.
 * @module pages/jobs
 *
 * ## Responsibilities
 * - Verify the render-only AI and runtime job panels keep fallback timestamps, empty details, and disabled action states visible.
 * - Exercise bounded progress rendering without re-testing the full Jobs route shell.
 *
 * ## Not responsible for
 * - Queue mutation orchestration; route-level tests own backend retry/cancel wiring.
 *
 * ## Dependencies
 * - Uses only React Testing Library because these panels do not require router or shell providers.
 *
 * ## Performance notes
 * - Fixtures are intentionally tiny and avoid async backend work.
 */

import { render, screen } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import type { IntelligenceJobOverview } from '../../lib/types'
import { aiJobStateLabel } from './job-panel-helpers'
import { JobPanel, RuntimeJobPanel } from './job-panels'

const jobsT = (key: string) => key
const settingsT = (key: string) => key

describe('Jobs job panels', () => {
  test('maps AI queue states to shipped labels and preserves unknown states', () => {
    expect(
      [
        'queued',
        'running',
        'succeeded',
        'failed',
        'cancelled',
        'paused',
        'stale',
        'custom-state',
      ].map((state) => aiJobStateLabel(state, jobsT)),
    ).toEqual([
      'jobStateQueued',
      'jobStateRunning',
      'jobStateSucceeded',
      'jobStateFailed',
      'jobStateCancelled',
      'jobStatePaused',
      'jobStateStale',
      'custom-state',
    ])
  })

  test('renders AI job fallback details and disables actions while a mutation is active', () => {
    render(
      <JobPanel
        action="Retrying"
        emptyLabel="No AI jobs"
        jobs={[
          {
            id: 1,
            jobType: 'assistant',
            state: 'paused',
            priority: 10,
            attempt: 1,
            maxAttempts: 3,
            runId: null,
            summary: null,
            queuedAt: 'not-a-date',
            availableAt: 'not-a-date',
            startedAt: null,
            finishedAt: null,
            heartbeatAt: null,
            errorCode: null,
            errorMessage: null,
          },
        ]}
        jobsT={jobsT}
        language="en"
        noDetailsLabel="No job details"
        onCancel={vi.fn()}
        onRetry={vi.fn()}
        title="Recent AI jobs"
      />,
    )

    expect(screen.getByText('No job details')).toBeVisible()
    expect(screen.getByText(/createdAt: not-a-date/)).toBeVisible()
    expect(screen.getByText(/startedAt: —/)).toBeVisible()
    expect(screen.getByText(/finishedAt: —/)).toBeVisible()
    expect(screen.getByRole('button', { name: 'retryJob' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'cancelJob' })).toBeDisabled()
  })

  test('renders runtime progress fallbacks, raw dates, and empty states', () => {
    const { rerender } = render(
      <RuntimeJobPanel
        action={null}
        emptyLabel="No runtime jobs"
        jobs={[
          runtimeJob({
            id: 2,
            jobType: 'deterministic-rebuild',
            pluginId: null,
            createdAt: 'not-a-date',
            startedAt: null,
            finishedAt: null,
            updatedAt: 'not-a-date',
            heartbeatAt: null,
            progressPercent: 2,
            progressLabel: null,
            progressDetail: '2 / 100 rows',
          }),
        ]}
        jobsT={jobsT}
        language="en"
        onCancel={vi.fn()}
        onRetry={vi.fn()}
        settingsT={settingsT}
        title="Recent runtime jobs"
      />,
    )

    expect(screen.getByText(/deterministic-rebuild/)).toBeVisible()
    expect(screen.getByText('runningCount')).toBeVisible()
    expect(screen.getAllByText('2 / 100 rows').length).toBeGreaterThan(0)
    expect(screen.getByText(/createdAt: not-a-date/)).toBeVisible()
    expect(screen.getByText(/startedAt: —/)).toBeVisible()
    expect(screen.getByText(/finishedAt: —/)).toBeVisible()
    expect(document.querySelector('.jobs-progress__fill')).toHaveStyle({
      width: '4%',
    })

    rerender(
      <RuntimeJobPanel
        action={null}
        emptyLabel="No runtime jobs"
        jobs={[
          runtimeJob({
            jobType: 'custom-runtime-job',
            pluginId: 'unknown-plugin',
          }),
        ]}
        jobsT={jobsT}
        language="en"
        onCancel={vi.fn()}
        onRetry={vi.fn()}
        settingsT={() => ''}
        title="Recent runtime jobs"
      />,
    )
    expect(screen.getByText(/unknown-plugin/)).toBeVisible()

    rerender(
      <RuntimeJobPanel
        action={null}
        emptyLabel="No runtime jobs"
        jobs={[]}
        jobsT={jobsT}
        language="en"
        onCancel={vi.fn()}
        onRetry={vi.fn()}
        settingsT={settingsT}
        title="Recent runtime jobs"
      />,
    )

    expect(screen.getByText('No runtime jobs')).toBeVisible()
  })
})

function runtimeJob(
  overrides: Partial<IntelligenceJobOverview> = {},
): IntelligenceJobOverview {
  return {
    id: 2,
    jobType: 'enrichment-plugin',
    pluginId: 'readable-content-refetch',
    state: 'running',
    historyId: null,
    profileId: 'chrome:Default',
    url: null,
    title: 'Runtime job',
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
    ...overrides,
  }
}
