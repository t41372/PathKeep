/**
 * @file route-content.test.tsx
 * @description Extracted live-data route-content matrix from the app-shell mega-suite.
 * @module app/index-tests
 *
 * ## Responsibilities
 * - Preserve the original shell route-content smoke matrix from `src/app/index.test.tsx`.
 * - Reuse the canonical app-shell test harness so split suites keep the same router and data contracts.
 * - Keep the entry-to-sentinel mapping readable while the larger shell suite is split incrementally.
 *
 * ## Not responsible for
 * - Does not redefine shared shell helpers, route metadata, or route-specific assertions beyond this extracted matrix.
 * - Does not modify the original mega-suite or take ownership of neighboring test slices.
 * - Does not add new coverage behavior beyond the existing `renders route $entry with live data-backed content` contract.
 *
 * ## Dependencies
 * - Depends on `App` and `appRoutes`, which remain the source of truth for shell routing behavior.
 * - Depends on `src/app/index-tests/test-helpers.tsx` for harness reset, archive seeding, schedule seeding, and i18n sentinels.
 *
 * ## Performance notes
 * - Seeds one initialized archive per case so route smoke coverage stays aligned with live read models without extra mock layers.
 */

import { render, screen, waitFor, within } from '@testing-library/react'
import { createMemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test } from 'vitest'
import App from '../index'
import { appRoutes } from '../router'
import {
  assistantT,
  intelligenceT,
  resetAppShellHarness,
  scheduleT,
  seedArchiveRun,
  seedInteractiveSchedule,
  securityT,
  settingsT,
} from './test-helpers'

describe('App shell', () => {
  beforeEach(() => {
    resetAppShellHarness()
  })

  test.each([
    {
      entry: '/schedule',
      pageTestId: 'schedule-page',
      sentinel: scheduleT('pageTitle'),
      prepare: () => seedInteractiveSchedule(),
    },
    {
      entry: '/security',
      pageTestId: 'security-page',
      sentinel: securityT('encryptionStatus'),
    },
    {
      entry: '/assistant',
      pageTestId: null,
      sentinel: assistantT('deferredTitle'),
    },
    {
      entry: '/intelligence',
      pageTestId: 'intelligence-page',
      sentinel: intelligenceT('digestTitle'),
    },
    {
      entry: '/intelligence/day/2026-04-18',
      pageTestId: 'day-insights-page',
      sentinel: intelligenceT('dayInsightsTitle'),
    },
    {
      entry: '/settings',
      pageTestId: 'settings-page',
      sentinel: settingsT('preferencesOverview'),
    },
    {
      entry: '/maintenance',
      pageTestId: 'maintenance-page',
      sentinel: settingsT('maintenanceTitle'),
    },
    {
      entry: '/integrations',
      pageTestId: 'integrations-page',
      sentinel: settingsT('integrationsTitle'),
    },
  ])(
    'renders route $entry with live data-backed content',
    async ({ entry, pageTestId, sentinel, prepare }) => {
      await seedArchiveRun()
      prepare?.()
      const router = createMemoryRouter(appRoutes, {
        initialEntries: [entry],
      })

      render(<App router={router} />)

      if (!pageTestId) {
        expect(await screen.findByText(sentinel)).toBeVisible()
        return
      }

      await waitFor(() => {
        const page = screen.getByTestId(pageTestId)
        expect(within(page).getAllByText(sentinel)[0]).toBeVisible()
      })
    },
  )
})
