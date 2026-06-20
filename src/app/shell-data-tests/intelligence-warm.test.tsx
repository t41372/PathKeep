/**
 * @file intelligence-warm.test.tsx
 * @description Covers the shell's `/intelligence` overview warm policy.
 * @module app/shell-data-tests/intelligence-warm
 *
 * ## Responsibilities
 * - Verify the shell warms the overview scope the `/intelligence` route will
 *   actually request: the active profile scope (archive-wide when unscoped) via
 *   the prioritized multi-preset warm, plus the archive-wide all-time default
 *   when a specific profile is active so clearing the scope stays instant.
 * - Confirm the bounded presets are NOT warmed a second time for the archive-wide
 *   companion (the archive-wide warm is all-time only).
 * - Confirm pending warms are cancelled on unmount.
 *
 * ## Not responsible for
 * - The preload implementation itself (covered by `preload.test.ts`).
 * - Snapshot fingerprinting or backend warm cost (owned by the Rust layer).
 *
 * ## Dependencies
 * - Mocks the two preload entry points from the core-intelligence barrel (via
 *   `importOriginal`) so the rest of the shell wiring stays real.
 */

import { render, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import type { ReactNode } from 'react'
import type * as coreIntelligenceModule from '../../lib/core-intelligence'

const { preloadAllTimeMock, preloadOverviewsMock } = vi.hoisted(() => ({
  preloadAllTimeMock: vi.fn<(profileId: string | null) => () => void>(() =>
    vi.fn(),
  ),
  preloadOverviewsMock: vi.fn<(profileId: string | null) => () => void>(() =>
    vi.fn(),
  ),
}))

vi.mock('../../lib/core-intelligence', async (importOriginal) => {
  const actual = await importOriginal<typeof coreIntelligenceModule>()
  return {
    ...actual,
    preloadAllTimeIntelligenceOverview: preloadAllTimeMock,
    preloadIntelligenceOverviews: preloadOverviewsMock,
  }
})

import { backend } from '../../lib/backend-client'
import { I18nContext } from '../../lib/i18n/context'
import { ProfileScopeProvider } from '../../lib/profile-scope'
import { ShellDataProvider } from '../shell-data'
import {
  createI18nValue,
  getDefaultBuildInfo,
  resetShellDataHarness,
  seedSnapshot,
} from './test-helpers'

const profileScopeStorageKey = 'pathkeep.profile-scope'

function renderShell(): { unmount: () => void } {
  return render(
    <I18nContext.Provider value={createI18nValue('en')}>
      <ProfileScopeProvider>
        <ShellDataProvider>
          <ShellProbeChild />
        </ShellDataProvider>
      </ProfileScopeProvider>
    </I18nContext.Provider>,
  )
}

function ShellProbeChild(): ReactNode {
  return null
}

describe('ShellDataProvider intelligence warm', () => {
  beforeEach(() => {
    resetShellDataHarness()
    preloadAllTimeMock.mockClear()
    preloadOverviewsMock.mockClear()
    window.localStorage.removeItem(profileScopeStorageKey)
  })

  afterEach(() => {
    window.localStorage.removeItem(profileScopeStorageKey)
  })

  test('warms only the archive-wide scope via the multi-preset warm when no profile is active', async () => {
    const { dashboard, snapshot } = await seedSnapshot()
    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)

    renderShell()

    await waitFor(() => expect(preloadOverviewsMock).toHaveBeenCalled())
    // The active (unscoped) warm covers all-time + bounded presets for null.
    expect(preloadOverviewsMock).toHaveBeenCalledWith(null)
    expect(
      preloadOverviewsMock.mock.calls.every(
        ([profileId]) => profileId === null,
      ),
    ).toBe(true)
    // No separate archive-wide all-time warm is issued when already unscoped.
    expect(preloadAllTimeMock).not.toHaveBeenCalled()
  })

  test('warms the active scope (all presets) and the archive-wide all-time default, cancelling both on unmount', async () => {
    window.localStorage.setItem(profileScopeStorageKey, 'chrome:Default')
    const cancelActive = vi.fn()
    const cancelArchiveWide = vi.fn()
    preloadOverviewsMock.mockImplementationOnce(() => cancelActive)
    preloadAllTimeMock.mockImplementationOnce(() => cancelArchiveWide)

    const { dashboard, snapshot } = await seedSnapshot()
    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)

    const { unmount } = renderShell()

    await waitFor(() =>
      expect(preloadOverviewsMock).toHaveBeenCalledWith('chrome:Default'),
    )
    // The scoped multi-preset warm seeds the keys the route reads; the
    // archive-wide all-time warm (only) keeps a later scope-clear instant
    // without paying the bounded presets' cold recompute twice.
    expect(preloadOverviewsMock).toHaveBeenCalledWith('chrome:Default')
    expect(preloadAllTimeMock).toHaveBeenCalledWith(null)
    // The bounded presets are not warmed a second time for archive-wide.
    expect(preloadOverviewsMock).not.toHaveBeenCalledWith(null)

    unmount()
    expect(cancelActive).toHaveBeenCalledTimes(1)
    expect(cancelArchiveWide).toHaveBeenCalledTimes(1)
  })
})
