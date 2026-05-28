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

import { render, renderHook, screen, waitFor } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { afterEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import { createTranslator } from '../../lib/i18n'
import type { AppSnapshot, SecurityStatus } from '../../lib/types'
import {
  shouldProbeDashboardArchiveAccessFallback,
  toDashboardArchiveAccessFallback,
  useDashboardArchiveAccessFallback,
} from './route-fallback-access'
import { DashboardRouteFallback } from './route-fallback'
import { resolveDashboardRouteFallback } from './route-fallback-state'

const t = createTranslator('en')

const securityStatus: SecurityStatus = {
  initialized: true,
  mode: 'encrypted',
  encrypted: true,
  unlocked: false,
  databasePath: '/tmp/pathkeep/archive.sqlite',
  strongholdPath: '/tmp/pathkeep/vault.hold',
  rememberDatabaseKeyInKeyring: true,
  keyringStatus: {
    available: true,
    backend: 'Test keyring',
    storedSecret: false,
  },
  lastSuccessfulBackupAt: null,
  lastRekeyAt: null,
  lastRekeyRunId: null,
  lastRekeySnapshotPath: null,
  warnings: [],
}

describe('dashboard route fallback', () => {
  afterEach(() => {
    vi.restoreAllMocks()
  })

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

  test('resolves read-error when an error landed but the archive is reachable and initialized', () => {
    // Encrypted=false, initialized=true, unlocked=true → no unlock needed; the
    // resolver must fall through to the generic read-error branch so the user
    // sees the underlying message.
    expect(
      resolveDashboardRouteFallback({
        archiveAccessFallback: {
          encrypted: false,
          initialized: true,
          unlocked: true,
        },
        dashboard: null,
        dashboardLoading: false,
        error: 'disk read failed',
        loading: false,
        snapshot: null,
      }),
    ).toEqual({ description: 'disk read failed', kind: 'read-error' })
  })

  test('resolves archive-unavailable when bootstrap has no ready snapshot pair', () => {
    expect(
      resolveDashboardRouteFallback({
        archiveAccessFallback: null,
        dashboard: null,
        dashboardLoading: false,
        error: null,
        loading: false,
        snapshot: null,
      }),
    ).toEqual({ kind: 'archive-unavailable' })
  })

  test('resolves onboarding zero-state when snapshot exists but the archive has not been initialized yet', () => {
    expect(
      resolveDashboardRouteFallback({
        archiveAccessFallback: null,
        dashboard: null,
        dashboardLoading: false,
        error: null,
        loading: false,
        snapshot: {
          config: { initialized: false },
        } as never,
      }),
    ).toEqual({ kind: 'onboarding-zero-state' })
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

  test('renders loading, read-error, and archive-unavailable fallback branches', () => {
    render(
      <MemoryRouter>
        <>
          <DashboardRouteFallback state={{ kind: 'loading' }} t={t} />
          <DashboardRouteFallback
            state={{ kind: 'read-error', description: 'sqlite unavailable' }}
            t={t}
          />
          <DashboardRouteFallback
            state={{ kind: 'archive-unavailable' }}
            t={t}
          />
        </>
      </MemoryRouter>,
    )

    expect(screen.getByLabelText(t('loadingDashboard'))).toHaveAttribute(
      'aria-busy',
      'true',
    )
    expect(screen.getByText('sqlite unavailable')).toBeVisible()
    expect(screen.getByText(t('archiveUnavailable'))).toBeVisible()
  })

  test('keeps archive-access probing limited to bootstrap error states', () => {
    expect(
      shouldProbeDashboardArchiveAccessFallback({
        dashboard: null,
        error: 'database key required',
        snapshot: null,
      }),
    ).toBe(true)
    expect(
      shouldProbeDashboardArchiveAccessFallback({
        dashboard: null,
        error: null,
        snapshot: null,
      }),
    ).toBe(false)
    expect(
      shouldProbeDashboardArchiveAccessFallback({
        dashboard: null,
        error: 'database key required',
        snapshot: {} as AppSnapshot,
      }),
    ).toBe(false)
  })

  test('narrows Security status to the Dashboard fallback fields', () => {
    expect(toDashboardArchiveAccessFallback(securityStatus)).toEqual({
      encrypted: true,
      initialized: true,
      unlocked: false,
    })
  })

  test('loads archive-access fallback through the hook and clears failed probes', async () => {
    const securityStatusSpy = vi
      .spyOn(backend, 'securityStatus')
      .mockResolvedValueOnce(securityStatus)
      .mockRejectedValueOnce(new Error('security unavailable'))
    const initialHookProps: { error: string | null; refreshKey: number } = {
      error: 'database key required',
      refreshKey: 0,
    }
    const { rerender, result } = renderHook(
      ({ error, refreshKey }: { error: string | null; refreshKey: number }) =>
        useDashboardArchiveAccessFallback({
          dashboard: null,
          error,
          refreshKey,
          snapshot: null,
        }),
      {
        initialProps: initialHookProps,
      },
    )

    await waitFor(() =>
      expect(result.current).toEqual({
        encrypted: true,
        initialized: true,
        unlocked: false,
      }),
    )

    rerender({
      error: 'database key required',
      refreshKey: 1,
    })

    await waitFor(() => expect(result.current).toBeNull())
    expect(securityStatusSpy).toHaveBeenCalledTimes(2)

    rerender({
      error: null,
      refreshKey: 2,
    })

    await waitFor(() => expect(result.current).toBeNull())
    expect(securityStatusSpy).toHaveBeenCalledTimes(2)
  })

  test('cancels the queued fallback clear when normal route state unmounts', async () => {
    const consoleErrorSpy = vi
      .spyOn(console, 'error')
      .mockImplementation(() => undefined)

    const { unmount } = renderHook(() =>
      useDashboardArchiveAccessFallback({
        dashboard: null,
        error: null,
        refreshKey: 0,
        snapshot: null,
      }),
    )

    unmount()
    await Promise.resolve()

    expect(consoleErrorSpy).not.toHaveBeenCalled()
  })

  test('ignores archive-access fallback probes that resolve after cleanup', async () => {
    let resolveStatus: (status: SecurityStatus) => void = () => {}
    const statusPromise = new Promise<SecurityStatus>((resolve) => {
      resolveStatus = resolve
    })
    vi.spyOn(backend, 'securityStatus').mockReturnValue(statusPromise)

    const { result, unmount } = renderHook(() =>
      useDashboardArchiveAccessFallback({
        dashboard: null,
        error: 'database key required',
        refreshKey: 0,
        snapshot: null,
      }),
    )

    unmount()
    resolveStatus(securityStatus)
    await statusPromise

    expect(result.current).toBeNull()
  })

  test('ignores archive-access fallback probes that reject after cleanup', async () => {
    let rejectStatus: (error: Error) => void = () => {}
    const statusPromise = new Promise<SecurityStatus>((_resolve, reject) => {
      rejectStatus = reject
    })
    vi.spyOn(backend, 'securityStatus').mockReturnValue(statusPromise)

    const { result, unmount } = renderHook(() =>
      useDashboardArchiveAccessFallback({
        dashboard: null,
        error: 'database key required',
        refreshKey: 0,
        snapshot: null,
      }),
    )

    unmount()
    rejectStatus(new Error('security unavailable'))
    await statusPromise.catch(() => undefined)

    expect(result.current).toBeNull()
  })
})
