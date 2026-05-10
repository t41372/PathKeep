/**
 * @file runtime-health-section.test.tsx
 * @description Focused render coverage for Jobs runtime health summary cards.
 * @module pages/jobs
 *
 * ## Responsibilities
 * - Verify runtime health summaries handle missing plugin owners and mixed module timestamps.
 * - Keep the extracted Jobs runtime section covered without mounting the route.
 *
 * ## Not responsible for
 * - Queue pause/retry/cancel mutations.
 * - Backend runtime polling.
 *
 * ## Dependencies
 * - Uses the shared review runtime-boundary cards through the real component.
 *
 * ## Performance notes
 * - Pure render tests avoid the full Jobs route harness for runtime summary edge cases.
 */

import { render, screen } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import type { IntelligenceRuntimeSnapshot } from '../../lib/types'
import { JobsRuntimeHealthSection } from './runtime-health-section'

describe('JobsRuntimeHealthSection', () => {
  test('renders fallback plugin copy and keeps the latest completed module timestamp', () => {
    render(
      <JobsRuntimeHealthSection
        commonT={translate}
        jobsT={translate}
        language="en"
        runtime={runtimeFixture()}
        settingsT={translate}
      />,
    )

    expect(screen.getByText('contentFetchDeferredBody')).toBeVisible()
    expect(screen.getByText('moduleHealthyBody')).toBeVisible()
    expect(screen.getAllByText(/Apr 25, 2026/).length).toBeGreaterThan(0)
    expect(screen.getAllByText('notAvailable').length).toBeGreaterThan(0)
  })

  test('renders queued content runtime, attention modules, notes, and raw invalid timestamps', () => {
    render(
      <JobsRuntimeHealthSection
        commonT={translate}
        jobsT={translate}
        language="en"
        runtime={{
          ...runtimeFixture(),
          notes: ['Rebuild semantic sidecars before export.'],
          plugins: [
            {
              pluginId: 'readable-content-refetch',
              sourceKind: 'network',
              enabled: true,
              storedRecords: 12,
              queuedJobs: 5,
              runningJobs: 0,
              failedJobs: 1,
              lastCompletedAt: 'not-a-date',
              lastError: 'HTTP 429',
            },
            {
              pluginId: 'title-normalization',
              sourceKind: 'local',
              enabled: true,
              storedRecords: 4,
              queuedJobs: 0,
              runningJobs: 2,
              failedJobs: 0,
              lastCompletedAt: null,
              lastError: null,
            },
          ],
          modules: [
            {
              ...runtimeFixture().modules[0],
              lastBuiltAt: 'not-a-module-date',
              staleReason: 'sessions are stale',
              status: 'stale',
            },
          ],
        }}
        settingsT={translate}
      />,
    )

    expect(screen.getByText('contentFetchDeferredBody')).toBeVisible()
    expect(screen.getByText('moduleAttentionBody:{"count":1}')).toBeVisible()
    expect(
      screen.getByText('Rebuild semantic sidecars before export.'),
    ).toBeVisible()
    expect(screen.getByText('not-a-date')).toBeVisible()
    expect(screen.getByText('not-a-module-date')).toBeVisible()
    expect(screen.getByText('sessions are stale')).toBeVisible()
  })

  test('keeps content runtime branches deferred while webpage body fetch is unavailable', () => {
    const queuedView = render(
      <JobsRuntimeHealthSection
        commonT={translate}
        jobsT={translate}
        language="en"
        runtime={{
          ...runtimeFixture(),
          plugins: [
            {
              pluginId: 'readable-content-refetch',
              sourceKind: 'network',
              enabled: true,
              storedRecords: 12,
              queuedJobs: 5,
              runningJobs: 0,
              failedJobs: 0,
              lastCompletedAt: null,
              lastError: null,
            },
          ],
        }}
        settingsT={translate}
      />,
    )

    expect(screen.getByText('contentFetchDeferredBody')).toBeVisible()
    expect(screen.getAllByText(/queuedCount:\s*0/).length).toBeGreaterThan(0)
    expect(screen.getAllByText(/savedReadableContent:\s*0/).length).toBe(1)
    queuedView.unmount()

    const runningView = render(
      <JobsRuntimeHealthSection
        commonT={translate}
        jobsT={translate}
        language="en"
        runtime={{
          ...runtimeFixture(),
          plugins: [
            {
              pluginId: 'readable-content-refetch',
              sourceKind: 'network',
              enabled: true,
              storedRecords: 12,
              queuedJobs: 0,
              runningJobs: 2,
              failedJobs: 0,
              lastCompletedAt: null,
              lastError: null,
            },
          ],
        }}
        settingsT={translate}
      />,
    )

    expect(screen.getByText('contentFetchDeferredBody')).toBeVisible()
    runningView.unmount()

    render(
      <JobsRuntimeHealthSection
        commonT={translate}
        jobsT={translate}
        language="en"
        runtime={{
          ...runtimeFixture(),
          plugins: [
            {
              pluginId: 'readable-content-refetch',
              sourceKind: 'network',
              enabled: true,
              storedRecords: 8,
              queuedJobs: 0,
              runningJobs: 0,
              failedJobs: 0,
              lastCompletedAt: null,
              lastError: null,
            },
          ],
        }}
        settingsT={translate}
      />,
    )

    expect(screen.getByText('contentFetchDeferredBody')).toBeVisible()
  })

  test('renders live content runtime branches when webpage body fetch is release-enabled', async () => {
    vi.resetModules()
    vi.doMock('../../lib/release-capabilities', () => ({
      deferredFeatureReleaseLabel: 'v0.3',
      optionalAiFeaturesAvailable: false,
      readableContentFetchAvailable: true,
    }))

    try {
      const { JobsRuntimeHealthSection: EnabledJobsRuntimeHealthSection } =
        await import('./runtime-health-section')
      const renderWithContentPlugin = (
        plugin: IntelligenceRuntimeSnapshot['plugins'][number] | null,
      ) =>
        render(
          <EnabledJobsRuntimeHealthSection
            commonT={translate}
            jobsT={translate}
            language="en"
            runtime={{
              ...runtimeFixture(),
              plugins: plugin ? [plugin] : [],
            }}
            settingsT={translate}
          />,
        )

      const fallbackView = renderWithContentPlugin(null)
      expect(screen.getByText('contentFetchFallbackBody')).toBeVisible()
      fallbackView.unmount()

      const errorView = renderWithContentPlugin(
        contentPluginFixture({ lastError: 'HTTP 500' }),
      )
      expect(screen.getAllByText('HTTP 500').length).toBeGreaterThan(0)
      errorView.unmount()

      const queuedView = renderWithContentPlugin(
        contentPluginFixture({ queuedJobs: 5, storedRecords: 12 }),
      )
      expect(
        screen.getByText('contentFetchBacklogBody:{"queued":5,"stored":12}'),
      ).toBeVisible()
      queuedView.unmount()

      const runningView = renderWithContentPlugin(
        contentPluginFixture({ runningJobs: 2, storedRecords: 12 }),
      )
      expect(
        screen.getByText('contentFetchRunningBody:{"stored":12}'),
      ).toBeVisible()
      runningView.unmount()

      renderWithContentPlugin(contentPluginFixture({ storedRecords: 8 }))
      expect(
        screen.getByText('contentFetchReadyBody:{"stored":8}'),
      ).toBeVisible()
    } finally {
      vi.doUnmock('../../lib/release-capabilities')
      vi.resetModules()
    }
  })
})

