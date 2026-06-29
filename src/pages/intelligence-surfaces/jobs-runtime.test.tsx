/**
 * @file jobs-runtime.test.tsx
 * @description Thin gate-verification suite for the new Activity center (JobsPage).
 * @module pages/intelligence-surfaces
 *
 * ## Responsibilities
 * - Verify the loading, setup, locked, and runtime-loading gates still work on the
 *   new ActivityPage after the redesign.
 * - Verify the activity page title is visible in normal operation.
 *
 * ## Non-Responsibilities
 * - Does not repeat the full zone tests from src/pages/jobs/index.test.tsx.
 * - Does not test the old job panels, hero cards, or legacy copy that was removed.
 *
 * ## Dependencies
 * - Shared test harness in test-helpers.tsx.
 */

import { screen } from '@testing-library/react'
import { beforeEach, describe, expect, test } from 'vitest'
import { createNamespaceTranslator } from '../../lib/i18n'
import { JobsPage } from '../jobs'
import {
  createShellValue,
  renderSurface,
  resetIntelligenceSurfaceHarness,
  seedArchiveState,
} from './test-helpers'

describe('intelligence surfaces — Activity center gates', () => {
  beforeEach(() => {
    resetIntelligenceSurfaceHarness()
  })

  test('loading gate renders activity-page-skeleton when snapshot is null and loading=true', async () => {
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

  test('setup gate renders setupTitle when archive is not initialized', async () => {
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

  test('locked gate renders lockedTitle when archive is locked', async () => {
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

  test('runtime-loading gate renders skeleton when aiQueue and intelligence are both null', async () => {
    const { snapshot } = await seedArchiveState()

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue: {
        ...createShellValue(snapshot),
        runtimeStatus: {
          aiQueue: null,
          intelligence: null,
          loading: false,
          error: null,
        },
      },
      snapshot,
    })

    expect(screen.getByTestId('activity-page-skeleton')).toBeInTheDocument()
  })

  test('normal state shows activityPageTitle visible', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue: createShellValue(snapshot),
      snapshot,
    })

    expect(screen.getByText(jobsT('activityPageTitle'))).toBeVisible()
  })
})
