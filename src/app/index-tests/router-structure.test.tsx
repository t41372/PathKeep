/**
 * @file router-structure.test.tsx
 * @description Structure-only app-shell slice for sidebar grouping and router-handle contracts.
 * @module app/index-tests
 *
 * ## Responsibilities
 * - Preserve the exact sidebar information architecture grouping contract.
 * - Verify desktop-router creation keeps the onboarding route handle readable.
 * - Reuse the shared shell reset contract so split suites stay behavior-identical.
 *
 * ## Not responsible for
 * - Rendering route content or onboarding/dashboard flows.
 * - Defining shell metadata; this suite only verifies the existing contract.
 * - Recreating backend reset logic outside the shared test helper.
 *
 * ## Dependencies
 * - Depends on `../router` for sidebar sections, onboarding metadata, and route handles.
 * - Depends on `../router-factory` for the desktop router contract.
 * - Depends on `./test-helpers` for the shared app-shell reset behavior.
 *
 * ## Performance notes
 * - Uses structure-only assertions so suite splitting does not add extra render or bootstrap work.
 */

import { beforeEach, describe, expect, test } from 'vitest'
import { createDesktopRouter } from '../router-factory'
import {
  appRoutes,
  onboardingScreen,
  readRouteHandle,
  sidebarSections,
} from '../router'
import { resetAppShellHarness } from './test-helpers'

describe('App shell', () => {
  beforeEach(() => {
    resetAppShellHarness()
  })

  test('keeps sidebar information architecture grouped by section', () => {
    expect(sidebarSections).toEqual([
      {
        id: 'core',
        labelKey: 'navigation.coreSection',
        items: [
          expect.objectContaining({
            id: 'dashboard',
            labelKey: 'navigation.dashboardLabel',
            subtitleKey: 'navigation.dashboardSubtitle',
            icon: '⌂',
            href: '/',
          }),
          expect.objectContaining({
            id: 'explorer',
            labelKey: 'navigation.explorerLabel',
            subtitleKey: 'navigation.explorerSubtitle',
            icon: '◎',
            href: '/explorer',
          }),
          expect.objectContaining({
            id: 'intelligence',
            labelKey: 'navigation.intelligenceLabel',
            subtitleKey: 'navigation.intelligenceSubtitle',
            titleKey: 'navigation.intelligenceTitle',
            section: 'CORE',
            icon: '◈',
            href: '/intelligence',
          }),
          expect.objectContaining({
            id: 'assistant',
            labelKey: 'navigation.assistantLabel',
            subtitleKey: 'navigation.assistantSubtitle',
            icon: '▷',
            href: '/assistant',
            badgeKey: 'navigation.assistantBadge',
          }),
        ],
      },
      {
        id: 'operations',
        labelKey: 'navigation.operationsSection',
        items: [
          expect.objectContaining({
            id: 'import',
            labelKey: 'navigation.importLabel',
            subtitleKey: 'navigation.importSubtitle',
            icon: '↓',
            href: '/import',
          }),
          expect.objectContaining({
            id: 'audit',
            labelKey: 'navigation.auditLabel',
            subtitleKey: 'navigation.auditSubtitle',
            icon: '⊞',
            href: '/audit',
          }),
          expect.objectContaining({
            id: 'jobs',
            labelKey: 'navigation.jobsLabel',
            subtitleKey: 'navigation.jobsSubtitle',
            icon: '≡',
            href: '/jobs',
          }),
          expect.objectContaining({
            id: 'schedule',
            labelKey: 'navigation.scheduleLabel',
            subtitleKey: 'navigation.scheduleSubtitle',
            icon: '⏀',
            href: '/schedule',
          }),
          expect.objectContaining({
            id: 'integrations',
            labelKey: 'navigation.integrationsLabel',
            subtitleKey: 'navigation.integrationsSubtitle',
            icon: '⧉',
            href: '/integrations',
          }),
        ],
      },
      {
        id: 'system',
        labelKey: 'navigation.systemSection',
        items: [
          expect.objectContaining({
            id: 'security',
            labelKey: 'navigation.securityLabel',
            subtitleKey: 'navigation.securitySubtitle',
            icon: '⊘',
            href: '/security',
          }),
          expect.objectContaining({
            id: 'settings',
            labelKey: 'navigation.settingsLabel',
            subtitleKey: 'navigation.settingsSubtitle',
            icon: '⚙',
            href: '/settings',
          }),
          expect.objectContaining({
            id: 'maintenance',
            labelKey: 'navigation.maintenanceLabel',
            subtitleKey: 'navigation.maintenanceSubtitle',
            icon: '◇',
            href: '/maintenance',
          }),
        ],
      },
    ])
    expect(onboardingScreen).toEqual(
      expect.objectContaining({
        labelKey: 'navigation.onboardingLabel',
        titleKey: 'navigation.onboardingTitle',
        subtitleKey: 'navigation.onboardingSubtitle',
        icon: '◌',
        href: '/onboarding',
      }),
    )
    expect(appRoutes[0]).toEqual(expect.objectContaining({ path: '/' }))
  })

  test('creates a desktop router and validates route handles', () => {
    const router = createDesktopRouter()

    expect(readRouteHandle(null)).toBeNull()
    expect(readRouteHandle({})).toBeNull()
    expect(readRouteHandle({ screen: null })).toBeNull()
    expect(readRouteHandle({ screen: 'dashboard' })).toBeNull()
    expect(readRouteHandle({ screen: onboardingScreen })).toEqual({
      screen: onboardingScreen,
    })
    expect(router.state.location.pathname).toBe('/')

    router.dispose()
  })
})
