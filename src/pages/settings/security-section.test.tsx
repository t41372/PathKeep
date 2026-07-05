/**
 * @file security-section.test.tsx
 * @description Coverage for the Settings > Archive Key section.
 * @module pages/settings
 *
 * ## What this suite covers
 * - Section renders keychain status correctly for each state.
 * - Toggle reflects the current config value (on/off).
 * - Toggling ON saves config with rememberDatabaseKeyInKeyring=true and flashes "Saved".
 * - Toggling OFF clears the keyring entry AND saves config with false, then flashes "Saved".
 * - When keychain is unavailable the toggle is disabled (value=false forced).
 */

import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import { backend } from '../../lib/backend-client'
import type * as BackendClient from '../../lib/backend-client'
import {
  ShellDataContext,
  type ShellDataContextValue,
} from '../../app/shell-data-context'
import type {
  AppConfig,
  AppSnapshot,
  KeyringStatusReport,
} from '../../lib/types'
import type { SettingsSectionNavItem } from './section-nav-items'
import { SecuritySection } from './security-section'

// ──────────────────────────────────────────────────────────────────────────────
// Mock backend
// ──────────────────────────────────────────────────────────────────────────────
vi.mock('../../lib/backend-client', async (importOriginal) => {
  const actual = await importOriginal<typeof BackendClient>()
  return {
    ...actual,
    backend: {
      ...actual.backend,
      keyringClearDatabaseKey: vi.fn().mockResolvedValue({
        available: true,
        backend: 'macOS Keychain',
        storedSecret: false,
      }),
    },
  }
})

// ──────────────────────────────────────────────────────────────────────────────
// Fixture helpers
// ──────────────────────────────────────────────────────────────────────────────

const navItem: SettingsSectionNavItem = {
  id: 'settings-security',
  icon: 'database',
  key: 'security',
  label: 'ARCHIVE KEY',
}

function keyringFixture(
  overrides: Partial<KeyringStatusReport> = {},
): KeyringStatusReport {
  return {
    available: true,
    backend: 'macOS Keychain',
    storedSecret: false,
    ...overrides,
  }
}

function configFixture(overrides: Partial<AppConfig> = {}): AppConfig {
  return {
    initialized: true,
    archiveMode: 'Encrypted',
    preferredLanguage: 'en',
    dueAfterHours: 24,
    scheduleCheckIntervalHours: 1,
    checkpointDays: 7,
    captureFavicons: true,
    selectedProfileIds: [],
    gitEnabled: false,
    rememberDatabaseKeyInKeyring: false,
    appAutostart: false,
    explorerBackgroundPrefetchPages: 2,
    appLock: {
      enabled: false,
      idleTimeoutMinutes: 15,
      biometricEnabled: false,
    },
    enrichment: { enabled: false, mode: 'manual' },
    deterministic: { deterministicMode: false },
    ai: {
      baseUrl: '',
      embeddingModel: '',
      llmModel: '',
      apiKey: null,
      embeddingEnabled: false,
    },
    ...overrides,
  } as AppConfig
}

function snapshotFixture(
  keyring: Partial<KeyringStatusReport> = {},
  config: Partial<AppConfig> = {},
): AppSnapshot {
  return {
    config: configFixture(config),
    archiveStatus: {
      initialized: true,
      encrypted: true,
      unlocked: true,
      warning: null,
    },
    keyringStatus: keyringFixture(keyring),
    browserProfiles: [],
  } as unknown as AppSnapshot
}

