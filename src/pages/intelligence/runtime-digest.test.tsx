/**
 * This test file protects the compact Intelligence runtime digest.
 *
 * Why this file exists:
 * - `/intelligence` depends on this digest for queue honesty without loading the full Jobs page.
 * - Archive gating, runtime errors, and queued background work each need distinct visible copy.
 *
 * Main declarations:
 * - This file is mostly internal implementation detail.
 *
 * Source-of-truth notes:
 * - Keep assertions centered on user-visible runtime truth and the Jobs escape hatch.
 */

import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test } from 'vitest'
import {
  ShellDataContext,
  type ShellDataContextValue,
  type ShellRuntimeStatus,
} from '../../app/shell-data-context'
import { I18nProvider } from '../../lib/i18n'
import { IntelligenceRuntimeDigest } from './runtime-digest'

const runtimeStatus = (
  patch: Partial<ShellRuntimeStatus> = {},
): ShellRuntimeStatus => ({
  aiQueue: {
    paused: false,
    concurrency: 2,
    queued: 0,
    running: 0,
    failed: 0,
    recentJobs: [],
  },
  intelligence: {
    queue: {
      queued: 0,
      running: 0,
      failed: 0,
      succeeded: 0,
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
  ...patch,
})

function renderDigest({
  initialized = true,
  runtime = runtimeStatus(),
  unlocked = true,
}: {
  initialized?: boolean
  runtime?: ShellRuntimeStatus
  unlocked?: boolean
} = {}) {
  return render(
    <I18nProvider>
      <MemoryRouter>
        <ShellDataContext.Provider
          value={{ runtimeStatus: runtime } as ShellDataContextValue}
        >
          <IntelligenceRuntimeDigest
            initialized={initialized}
            unlocked={unlocked}
          />
        </ShellDataContext.Provider>
      </MemoryRouter>
    </I18nProvider>,
  )
}

describe('IntelligenceRuntimeDigest', () => {
  test('shows archive gating before queue truth is available', () => {
    renderDigest({ initialized: false, unlocked: false })

    expect(screen.getByText('Archive setup required')).toBeVisible()
    expect(
      screen.getByText(
        'Finish setup and unlock the archive before PathKeep can review Core Intelligence work.',
      ),
    ).toBeVisible()
  })

  test('shows runtime transport errors directly', () => {
    renderDigest({
      runtime: runtimeStatus({ error: 'runtime bridge unavailable' }),
    })

    expect(screen.getByText('Runtime review unavailable')).toBeVisible()
    expect(screen.getByText('runtime bridge unavailable')).toBeVisible()
  })

  test('stays silent for healthy queued background work (covered by sidebar + /jobs)', () => {
    renderDigest({
      runtime: runtimeStatus({
        intelligence: {
          ...runtimeStatus().intelligence!,
          queue: {
            ...runtimeStatus().intelligence!.queue,
            queued: 2,
          },
        },
      }),
    })

    // v0.3 redesign hides the digest on healthy info-tone states; the sidebar
    // background-status strip and /jobs route already cover queued work.
    expect(
      screen.queryByTestId('intelligence-runtime-digest'),
    ).not.toBeInTheDocument()
  })

  test('summarizes failed runtime jobs with last activity details', () => {
    renderDigest({
      runtime: runtimeStatus({
        aiQueue: null,
        intelligence: {
          ...runtimeStatus().intelligence!,
          queue: {
            ...runtimeStatus().intelligence!.queue,
            failed: 2,
            lastActivityAt: '2026-04-25T10:15:00.000Z',
          },
          recentJobs: [
            jobFixture({
              lastError: 'provider refused the request',
              state: 'failed',
            }),
          ],
        },
      }),
    })

    expect(screen.getByText('2 jobs need review')).toBeVisible()
    expect(screen.getByText('provider refused the request')).toBeVisible()
    expect(screen.getByText(/Last activity/)).toBeVisible()
  })

  test('shows the digest only on the warning tone (failed jobs)', () => {
    const failed = renderDigest({
      runtime: runtimeStatus({
        intelligence: {
          ...runtimeStatus().intelligence!,
          queue: {
            ...runtimeStatus().intelligence!.queue,
            failed: 1,
          },
          recentJobs: [jobFixture({ lastError: null, state: 'failed' })],
        },
      }),
    })

    // Failed runtime jobs are an actionable warning — digest renders.
    expect(screen.getByText('1 jobs need review')).toBeVisible()
    failed.unmount()

    // Pure running state with no failures is healthy info — digest stays
    // silent and the sidebar background-status strip carries the signal.
    const running = renderDigest({
      runtime: runtimeStatus({
        intelligence: {
          ...runtimeStatus().intelligence!,
          queue: {
            ...runtimeStatus().intelligence!.queue,
            running: 1,
          },
          recentJobs: [
            jobFixture({
              progressDetail: '12,000 / 20,000 visits',
              state: 'running',
            }),
          ],
        },
      }),
    })

    expect(
      screen.queryByTestId('intelligence-runtime-digest'),
    ).not.toBeInTheDocument()
    running.unmount()

    renderDigest({
      runtime: runtimeStatus({
        intelligence: {
          ...runtimeStatus().intelligence!,
          queue: {
            ...runtimeStatus().intelligence!.queue,
            queued: 1,
          },
          recentJobs: [jobFixture({ state: 'queued', title: null })],
        },
      }),
    })

    expect(
      screen.queryByTestId('intelligence-runtime-digest'),
    ).not.toBeInTheDocument()
  })

  test('stays silent for healthy running and ready runtime states', () => {
    const { rerender } = renderDigest({
      runtime: runtimeStatus({
        intelligence: null,
        loading: true,
        aiQueue: {
          ...runtimeStatus().aiQueue!,
          running: 1,
        },
      }),
    })

    expect(
      screen.queryByTestId('intelligence-runtime-digest'),
    ).not.toBeInTheDocument()

    rerender(
      <I18nProvider>
        <MemoryRouter>
          <ShellDataContext.Provider
            value={
              {
                runtimeStatus: runtimeStatus({
                  intelligence: {
                    ...runtimeStatus().intelligence!,
                    recentJobs: [
                      jobFixture({
                        state: 'succeeded',
                        title: null,
                        updatedAt: '2026-04-25T10:30:00.000Z',
                      }),
                    ],
                  },
                }),
              } as ShellDataContextValue
            }
          >
            <IntelligenceRuntimeDigest initialized={true} unlocked={true} />
          </ShellDataContext.Provider>
        </MemoryRouter>
      </I18nProvider>,
    )

    // Ready / success tone: the v0.3 design surfaces nothing.
    expect(
      screen.queryByTestId('intelligence-runtime-digest'),
    ).not.toBeInTheDocument()
  })
})

function jobFixture(
  overrides: Partial<
    NonNullable<ShellRuntimeStatus['intelligence']>['recentJobs'][number]
  > = {},
): NonNullable<ShellRuntimeStatus['intelligence']>['recentJobs'][number] {
  return {
    id: 5,
    jobType: 'deterministic-rebuild',
    pluginId: null,
    state: 'queued',
    historyId: null,
    profileId: null,
    url: null,
    title: 'Rebuild local intelligence',
    attempt: 1,
    createdAt: '2026-04-25T10:00:00.000Z',
    startedAt: null,
    finishedAt: null,
    updatedAt: '2026-04-25T10:00:00.000Z',
    heartbeatAt: null,
    progressLabel: null,
    progressDetail: null,
    progressCurrent: null,
    progressTotal: null,
    progressPercent: null,
    lastError: null,
    retryable: false,
    cancellable: false,
    ...overrides,
  }
}