function translate(key: string, vars?: Record<string, string | number>) {
  if (vars) {
    return `${key}:${JSON.stringify(vars)}`
  }
  return key
}

function runtimeFixture(): IntelligenceRuntimeSnapshot {
  return {
    queue: {
      queued: 0,
      running: 0,
      succeeded: 3,
      failed: 0,
      cancelled: 0,
      lastActivityAt: '2026-04-25T11:00:00.000Z',
    },
    plugins: [
      {
        pluginId: 'title-normalization',
        sourceKind: 'local',
        enabled: true,
        storedRecords: 4,
        queuedJobs: 0,
        runningJobs: 0,
        failedJobs: 0,
        lastCompletedAt: null,
        lastError: null,
      },
    ],
    modules: [
      {
        moduleId: 'visit-derived-facts',
        enabled: true,
        version: 'ci-v1',
        status: 'ready',
        dependsOn: [],
        derivedTables: ['visit_derived_facts'],
        lastRunId: null,
        lastBuiltAt: null,
        lastInvalidatedAt: null,
        staleReason: null,
        notes: [],
      },
      {
        moduleId: 'sessions',
        enabled: true,
        version: 'ci-v1',
        status: 'ready',
        dependsOn: ['visit-derived-facts'],
        derivedTables: ['sessions'],
        lastRunId: 12,
        lastBuiltAt: '2026-04-25T12:00:00.000Z',
        lastInvalidatedAt: null,
        staleReason: null,
        notes: [],
      },
      {
        moduleId: 'search-trails',
        enabled: true,
        version: 'ci-v1',
        status: 'ready',
        dependsOn: ['sessions'],
        derivedTables: ['search_trails'],
        lastRunId: 11,
        lastBuiltAt: '2026-04-24T12:00:00.000Z',
        lastInvalidatedAt: null,
        staleReason: null,
        notes: [],
      },
    ],
    recentJobs: [],
    notes: [],
  }
}

function contentPluginFixture(
  overrides: Partial<IntelligenceRuntimeSnapshot['plugins'][number]> = {},
): IntelligenceRuntimeSnapshot['plugins'][number] {
  return {
    pluginId: 'readable-content-refetch',
    sourceKind: 'network',
    enabled: true,
    storedRecords: 0,
    queuedJobs: 0,
    runningJobs: 0,
    failedJobs: 0,
    lastCompletedAt: null,
    lastError: null,
    ...overrides,
  }
}
