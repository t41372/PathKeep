/**
 * @file route-fallback.test.tsx
 * @description Guards the Dashboard route fallback owner after extracting loading and bootstrap error branches out of the route shell.
 * @module pages/dashboard
 *
 * ## Responsibilities
 * - Verify fallback-state resolution for loading, unlock-required, onboarding-zero-state, and unavailable branches.
 * - Verify the fallback renderer keeps the shipped onboarding/security links intact.
 *
 * ## Not responsible for
 * - Re-testing the fully populated Dashboard route.
 * - Covering rhythm, On This Day, or populated panel composition.
 *
 * ## Dependencies
 * - Depends on Dashboard i18n strings and MemoryRouter for link assertions.
 *
 * ## Performance notes
 * - Focused render and pure-helper tests avoid mounting the full Dashboard route for every fallback branch.
 */

import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test } from 'vitest'
import { createTranslator } from '../../lib/i18n'
import { DashboardRouteFallback } from './route-fallback'
import { resolveDashboardRouteFallback } from './route-fallback-state'

const t = createTranslator('en')

describe('dashboard route fallback', () => {
  test('resolves loading before any dashboard snapshot is ready', () => {
    expect(
      resolveDashboardRouteFallback({
        archiveAccessFallback: null,
        dashboard: null,
        dashboardLoading: false,
        error: null,
        loading: true,
        snapshot: null,
      }),
    ).toEqual({ kind: 'loading' })
  })

  test('resolves onboarding zero-state when bootstrap failed before initialization', () => {
    expect(
      resolveDashboardRouteFallback({
        archiveAccessFallback: {
          encrypted: false,
          initialized: false,
          unlocked: false,
        },
        dashboard: null,
        dashboardLoading: false,
        error: t('archiveUnavailableBody'),
        loading: false,
        snapshot: null,
      }),
    ).toEqual({ kind: 'onboarding-zero-state' })
  })

  test('resolves unlock-required when archive access fallback says the archive is locked', () => {
    expect(
      resolveDashboardRouteFallback({
        archiveAccessFallback: {
          encrypted: true,
          initialized: true,
          unlocked: false,
        },
        dashboard: null,
        dashboardLoading: false,
        error: t('archiveUnavailableBody'),
        loading: false,
        snapshot: null,
      }),
    ).toEqual({ kind: 'unlock-required' })
  })

  test('renders the onboarding and security actions for non-ready fallback states', () => {
    render(
      <MemoryRouter>
        <>
          <DashboardRouteFallback
            state={{ kind: 'onboarding-zero-state' }}
            t={t}
          />
          <DashboardRouteFallback state={{ kind: 'unlock-required' }} t={t} />
        </>
      </MemoryRouter>,
    )

    expect(
      screen.getByRole('link', { name: t('openOnboardingFlow') }),
    ).toHaveAttribute('href', '/onboarding')
    expect(
      screen.getByRole('link', { name: t('archiveUnlockAction') }),
    ).toHaveAttribute('href', '/security#unlock-archive')
  })
})
