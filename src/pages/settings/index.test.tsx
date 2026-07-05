/**
 * @file index.test.tsx
 * @description Route-shell coverage for the Settings page gating + paperLayout branching.
 * @module pages/settings
 *
 * ## Responsibilities
 * - Verify the snapshot-less loading / locked-archive / unavailable gates.
 * - Verify that `?layout=paper` swaps the legacy section nav for the
 *   paper-redesign header, and that the removed overview block stays gone.
 * - Keep the assertion small enough that the existing live-shell suites can
 *   continue to own the non-paper section composition baseline.
 *
 * ## Not responsible for
 * - Re-testing individual settings sections; their `*-section.test.tsx`
 *   files own those contracts.
 * - Re-testing the shell snapshot lifecycle; the canonical app-shell suites
 *   under `src/app/index-tests/` already exercise the populated path.
 *
 * ## Dependencies
 * - Mocks `useShellData`, `useSettingsRouteState`, and each extracted
 *   section so the page renders its branching logic deterministically.
 *
 * ## Performance notes
 * - Mocks keep the page hydration cost negligible; no archive bootstrap.
 */

import type { ReactNode } from 'react'
import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import type { AppSnapshot, SecurityStatus } from '../../lib/types'
import { SettingsPage } from './index'

const { shellDataMock, routeStateMock } = vi.hoisted(() => ({
  shellDataMock: vi.fn(),
  routeStateMock: vi.fn(),
}))

vi.mock('../../app/shell-data-context', () => ({
  useShellData: shellDataMock,
}))

vi.mock('./use-settings-route-state', () => ({
  useSettingsRouteState: routeStateMock,
}))

vi.mock('./appearance-section', () => ({
  AppearanceSection: () => <div data-testid="mock-appearance" />,
}))

vi.mock('./general-section', () => ({
  GeneralSection: () => <div data-testid="mock-general" />,
}))

vi.mock('./profile-selection-section', () => ({
  ProfileSelectionSection: () => <div data-testid="mock-profiles" />,
}))

vi.mock('./app-lock-section', () => ({
  AppLockSection: () => <div data-testid="mock-applock" />,
}))

vi.mock('./ai-providers-section', () => ({
  AiProvidersSection: () => <div data-testid="mock-ai" />,
}))

vi.mock('./data-migration-section', () => ({
  DataMigrationSection: () => <div data-testid="mock-migration" />,
}))

vi.mock('./link-previews-section', () => ({
  LinkPreviewsSection: () => <div data-testid="mock-link-previews" />,
}))

vi.mock('./paper-settings-header', () => ({
  PaperSettingsHeader: ({ testId }: { testId?: string }) => (
    <header data-testid={testId ?? 'paper-settings-header'} />
  ),
}))

// Capture the `items` identity the nav receives on every render so the suite
// can prove BUG A's root fix: the descriptor list is memoized, so an unrelated
// re-render at the same language hands the nav the SAME array reference (which
// is what keeps the deep-link auto-scroll from re-firing on every render).
const navItemsRenders: unknown[] = []
vi.mock('./section-nav', () => ({
  SettingsSectionNav: ({ items, label }: { items: unknown; label: string }) => {
    navItemsRenders.push(items)
    return <nav data-testid="mock-section-nav" aria-label={label} />
  },
}))

interface ShellOverrides {
  snapshot?: AppSnapshot | null
  loading?: boolean
}

interface RouteOverrides {
  supportStateLoaded?: boolean
  securityStatus?: SecurityStatus | null
}

