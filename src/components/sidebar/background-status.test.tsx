/**
 * This test file protects the sidebar's compact background-work status strip.
 *
 * Why this file exists:
 * - Sidebar status copy is a global honesty surface, so each queue state needs direct regression coverage.
 * - Focused tests keep the shell-wide Sidebar suite from carrying every small queue-state permutation.
 *
 * Main declarations:
 * - This file is mostly internal implementation detail.
 *
 * Source-of-truth notes:
 * - Keep assertions centered on visible state, routing, and status tone instead of decorative markup.
 */

import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { afterEach, describe, expect, test, vi } from 'vitest'
import type { ShellRuntimeStatus } from '../../app/shell-data-context'
import type { ShellTask } from '../../app/shell-tasks'
import { I18nProvider } from '../../lib/i18n'
import { SidebarBackgroundStatus } from './background-status'

type RuntimeJob = NonNullable<
  ShellRuntimeStatus['intelligence']
>['recentJobs'][number]

type AiQueueJob = NonNullable<
  NonNullable<ShellRuntimeStatus['aiQueue']>['recentJobs']
>[number]

const idleRuntimeStatus = (): ShellRuntimeStatus => ({
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
})

const runtimeJob = (overrides: Partial<RuntimeJob> = {}): RuntimeJob => ({
  attempt: 1,
  cancellable: true,
  createdAt: '2026-04-25T10:00:00Z',
  id: 1,
  jobType: 'rebuild',
  progressDetail: null,
  progressLabel: null,
  progressPercent: null,
  retryable: false,
  state: 'running',
  updatedAt: '2026-04-25T10:01:00Z',
  ...overrides,
})

const aiQueueJob = (overrides: Partial<AiQueueJob> = {}): AiQueueJob => ({
  id: 1,
  jobType: 'embedding',
  priority: 0,
  state: 'succeeded',
  queuedAt: '2026-04-25T09:00:00Z',
  startedAt: '2026-04-25T09:01:00Z',
  finishedAt: '2026-04-25T09:02:00Z',
  attempt: 1,
  maxAttempts: 1,
  availableAt: '2026-04-25T09:00:00Z',
  ...overrides,
})

function renderStatus(
  runtimeStatus: ShellRuntimeStatus,
  options: {
    activeArchiveTask?: ShellTask | null
    initialized?: boolean
    unlocked?: boolean
  } = {},
) {
  return render(
    <I18nProvider>
      <MemoryRouter>
        <SidebarBackgroundStatus
          activeArchiveTask={options.activeArchiveTask ?? null}
          initialized={options.initialized ?? true}
          runtimeStatus={runtimeStatus}
          unlocked={options.unlocked ?? true}
        />
      </MemoryRouter>
    </I18nProvider>,
  )
}

function statusRoot() {
  const root = screen
    .getByText('Background work')
    .closest('.sidebar-background-status')
  expect(root).not.toBeNull()
  return root as HTMLElement
}

function expectStatus({
  actionHref,
  actionLabel,
  detail,
  indeterminate,
  summary,
  tone,
  width,
}: {
  actionHref: string
  actionLabel: string
  detail: string | RegExp | null
  indeterminate: boolean
  summary: string
  tone: string
  width: string
}) {
  const root = statusRoot()
  expect(root).toHaveAttribute('data-tone', tone)
  expect(screen.getByText(summary)).toBeVisible()
  const detailNode = root.querySelector('.sidebar-background-status__detail')
  if (detail === null) {
    expect(detailNode).toHaveTextContent('')
  } else {
    expect(screen.getByText(detail)).toBeVisible()
  }
  expect(screen.getByRole('link', { name: actionLabel })).toHaveAttribute(
    'href',
    actionHref,
  )
  expect(root.querySelector('.sidebar-background-status__fill')).toHaveStyle({
    width,
  })
  const track = root.querySelector('.sidebar-background-status__track')
  if (indeterminate) {
    expect(track).toHaveClass('sidebar-background-status__track--indeterminate')
  } else {
    expect(track?.className.trim()).toBe('sidebar-background-status__track')
    expect(track).not.toHaveClass(
      'sidebar-background-status__track--indeterminate',
    )
  }
}

