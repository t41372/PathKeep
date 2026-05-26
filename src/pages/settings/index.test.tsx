/**
 * @file index.test.tsx
 * @description Route-shell coverage for the Settings page gating + paperLayout branching.
 * @module pages/settings
 *
 * ## Responsibilities
 * - Verify the snapshot-less loading / locked-archive / unavailable gates.
 * - Verify that `?layout=paper` swaps the legacy section nav + overview block
 *   for the paper-redesign header.
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
import { describe, expect, test, vi } from 'vitest'
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

vi.mock('./section-nav', () => ({
  SettingsSectionNav: ({ label }: { label: string }) => (
    <nav data-testid="mock-section-nav" aria-label={label} />
  ),
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
    test('renders the paper header and omits the legacy overview when layout=paper', () => {
      renderPage('/settings?layout=paper')

      expect(screen.getByTestId('settings-paper-header')).toBeInTheDocument()
      expect(screen.queryByTestId('mock-section-nav')).toBeNull()
      // The legacy `.settings-overview` block (h2#settings-overview + workflow
      // links) must NOT render in paper mode.
      expect(document.getElementById('settings-overview')).toBeNull()
      // The settings groups themselves are still composed in paper mode.
      expect(screen.getByTestId('mock-appearance')).toBeInTheDocument()
      expect(screen.getByTestId('mock-ai')).toBeInTheDocument()
    })

    test('renders the legacy section nav and overview block when layout query is absent', () => {
      renderPage('/settings')

      expect(screen.getByTestId('mock-section-nav')).toBeInTheDocument()
      expect(screen.queryByTestId('settings-paper-header')).toBeNull()
      expect(document.getElementById('settings-overview')).not.toBeNull()
      // The two workflow links — Maintenance and Integrations — are rendered.
      const links = screen
        .getAllByRole('link')
        .map((link) => link.getAttribute('href'))
      expect(links).toContain('/maintenance')
      expect(links).toContain('/integrations')
    })
  })
})

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