function renderPage(
  route: string,
  shellOverrides: ShellOverrides = {},
  routeOverrides: RouteOverrides = {},
): void {
  shellDataMock.mockReturnValue({
    appLockStatus: null,
    buildInfo: null,
    clearAppLockPasscode: vi.fn(),
    dashboard: null,
    loading: shellOverrides.loading ?? false,
    lockAppSession: vi.fn(),
    refreshAppData: vi.fn().mockResolvedValue(undefined),
    refreshKey: 1,
    saveConfig: vi.fn(),
    setAppLockPasscode: vi.fn(),
    snapshot:
      shellOverrides.snapshot === undefined
        ? snapshotFixture()
        : shellOverrides.snapshot,
  })
  routeStateMock.mockReturnValue(
    routeStateFixture({
      supportStateLoaded: routeOverrides.supportStateLoaded ?? true,
      securityStatus: routeOverrides.securityStatus ?? null,
    }),
  )

  render(
    <MemoryRouter initialEntries={[route]}>
      <I18nProvider>
        <SettingsPage />
      </I18nProvider>
    </MemoryRouter>,
  )
}

describe('SettingsPage', () => {
  beforeEach(() => {
    navItemsRenders.length = 0
  })

  describe('section nav items stability', () => {
    test('hands the nav the SAME items array identity across an unrelated re-render at the same language', () => {
      // BUG A root fix: createSettingsSectionNavItems is memoized on `t`, so a
      // re-render that does not change the language (the AI-draft-edit case)
      // reuses the SAME array reference. Stable identity here is exactly what
      // stops the deep-link auto-scroll from re-firing every render.
      shellDataMock.mockReturnValue(populatedShellData())
      routeStateMock.mockReturnValue(routeStateFixture())

      // A fresh element each time forces SettingsPage to actually re-render
      // (React would bail on an identical element reference).
      const tree = () => (
        <MemoryRouter initialEntries={['/settings#settings-ai']}>
          <I18nProvider>
            <SettingsPage />
          </I18nProvider>
        </MemoryRouter>
      )
      const { rerender } = render(tree())

      expect(navItemsRenders.length).toBe(1)

      // Force an unrelated re-render at the same language (the mocked hooks hand
      // back fresh objects each call, exactly like an AI-draft edit would).
      rerender(tree())

      expect(navItemsRenders.length).toBe(2)
      // Same reference across both renders — the useMemo on `t` held, so the nav
      // never sees a new `items` identity that would re-fire the deep-link scroll.
      expect(navItemsRenders[1]).toBe(navItemsRenders[0])
    })
  })

  describe('snapshot-less gates', () => {
    test('renders the loading gate when shell snapshot is still loading', () => {
      renderPage(
        '/settings',
        { snapshot: null, loading: true },
        { supportStateLoaded: false },
      )

      expect(screen.getByText('Loading…')).toBeVisible()
      // The settings groups must not render until shell snapshot is ready.
      expect(screen.queryByTestId('mock-appearance')).toBeNull()
    })

    test('renders the loading gate when support state is still hydrating', () => {
      renderPage(
        '/settings',
        { snapshot: null, loading: false },
        { supportStateLoaded: false },
      )

      expect(screen.getByText('Loading…')).toBeVisible()
    })

    test('renders the locked-archive gate when the encrypted archive is not unlocked', () => {
      renderPage(
        '/settings',
        { snapshot: null, loading: false },
        {
          supportStateLoaded: true,
          securityStatus: lockedSecurityStatus(),
        },
      )

      expect(
        screen.getByText('Unlock the archive before reviewing settings'),
      ).toBeVisible()
      const reviewLink = screen.getByRole('link', { name: 'Check security' })
      expect(reviewLink).toHaveAttribute('href', '/security')
    })

    test('renders the generic unavailable gate when shell snapshot is missing without a locked archive', () => {
      renderPage(
        '/settings',
        { snapshot: null, loading: false },
        {
          supportStateLoaded: true,
          // An archive that's initialized + unlocked still leaves us snapshot-less
          // here, so the page falls through to the generic unavailable gate.
          securityStatus: unlockedSecurityStatus(),
        },
      )

      expect(
        screen.getByText('Settings are temporarily unavailable'),
      ).toBeVisible()
    })
  })

  describe('paperLayout branches', () => {
    test('renders the paper header instead of the legacy section nav when layout=paper', () => {
      renderPage('/settings?layout=paper')

      expect(screen.getByTestId('settings-paper-header')).toBeInTheDocument()
      expect(screen.queryByTestId('mock-section-nav')).toBeNull()
      // The settings groups themselves are still composed in paper mode.
      expect(screen.getByTestId('mock-appearance')).toBeInTheDocument()
      expect(screen.getByTestId('mock-ai')).toBeInTheDocument()
    })

    test('renders the legacy section nav when the layout query is absent', () => {
      renderPage('/settings')

      expect(screen.getByTestId('mock-section-nav')).toBeInTheDocument()
      expect(screen.queryByTestId('settings-paper-header')).toBeNull()
      // The removed intro/overview block must NOT render in either layout: it
      // duplicated the sidebar and wasted the first screen.
      expect(document.getElementById('settings-overview')).toBeNull()
      // The two in-page workflow cards (Open Maintenance / Open Integrations)
      // are gone — the section composition renders directly under the nav.
      expect(screen.getByTestId('mock-appearance')).toBeInTheDocument()
      expect(screen.getByTestId('mock-migration')).toBeInTheDocument()
    })
  })
})

