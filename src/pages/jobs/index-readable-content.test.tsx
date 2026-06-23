/**
 * @file index-readable-content.test.tsx
 * @description Jobs overview coverage for the future readable-content enabled branch.
 * @module pages/jobs
 *
 * ## Responsibilities
 * - Exercise the Jobs route when the release capability flag allows readable-content fetching.
 * - Protect the queued, running, ready, and missing-plugin copy branches from regressing while v0.2 keeps them disabled.
 * - Reuse the shared route harness so release-flag mocking does not drift from app shell behavior.
 *
 * ## Non-Responsibilities
 * - Does not enable readable-content fetching for the shipped v0.2.0 runtime.
 * - Does not test runtime-health internals; those remain covered by the dedicated section suite.
 * - Does not redefine the shared Intelligence surface fixtures.
 *
 * ## Dependencies
 * - Depends on the Jobs route component and the shared Intelligence surface harness.
 * - Mocks `release-capabilities` only inside this test module to cover the future branch.
 */

import { screen, within } from '@testing-library/react'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { createNamespaceTranslator } from '../../lib/i18n'
import type { IntelligenceRuntimeSnapshot } from '../../lib/types'
import {
  createEmptyRuntimeSnapshot,
  createShellValue,
  renderSurface,
  resetIntelligenceSurfaceHarness,
  seedArchiveState,
} from '../intelligence-surfaces/test-helpers'
import { JobsPage } from '.'

type RuntimePluginStatus = IntelligenceRuntimeSnapshot['plugins'][number]

vi.mock('../../lib/release-capabilities', () => ({
  optionalAiFeaturesAvailable: false,
  readableContentFetchAvailable: true,
  deferredFeatureReleaseLabel: 'v0.3',
}))

describe('Jobs readable-content enabled overview branch', () => {
  beforeEach(() => {
    resetIntelligenceSurfaceHarness()
  })

  test.each([
    {
      name: 'missing plugin',
      plugin: null,
      messageKey: 'contentFetchOffBody',
      storedRows: 0,
    },
    {
      name: 'queued backlog',
      plugin: contentPlugin({ storedRecords: 12, queuedJobs: 3 }),
      messageKey: 'contentFetchBacklogBody',
      storedRows: 12,
    },
    {
      name: 'running backlog',
      plugin: contentPlugin({ storedRecords: 8, runningJobs: 2 }),
      messageKey: 'contentFetchRunningBody',
      storedRows: 8,
    },
    {
      name: 'ready queue',
      plugin: contentPlugin({ storedRecords: 21 }),
      messageKey: 'contentFetchReadyBody',
      storedRows: 21,
    },
  ])(
    'renders readable-content $name messaging',
    async ({ plugin, messageKey, storedRows }) => {
      const { snapshot } = await seedArchiveState()
      const jobsT = createNamespaceTranslator('en', 'jobs')
      const runtime = createEmptyRuntimeSnapshot()
      runtime.plugins = plugin ? [plugin] : []
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
        intelligence: runtime,
        loading: false,
        error: null,
      }

      const { unmount } = renderSurface(<JobsPage />, {
        language: 'en',
        route: '/jobs',
        shellValue,
        snapshot,
      })

      const contentMessages = await screen.findAllByText(
        jobsT(messageKey as 'contentFetchOffBody', {
          queued: plugin?.queuedJobs ?? 0,
          stored: plugin?.storedRecords ?? 0,
        }),
      )
      expect(contentMessages.length).toBeGreaterThan(0)

      const storedReadableContentStat = screen
        .getAllByText(jobsT('savedReadableContent'))[0]
        .closest('.jobs-hero-stat')
      expect(storedReadableContentStat).toBeInstanceOf(HTMLElement)
      if (!(storedReadableContentStat instanceof HTMLElement)) {
        throw new Error('expected stored readable content overview stat')
      }
      expect(
        within(storedReadableContentStat).getByText(
          storedRows.toLocaleString('en'),
        ),
      ).toBeVisible()

      unmount()
    },
  )
})

function contentPlugin(
  overrides: Partial<RuntimePluginStatus> = {},
): RuntimePluginStatus {
  return {
    pluginId: 'readable-content-refetch',
    sourceKind: 'network',
    enabled: true,
    storedRecords: 0,
    queuedJobs: 0,
    runningJobs: 0,
    failedJobs: 0,
    lastCompletedAt: '2026-04-10T16:20:00Z',
    lastError: null,
    ...overrides,
  }
}
