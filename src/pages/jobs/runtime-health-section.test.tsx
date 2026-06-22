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
import { MemoryRouter } from 'react-router-dom'
import type { ReactElement } from 'react'
import { describe, expect, test, vi } from 'vitest'
import type { IntelligenceRuntimeSnapshot } from '../../lib/types'
import { JobsRuntimeHealthSection } from './runtime-health-section'

// The honest off-state body renders a `<Link>` to the consent section, so the
// section must be mounted inside a router context (matching the real Jobs
// route, which already lives under the app router).
function renderSection(ui: ReactElement) {
  return render(<MemoryRouter>{ui}</MemoryRouter>)
}

describe('JobsRuntimeHealthSection', () => {
  // ENR-2 / R2: `readableContentFetchAvailable` now ships TRUE, so the default
  // (unmocked) render shows the LIVE content-fetch branch. With no runtime
  // plugin yet (the default unconsented state every fresh user is in) the card
  // must show the honest "off — opt in" body plus a deep-link to the consent
  // section — NEVER the genuinely-deferred placeholder, which only belongs on
  // the flag-false branch exercised separately below.
  test('renders the honest off-state body + consent deep-link in the default unconsented state', () => {
    renderSection(
      <JobsRuntimeHealthSection
        commonT={translate}
        jobsT={translate}
        language="en"
        runtime={runtimeFixture()}
        settingsT={translate}
      />,
    )

    // No readable-content plugin in the fixture → live branch resolves the
    // honest off-state body, not the deferred placeholder ("future release").
    expect(screen.getByText('contentFetchOffBody')).toBeVisible()
    expect(screen.queryByText('contentFetchDeferredBody')).toBeNull()
    // The off-state offers a concrete next step: a deep-link straight to the
    // Settings content-fetch consent section.
    const settingsLink = screen.getByTestId('jobs-content-fetch-settings-link')
    expect(settingsLink).toHaveTextContent('contentFetchOpenSettings')
    expect(settingsLink).toHaveAttribute('href', '/settings#content-fetch')
    expect(screen.getByText('moduleHealthyBody')).toBeVisible()
    expect(screen.getAllByText(/Apr 25, 2026/).length).toBeGreaterThan(0)
    expect(screen.getAllByText('notAvailable').length).toBeGreaterThan(0)
  })

  test('renders live content runtime error, attention modules, notes, and raw invalid timestamps', () => {
    renderSection(
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

    // Live branch surfaces the real plugin error (a 429 → rate-limited copy)
    // instead of the deferred placeholder or the off-state opt-in.
    expect(screen.getAllByText('errorRateLimited').length).toBeGreaterThan(0)
    expect(screen.queryByText('contentFetchDeferredBody')).toBeNull()
    expect(screen.queryByText('contentFetchOffBody')).toBeNull()
    expect(screen.queryByTestId('jobs-content-fetch-settings-link')).toBeNull()
    expect(screen.getByText('moduleAttentionBody:{"count":1}')).toBeVisible()
    expect(
      screen.getByText('Rebuild semantic sidecars before export.'),
    ).toBeVisible()
    expect(screen.getByText('not-a-date')).toBeVisible()
    expect(screen.getByText('not-a-module-date')).toBeVisible()
    expect(screen.getByText('sessions are stale')).toBeVisible()
  })

  test('renders live content runtime queued/running/ready branches with honest stats', () => {
    const queuedView = renderSection(
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

    // Live queued copy plus the REAL stored count (12), never the masked 0.
    expect(
      screen.getByText('contentFetchBacklogBody:{"queued":5,"stored":12}'),
    ).toBeVisible()
    expect(screen.getAllByText(/savedReadableContent:\s*12/).length).toBe(1)
    queuedView.unmount()

    const runningView = renderSection(
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

    expect(
      screen.getByText('contentFetchRunningBody:{"stored":12}'),
    ).toBeVisible()
    runningView.unmount()

    renderSection(
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

    expect(screen.getByText('contentFetchReadyBody:{"stored":8}')).toBeVisible()
  })

  test('keeps the deferred placeholder branch covered when the release flag is off', async () => {
    vi.resetModules()
    vi.doMock('../../lib/release-capabilities', () => ({
      deferredFeatureReleaseLabel: 'v0.3',
      optionalAiFeaturesAvailable: false,
      readableContentFetchAvailable: false,
    }))

    try {
      const { JobsRuntimeHealthSection: DeferredJobsRuntimeHealthSection } =
        await import('./runtime-health-section')
      renderSection(
        <DeferredJobsRuntimeHealthSection
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

      // Deferred branch masks live counts (stored shows 0) and shows the
      // placeholder badge + body — never the honest off-state opt-in (that is
      // only correct once the feature has actually shipped).
      expect(screen.getByText('contentFetchDeferredBody')).toBeVisible()
      expect(screen.getByText('contentFetchDeferredBadge')).toBeVisible()
      expect(screen.queryByText('contentFetchOffBody')).toBeNull()
      expect(
        screen.queryByTestId('jobs-content-fetch-settings-link'),
      ).toBeNull()
      expect(screen.getAllByText(/savedReadableContent:\s*0/).length).toBe(1)
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