function populatedShellData() {
  return {
    appLockStatus: null,
    buildInfo: null,
    clearAppLockPasscode: vi.fn(),
    dashboard: null,
    loading: false,
    lockAppSession: vi.fn(),
    refreshAppData: vi.fn().mockResolvedValue(undefined),
    refreshKey: 1,
    saveConfig: vi.fn(),
    setAppLockPasscode: vi.fn(),
    snapshot: snapshotFixture(),
  }
}

function snapshotFixture(): AppSnapshot {
  // We only need a truthy snapshot — the SettingsPage forwards it to
  // mocked sections, so the schema details are irrelevant for this suite.
  return { fake: 'snapshot' } as unknown as AppSnapshot
}

interface RouteStateFixture {
  supportStateLoaded: boolean
  supportState: { securityStatus: SecurityStatus | null }
  general: {
    explorerBackgroundPrefetchPages: number
    onExplorerBackgroundPrefetchPagesChange: ReturnType<typeof vi.fn>
    onLanguageChange: ReturnType<typeof vi.fn>
    saving: boolean
  }
  profiles: Record<string, unknown>
  appLock: Record<string, unknown>
  ai: Record<string, unknown>
  children?: ReactNode
}

function routeStateFixture(
  overrides: {
    supportStateLoaded?: boolean
    securityStatus?: SecurityStatus | null
  } = {},
): RouteStateFixture {
  return {
    supportStateLoaded: overrides.supportStateLoaded ?? true,
    supportState: { securityStatus: overrides.securityStatus ?? null },
    general: {
      explorerBackgroundPrefetchPages: 4,
      onExplorerBackgroundPrefetchPagesChange: vi.fn(),
      onLanguageChange: vi.fn(),
      saving: false,
    },
    profiles: {},
    appLock: {},
    ai: {},
  }
}

function lockedSecurityStatus(): SecurityStatus {
  return {
    initialized: true,
    mode: 'Encrypted',
    encrypted: true,
    unlocked: false,
    databasePath: '/tmp/history.sqlite',
    strongholdPath: '/tmp/stronghold',
    rememberDatabaseKeyInKeyring: false,
    lastSuccessfulBackupAt: null,
    lastRekeyAt: null,
    lastRekeyRunId: null,
    lastRekeySnapshotPath: null,
    keyringStatus: {
      available: true,
      backend: 'file-backed-test',
      storedSecret: false,
    },
    warnings: [],
  }
}

function unlockedSecurityStatus(): SecurityStatus {
  return {
    ...lockedSecurityStatus(),
    encrypted: false,
    unlocked: true,
  }
}