describe('SidebarBackgroundStatus', () => {
  afterEach(() => {
    vi.useRealTimers()
  })

  test('routes setup and locked states to the correct recovery surfaces', () => {
    const { rerender } = renderStatus(idleRuntimeStatus(), {
      initialized: false,
      unlocked: false,
    })

    expectStatus({
      actionHref: '/jobs',
      actionLabel: 'Jobs',
      detail: null,
      indeterminate: false,
      summary: 'Background work appears after setup.',
      tone: 'idle',
      width: '0%',
    })

    rerender(
      <I18nProvider>
        <MemoryRouter>
          <SidebarBackgroundStatus
            initialized
            runtimeStatus={idleRuntimeStatus()}
            unlocked={false}
          />
        </MemoryRouter>
      </I18nProvider>,
    )

    expectStatus({
      actionHref: '/security#unlock-archive',
      actionLabel: 'Security',
      detail: 'Open Security before reviewing queued work.',
      indeterminate: false,
      summary: 'Unlock the archive first',
      tone: 'warning',
      width: '100%',
    })
  })

  test('prioritizes active archive writes over idle runtime status', () => {
    renderStatus(idleRuntimeStatus(), {
      activeArchiveTask: {
        id: 'task-import',
        kind: 'import',
        state: 'running',
        title: 'Import Google Takeout',
        detail: 'Writing archive records',
        startedAt: '2026-04-27T10:00:00.000Z',
        updatedAt: '2026-04-27T10:01:00.000Z',
        finishedAt: null,
        progressLabel: '3 / 12 records',
        progressValue: 25,
        logEntries: [],
      },
    })

    expectStatus({
      actionHref: '/jobs',
      actionLabel: 'Jobs',
      detail: '3 / 12 records',
      indeterminate: false,
      summary: 'Import Google Takeout',
      tone: 'running',
      width: '25%',
    })
  })

  test('shows active archive writes without known progress as indeterminate', () => {
    renderStatus(idleRuntimeStatus(), {
      activeArchiveTask: {
        id: 'task-import',
        kind: 'import',
        state: 'running',
        title: 'Import Google Takeout',
        detail: null,
        startedAt: '2026-04-27T10:00:00.000Z',
        updatedAt: '2026-04-27T10:01:00.000Z',
        finishedAt: null,
        progressLabel: null,
        progressValue: null,
        logEntries: [],
      } as unknown as ShellTask,
    })

    expectStatus({
      actionHref: '/jobs',
      actionLabel: 'Jobs',
      detail: 'Open Jobs',
      indeterminate: true,
      summary: 'Import Google Takeout',
      tone: 'running',
      width: '55%',
    })
  })

  test('shows unavailable runtime errors without pretending background work is idle', () => {
    renderStatus({
      ...idleRuntimeStatus(),
      error: 'runtime snapshot failed',
    })

    expectStatus({
      actionHref: '/jobs',
      actionLabel: 'Jobs',
      detail: 'runtime snapshot failed',
      indeterminate: false,
      summary: 'Background work is unavailable',
      tone: 'warning',
      width: '100%',
    })
  })

  test('shows loading, paused, failed, queued, and running fallback states', () => {
    const { rerender } = render(
      <I18nProvider>
        <MemoryRouter>
          <SidebarBackgroundStatus
            initialized
            runtimeStatus={{ ...idleRuntimeStatus(), loading: true }}
            unlocked
          />
        </MemoryRouter>
      </I18nProvider>,
    )

    expectStatus({
      actionHref: '/jobs',
      actionLabel: 'Jobs',
      detail: 'Running',
      indeterminate: true,
      summary: 'Open Jobs',
      tone: 'queued',
      width: '28%',
    })

    rerender(
      <I18nProvider>
        <MemoryRouter>
          <SidebarBackgroundStatus
            initialized
            runtimeStatus={{
              ...idleRuntimeStatus(),
              loading: true,
              intelligence: {
                ...idleRuntimeStatus().intelligence!,
                queue: {
                  ...idleRuntimeStatus().intelligence!.queue,
                  queued: 3,
                },
              },
            }}
            unlocked
          />
        </MemoryRouter>
      </I18nProvider>,
    )
    expectStatus({
      actionHref: '/jobs',
      actionLabel: 'Jobs',
      detail: 'Open Jobs',
      indeterminate: false,
      summary: '3 queued',
      tone: 'queued',
      width: '28%',
    })

    rerender(
      <I18nProvider>
        <MemoryRouter>
          <SidebarBackgroundStatus
            initialized
            runtimeStatus={{
              ...idleRuntimeStatus(),
              loading: true,
              aiQueue: {
                ...idleRuntimeStatus().aiQueue!,
                running: 1,
              },
            }}
            unlocked
          />
        </MemoryRouter>
      </I18nProvider>,
    )
    expectStatus({
      actionHref: '/jobs',
      actionLabel: 'Jobs',
      detail: 'Open Jobs',
      indeterminate: true,
      summary: '1 running · 0 queued',
      tone: 'running',
      width: '55%',
    })

    rerender(
      <I18nProvider>
        <MemoryRouter>
          <SidebarBackgroundStatus
            initialized
            runtimeStatus={{
              ...idleRuntimeStatus(),
              loading: true,
              aiQueue: {
                ...idleRuntimeStatus().aiQueue!,
                failed: 1,
              },
            }}
            unlocked
          />
        </MemoryRouter>
      </I18nProvider>,
    )
    expectStatus({
      actionHref: '/jobs',
      actionLabel: 'Jobs',
      detail: 'Open Jobs',
      indeterminate: false,
      summary: '1 need review',
      tone: 'warning',
      width: '100%',
    })

    rerender(
      <I18nProvider>
        <MemoryRouter>
          <SidebarBackgroundStatus
            initialized
            runtimeStatus={{
              ...idleRuntimeStatus(),
              aiQueue: {
                ...idleRuntimeStatus().aiQueue!,
                paused: true,
                queued: 4,
              },
            }}
            unlocked
          />
        </MemoryRouter>
      </I18nProvider>,
    )
    expectStatus({
      actionHref: '/jobs',
      actionLabel: 'Jobs',
      detail: 'Open Jobs',
      indeterminate: false,
      summary: '4 queued · paused',
      tone: 'paused',
      width: '24%',
    })

    rerender(
      <I18nProvider>
        <MemoryRouter>
          <SidebarBackgroundStatus
            initialized
            runtimeStatus={{
              ...idleRuntimeStatus(),
              aiQueue: {
                ...idleRuntimeStatus().aiQueue!,
                running: 1,
              },
            }}
            unlocked
          />
        </MemoryRouter>
      </I18nProvider>,
    )
    expectStatus({
      actionHref: '/jobs',
      actionLabel: 'Jobs',
      detail: 'Open Jobs',
      indeterminate: true,
      summary: '1 running · 0 queued',
      tone: 'running',
      width: '55%',
    })

    rerender(
      <I18nProvider>
        <MemoryRouter>
          <SidebarBackgroundStatus
            initialized
            runtimeStatus={{
              ...idleRuntimeStatus(),
              aiQueue: {
                ...idleRuntimeStatus().aiQueue!,
                failed: 2,
              },
              intelligence: {
                ...idleRuntimeStatus().intelligence!,
                queue: {
                  ...idleRuntimeStatus().intelligence!.queue,
                  failed: 1,
                },
              },
            }}
            unlocked
          />
        </MemoryRouter>
      </I18nProvider>,
    )
    expectStatus({
      actionHref: '/jobs',
      actionLabel: 'Jobs',
      detail: 'Open Jobs',
      indeterminate: false,
      summary: '3 need review',
      tone: 'warning',
      width: '100%',
    })

    rerender(
      <I18nProvider>
        <MemoryRouter>
          <SidebarBackgroundStatus
            initialized
            runtimeStatus={{
              ...idleRuntimeStatus(),
              intelligence: {
                ...idleRuntimeStatus().intelligence!,
                queue: {
                  ...idleRuntimeStatus().intelligence!.queue,
                  queued: 3,
                },
              },
            }}
            unlocked
          />
        </MemoryRouter>
      </I18nProvider>,
    )
    expectStatus({
      actionHref: '/jobs',
      actionLabel: 'Jobs',
      detail: 'Open Jobs',
      indeterminate: false,
      summary: '3 queued',
      tone: 'queued',
      width: '28%',
    })
  })

  test('uses AI queue totals when the intelligence runtime snapshot is absent', () => {
    renderStatus({
      ...idleRuntimeStatus(),
      aiQueue: {
        ...idleRuntimeStatus().aiQueue!,
        queued: 2,
        running: 0,
        failed: 3,
      },
      intelligence: null,
    })

    expectStatus({
      actionHref: '/jobs',
      actionLabel: 'Jobs',
      detail: 'Open Jobs',
      indeterminate: false,
      summary: '3 need review',
      tone: 'warning',
      width: '100%',
    })
  })

  test('shows runtime progress details, clamped width, and last activity idle copy', () => {
    const { rerender } = render(
      <I18nProvider>
        <MemoryRouter>
          <SidebarBackgroundStatus
            initialized
            runtimeStatus={{
              ...idleRuntimeStatus(),
              intelligence: {
                ...idleRuntimeStatus().intelligence!,
                queue: {
                  ...idleRuntimeStatus().intelligence!.queue,
                  running: 1,
                },
                recentJobs: [
                  {
                    attempt: 1,
                    cancellable: true,
                    createdAt: '2026-04-25T10:00:00Z',
                    id: 1,
                    jobType: 'rebuild',
                    progressDetail: null,
                    progressLabel: 'Indexing visits',
                    progressPercent: 42,
                    retryable: false,
                    state: 'running',
                    updatedAt: '2026-04-25T10:01:00Z',
                  },
                ],
              },
            }}
            unlocked
          />
        </MemoryRouter>
      </I18nProvider>,
    )

    expectStatus({
      actionHref: '/jobs',
      actionLabel: 'Jobs',
      detail: 'Indexing visits',
      indeterminate: false,
      summary: '1 running · 0 queued',
      tone: 'running',
      width: '42%',
    })

    rerender(
      <I18nProvider>
        <MemoryRouter>
          <SidebarBackgroundStatus
            initialized
            runtimeStatus={{
              ...idleRuntimeStatus(),
              intelligence: {
                ...idleRuntimeStatus().intelligence!,
                queue: {
                  ...idleRuntimeStatus().intelligence!.queue,
                  running: 1,
                },
                recentJobs: [
                  {
                    attempt: 1,
                    cancellable: true,
                    createdAt: '2026-04-25T10:00:00Z',
                    id: 2,
                    jobType: 'rebuild',
                    progressDetail: 'Reading domains',
                    progressLabel: 'Indexing visits',
                    progressPercent: 50,
                    retryable: false,
                    state: 'running',
                    updatedAt: '2026-04-25T10:01:00Z',
                  },
                ],
              },
            }}
            unlocked
          />
        </MemoryRouter>
      </I18nProvider>,
    )

    expectStatus({
      actionHref: '/jobs',
      actionLabel: 'Jobs',
      detail: 'Reading domains',
      indeterminate: false,
      summary: '1 running · 0 queued',
      tone: 'running',
      width: '50%',
    })

    rerender(
      <I18nProvider>
        <MemoryRouter>
          <SidebarBackgroundStatus
            initialized
            runtimeStatus={{
              ...idleRuntimeStatus(),
              intelligence: {
                ...idleRuntimeStatus().intelligence!,
                queue: {
                  ...idleRuntimeStatus().intelligence!.queue,
                  running: 1,
                },
                recentJobs: [
                  {
                    attempt: 1,
                    cancellable: true,
                    createdAt: '2026-04-25T10:00:00Z',
                    id: 2,
                    jobType: 'rebuild',
                    progressDetail: null,
                    progressLabel: null,
                    progressPercent: 12,
                    retryable: false,
                    state: 'running',
                    updatedAt: '2026-04-25T10:01:00Z',
                  },
                ],
              },
            }}
            unlocked
          />
        </MemoryRouter>
      </I18nProvider>,
    )

    expectStatus({
      actionHref: '/jobs',
      actionLabel: 'Jobs',
      detail: 'Open Jobs',
      indeterminate: false,
      summary: '1 running · 0 queued',
      tone: 'running',
      width: '12%',
    })

    rerender(
      <I18nProvider>
        <MemoryRouter>
          <SidebarBackgroundStatus
            initialized
            runtimeStatus={{
              ...idleRuntimeStatus(),
              intelligence: {
                ...idleRuntimeStatus().intelligence!,
                queue: {
                  ...idleRuntimeStatus().intelligence!.queue,
                  running: 1,
                },
                recentJobs: [
                  {
                    attempt: 1,
                    cancellable: true,
                    createdAt: '2026-04-25T10:00:00Z',
                    id: 3,
                    jobType: 'rebuild',
                    progressDetail: 'Starting',
                    progressLabel: null,
                    progressPercent: 1,
                    retryable: false,
                    state: 'running',
                    updatedAt: '2026-04-25T10:01:00Z',
                  },
                ],
              },
            }}
            unlocked
          />
        </MemoryRouter>
      </I18nProvider>,
    )

    expectStatus({
      actionHref: '/jobs',
      actionLabel: 'Jobs',
      detail: 'Starting',
      indeterminate: false,
      summary: '1 running · 0 queued',
      tone: 'running',
      width: '8%',
    })

    rerender(
      <I18nProvider>
        <MemoryRouter>
          <SidebarBackgroundStatus
            initialized
            runtimeStatus={{
              ...idleRuntimeStatus(),
              intelligence: {
                ...idleRuntimeStatus().intelligence!,
                queue: {
                  ...idleRuntimeStatus().intelligence!.queue,
                  running: 1,
                },
                recentJobs: [
                  {
                    attempt: 1,
                    cancellable: true,
                    createdAt: '2026-04-25T10:00:00Z',
                    id: 4,
                    jobType: 'rebuild',
                    progressDetail: 'Finishing',
                    progressLabel: null,
                    progressPercent: 180,
                    retryable: false,
                    state: 'running',
                    updatedAt: '2026-04-25T10:01:00Z',
                  },
                ],
              },
            }}
            unlocked
          />
        </MemoryRouter>
      </I18nProvider>,
    )

    expectStatus({
      actionHref: '/jobs',
      actionLabel: 'Jobs',
      detail: 'Finishing',
      indeterminate: false,
      summary: '1 running · 0 queued',
      tone: 'running',
      width: '100%',
    })

    rerender(
      <I18nProvider>
        <MemoryRouter>
          <SidebarBackgroundStatus
            initialized
            runtimeStatus={{
              ...idleRuntimeStatus(),
              intelligence: {
                ...idleRuntimeStatus().intelligence!,
                queue: {
                  ...idleRuntimeStatus().intelligence!.queue,
                  lastActivityAt: '2026-04-25T10:00:00Z',
                },
              },
            }}
            unlocked
          />
        </MemoryRouter>
      </I18nProvider>,
    )

    expectStatus({
      actionHref: '/jobs',
      actionLabel: 'Jobs',
      detail: /Last activity/,
      indeterminate: false,
      summary: 'All caught up',
      tone: 'idle',
      width: '100%',
    })
  })

  test('uses only running jobs with numeric progress for determinate runtime progress', () => {
    const { rerender } = renderStatus({
      ...idleRuntimeStatus(),
      intelligence: {
        ...idleRuntimeStatus().intelligence!,
        queue: {
          ...idleRuntimeStatus().intelligence!.queue,
          running: 1,
        },
        recentJobs: [
          runtimeJob({
            id: 11,
            progressDetail: 'Completed rebuild',
            progressPercent: 91,
            state: 'succeeded',
          }),
          runtimeJob({
            id: 12,
            progressDetail: 'Active rebuild',
            progressPercent: 37,
            state: 'running',
          }),
        ],
      },
    })

    expectStatus({
      actionHref: '/jobs',
      actionLabel: 'Jobs',
      detail: 'Active rebuild',
      indeterminate: false,
      summary: '1 running · 0 queued',
      tone: 'running',
      width: '37%',
    })

    rerender(
      <I18nProvider>
        <MemoryRouter>
          <SidebarBackgroundStatus
            initialized
            runtimeStatus={{
              ...idleRuntimeStatus(),
              intelligence: {
                ...idleRuntimeStatus().intelligence!,
                queue: {
                  ...idleRuntimeStatus().intelligence!.queue,
                  running: 1,
                },
                recentJobs: [
                  runtimeJob({
                    id: 13,
                    progressDetail: 'Waiting for counter',
                    progressPercent: null,
                    state: 'running',
                  }),
                  runtimeJob({
                    id: 14,
                    progressDetail: 'Measured rebuild',
                    progressPercent: 64,
                    state: 'running',
                  }),
                ],
              },
            }}
            unlocked
          />
        </MemoryRouter>
      </I18nProvider>,
    )

    expectStatus({
      actionHref: '/jobs',
      actionLabel: 'Jobs',
      detail: 'Measured rebuild',
      indeterminate: false,
      summary: '1 running · 0 queued',
      tone: 'running',
      width: '64%',
    })
  })

  test('uses AI queue activity fallback and does not treat missing AI queues as paused', () => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-04-27T09:02:00Z'))

    const { rerender } = renderStatus({
      ...idleRuntimeStatus(),
      aiQueue: {
        ...idleRuntimeStatus().aiQueue!,
        recentJobs: [
          aiQueueJob({
            id: 1,
            queuedAt: '2026-04-24T09:02:00Z',
            startedAt: null,
            finishedAt: null,
          }),
          aiQueueJob({
            id: 2,
            queuedAt: '2026-04-27T08:00:00Z',
            startedAt: '2026-04-27T08:02:00Z',
            finishedAt: null,
          }),
          aiQueueJob({
            id: 3,
            queuedAt: '2026-04-25T09:00:00Z',
            startedAt: '2026-04-25T09:01:00Z',
            finishedAt: '2026-04-25T09:02:00Z',
          }),
        ],
      },
    })

    expectStatus({
      actionHref: '/jobs',
      actionLabel: 'Jobs',
      detail: 'Last activity 1 hour ago',
      indeterminate: false,
      summary: 'All caught up',
      tone: 'idle',
      width: '100%',
    })

    rerender(
      <I18nProvider>
        <MemoryRouter>
          <SidebarBackgroundStatus
            initialized
            runtimeStatus={{
              ...idleRuntimeStatus(),
              aiQueue: null,
              intelligence: {
                ...idleRuntimeStatus().intelligence!,
                queue: {
                  ...idleRuntimeStatus().intelligence!.queue,
                  queued: 2,
                },
              },
            }}
            unlocked
          />
        </MemoryRouter>
      </I18nProvider>,
    )

    expectStatus({
      actionHref: '/jobs',
      actionLabel: 'Jobs',
      detail: 'Open Jobs',
      indeterminate: false,
      summary: '2 queued',
      tone: 'queued',
      width: '28%',
    })

    rerender(
      <I18nProvider>
        <MemoryRouter>
          <SidebarBackgroundStatus
            initialized
            runtimeStatus={{
              ...idleRuntimeStatus(),
              aiQueue: {
                ...idleRuntimeStatus().aiQueue!,
                paused: true,
              },
            }}
            unlocked
          />
        </MemoryRouter>
      </I18nProvider>,
    )

    expectStatus({
      actionHref: '/jobs',
      actionLabel: 'Jobs',
      detail: 'No queued background work.',
      indeterminate: false,
      summary: 'All caught up',
      tone: 'idle',
      width: '100%',
    })
  })
})