function renderSection(
  snapshot: AppSnapshot | null,
  shellOverrides: Partial<ShellDataContextValue> = {},
) {
  const saveConfig = vi.fn().mockResolvedValue({})
  const shell: ShellDataContextValue = {
    buildInfo: null,
    appLockStatus: null,
    snapshot,
    dashboard: null,
    loading: false,
    busyAction: null,
    busyOverlay: null,
    error: null,
    rawError: null,
    notice: null,
    refreshKey: 0,
    errorKind: null,
    refreshAppData: vi.fn().mockResolvedValue(undefined),
    refreshRuntimeStatus: vi.fn().mockResolvedValue({}),
    saveConfig,
    initializeArchive: vi.fn(),
    runBackup: vi.fn().mockResolvedValue({}),
    setAppLockPasscode: vi.fn(),
    clearAppLockPasscode: vi.fn(),
    lockAppSession: vi.fn().mockResolvedValue({}),
    unlockAppSession: vi.fn().mockResolvedValue({}),
    clearNotice: vi.fn(),
    clearError: vi.fn(),
    ...shellOverrides,
    // Always use the saveConfig above unless overridden explicitly.
    ...(shellOverrides.saveConfig ? {} : {}),
  } as ShellDataContextValue
  // Allow per-test overrides of saveConfig.
  const finalSaveConfig = (shellOverrides as Record<string, unknown>)
    .saveConfig as typeof saveConfig | undefined
  const contextValue = {
    ...shell,
    saveConfig: finalSaveConfig ?? saveConfig,
  } as ShellDataContextValue

  render(
    <I18nProvider>
      <ShellDataContext.Provider value={contextValue}>
        <SecuritySection navItem={navItem} />
      </ShellDataContext.Provider>
    </I18nProvider>,
  )

  return { saveConfig: finalSaveConfig ?? saveConfig }
}

