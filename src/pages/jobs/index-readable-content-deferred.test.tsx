/**
 * @file index-readable-content-deferred.test.tsx
 * @description Jobs overview coverage for the deferred (release-flag-off) readable-content branch.
 * @module pages/jobs
 *
 * ## Responsibilities
 * - Exercise the Jobs route when `readableContentFetchAvailable` is false so the
 *   deferred placeholder copy + masked content count stay covered after ENR-2
 *   shipped the live default.
 *
 * ## Non-Responsibilities
 * - Does not cover the live branch (that is the unmocked default + the dedicated
 *   readable-content suite).
 *
 * ## Dependencies
 * - Reuses the shared Intelligence surface harness and mocks `release-capabilities`
 *   only inside this module.
 */

import { screen, within } from '@testing-library/react'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { createNamespaceTranslator } from '../../lib/i18n'
import {
  createEmptyRuntimeSnapshot,
  createShellValue,
  renderSurface,
  resetIntelligenceSurfaceHarness,
  seedArchiveState,
} from '../intelligence-surfaces/test-helpers'
import { JobsPage } from '.'

vi.mock('../../lib/release-capabilities', () => ({
  optionalAiFeaturesAvailable: false,
  readableContentFetchAvailable: false,
  deferredFeatureReleaseLabel: 'v0.3',
}))

describe('Jobs readable-content deferred overview branch', () => {
  beforeEach(() => {
    resetIntelligenceSurfaceHarness()
  })

  test('masks live content stats and shows the deferred copy while the flag is off', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')
    const runtime = createEmptyRuntimeSnapshot()
    // A real content plugin with stored rows: the deferred branch must NOT leak
    // its counts into the overview (it reports 0) and must keep the deferred copy.
    runtime.plugins = [
      {
        pluginId: 'readable-content-refetch',
        sourceKind: 'network',
        enabled: true,
        storedRecords: 17,
        queuedJobs: 2,
        runningJobs: 0,
        failedJobs: 0,
        lastCompletedAt: '2026-04-10T16:20:00Z',
        lastError: null,
      },
    ]
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
      intelligence: runtime,
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
      (await screen.findAllByText(jobsT('contentFetchDeferredBody'))).length,
    ).toBeGreaterThan(0)
    const storedReadableContentStat = screen
      .getAllByText(jobsT('savedReadableContent'))[0]
      .closest('.jobs-hero-stat')
    expect(storedReadableContentStat).toBeInstanceOf(HTMLElement)
    if (!(storedReadableContentStat instanceof HTMLElement)) {
      throw new Error('expected stored readable content overview stat')
    }
    // Masked to 0 despite the 17 stored rows in the runtime snapshot.
    expect(within(storedReadableContentStat).getByText('0')).toBeVisible()
  })
})