// ──────────────────────────────────────────────────────────────────────────────
// Reset mocks between tests
// ──────────────────────────────────────────────────────────────────────────────
beforeEach(() => {
  vi.mocked(backend.keyringClearDatabaseKey).mockResolvedValue({
    available: true,
    backend: 'macOS Keychain',
    storedSecret: false,
  })
})

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────
describe('SecuritySection', () => {
  test('renders the section card', () => {
    renderSection(snapshotFixture())
    expect(screen.getByTestId('settings-security-section')).toBeInTheDocument()
  })

  test('toggle is OFF when rememberDatabaseKeyInKeyring=false', () => {
    renderSection(snapshotFixture({}, { rememberDatabaseKeyInKeyring: false }))
    const toggle = screen.getByTestId('keychain-remember-toggle')
    expect(toggle).toHaveAttribute('aria-checked', 'false')
  })

  test('toggle is ON when rememberDatabaseKeyInKeyring=true and keychain available', () => {
    renderSection(
      snapshotFixture(
        { available: true },
        { rememberDatabaseKeyInKeyring: true },
      ),
    )
    const toggle = screen.getByTestId('keychain-remember-toggle')
    expect(toggle).toHaveAttribute('aria-checked', 'true')
  })

  test('toggle is OFF when keychain unavailable (even if config flag is true)', () => {
    renderSection(
      snapshotFixture(
        { available: false },
        { rememberDatabaseKeyInKeyring: true },
      ),
    )
    const toggle = screen.getByTestId('keychain-remember-toggle')
    expect(toggle).toHaveAttribute('aria-checked', 'false')
  })

  test('shows "Not stored" status when keyring available but no secret stored', () => {
    renderSection(snapshotFixture({ available: true, storedSecret: false }))
    expect(screen.getByText(/not stored/i)).toBeInTheDocument()
  })

  test('shows "Stored in" status when secret is stored', () => {
    renderSection(
      snapshotFixture({
        available: true,
        storedSecret: true,
        backend: 'macOS Keychain',
      }),
    )
    expect(screen.getByText(/stored in macOS Keychain/i)).toBeInTheDocument()
  })

  test('shows unavailable status when keychain unavailable', () => {
    renderSection(snapshotFixture({ available: false }))
    // The status text appears twice (also as toggle offLabel) — check at least one.
    expect(
      screen.getAllByText(/system keychain unavailable/i).length,
    ).toBeGreaterThanOrEqual(1)
  })

  test('toggling ON: saves config with rememberDatabaseKeyInKeyring=true and flashes Saved', async () => {
    const saveConfig = vi.fn().mockResolvedValue({})
    renderSection(
      snapshotFixture(
        { available: true },
        { rememberDatabaseKeyInKeyring: false },
      ),
      { saveConfig },
    )

    fireEvent.click(screen.getByTestId('keychain-remember-toggle'))

    await waitFor(() =>
      expect(saveConfig).toHaveBeenCalledWith(
        expect.objectContaining({ rememberDatabaseKeyInKeyring: true }),
        { quiet: true },
      ),
    )
    expect(await screen.findByText('Saved')).toBeInTheDocument()
  })

  test('toggling OFF: clears keyring + saves config with rememberDatabaseKeyInKeyring=false', async () => {
    const saveConfig = vi.fn().mockResolvedValue({})
    renderSection(
      snapshotFixture(
        { available: true, storedSecret: true },
        { rememberDatabaseKeyInKeyring: true },
      ),
      { saveConfig },
    )

    fireEvent.click(screen.getByTestId('keychain-remember-toggle'))

    await waitFor(() =>
      expect(backend.keyringClearDatabaseKey).toHaveBeenCalled(),
    )
    await waitFor(() =>
      expect(saveConfig).toHaveBeenCalledWith(
        expect.objectContaining({ rememberDatabaseKeyInKeyring: false }),
        { quiet: true },
      ),
    )
    expect(await screen.findByText('Saved')).toBeInTheDocument()
  })

  test('toggling OFF still persists the opt-out when clearing the keychain fails', async () => {
    // The keychain clear is best-effort: a rejection is swallowed so the
    // remember=false flag still persists (no stuck "remembered" state).
    vi.mocked(backend.keyringClearDatabaseKey).mockRejectedValueOnce(
      new Error('keychain clear boom'),
    )
    const saveConfig = vi.fn().mockResolvedValue({})
    renderSection(
      snapshotFixture(
        { available: true, storedSecret: true },
        { rememberDatabaseKeyInKeyring: true },
      ),
      { saveConfig },
    )

    fireEvent.click(screen.getByTestId('keychain-remember-toggle'))

    await waitFor(() =>
      expect(saveConfig).toHaveBeenCalledWith(
        expect.objectContaining({ rememberDatabaseKeyInKeyring: false }),
        { quiet: true },
      ),
    )
    expect(await screen.findByText('Saved')).toBeInTheDocument()
  })

  test('does not call keyringClearDatabaseKey when toggling ON', async () => {
    const saveConfig = vi.fn().mockResolvedValue({})
    renderSection(
      snapshotFixture(
        { available: true },
        { rememberDatabaseKeyInKeyring: false },
      ),
      { saveConfig },
    )

    fireEvent.click(screen.getByTestId('keychain-remember-toggle'))

    await waitFor(() => expect(saveConfig).toHaveBeenCalled())
    expect(backend.keyringClearDatabaseKey).not.toHaveBeenCalled()
  })

  test('M2: toggle is aria-disabled and a no-op when keychain unavailable — no save, no flash', async () => {
    const saveConfig = vi.fn().mockResolvedValue({})
    renderSection(snapshotFixture({ available: false }), { saveConfig })

    const toggle = screen.getByTestId('keychain-remember-toggle')
    // Honest disabled state, not a switch that snaps back after a fake "Saved".
    expect(toggle).toHaveAttribute('aria-disabled', 'true')

    // The switch stays clickable (aria-disabled, not native disabled), so the
    // click reaches handleToggle — which must bail without saving or flashing.
    fireEvent.click(toggle)

    // Give any (incorrect) async save a chance to run before asserting absence.
    await Promise.resolve()
    expect(saveConfig).not.toHaveBeenCalled()
    expect(backend.keyringClearDatabaseKey).not.toHaveBeenCalled()
    expect(screen.queryByText('Saved')).toBeNull()
  })

  test('renders with safe fallbacks and a no-op toggle when snapshot is null', async () => {
    const saveConfig = vi.fn().mockResolvedValue({})
    renderSection(null, { saveConfig })

    // Fallbacks: no snapshot → unavailable status + aria-disabled toggle.
    expect(screen.getByTestId('settings-security-section')).toBeInTheDocument()
    const toggle = screen.getByTestId('keychain-remember-toggle')
    expect(toggle).toHaveAttribute('aria-checked', 'false')
    expect(toggle).toHaveAttribute('aria-disabled', 'true')

    // Clicking exercises handleToggle's `!snapshot` guard (early return).
    fireEvent.click(toggle)
    await Promise.resolve()
    expect(saveConfig).not.toHaveBeenCalled()
  })
})
