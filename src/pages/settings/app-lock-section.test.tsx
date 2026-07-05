import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import type { AppLockConfig, AppLockStatus } from '../../lib/types'
import { AppLockSection, type AppLockSectionState } from './app-lock-section'
import type { SettingsSectionNavItem } from './section-nav-items'

const navItem: SettingsSectionNavItem = {
  id: 'settings-app-lock',
  icon: 'shield',
  key: 'applock',
  label: 'App Lock',
}

describe('AppLockSection', () => {
  test('does not render before the app-lock draft hydrates', () => {
    const { container } = renderSection({
      currentSettings: null,
    })

    expect(container.firstChild).toBeNull()
  })

  test('auto-saves field edits and keeps passcode/lock-now explicit (no Save button)', async () => {
    const handlers = handlerFixture()
    renderSection({
      ...handlers,
      canEnable: true,
      currentSettings: configFixture({
        biometricEnabled: false,
        enabled: true,
        idleTimeoutMinutes: 15,
      }),
      passcode: '1234',
      recoveryHint: 'phrase',
      status: statusFixture({
        biometricAvailable: true,
        enabled: true,
        locked: true,
        passcodeConfigured: true,
      }),
      usesTouchId: true,
    })

    // The all-auto-save model removes the per-section Save button entirely.
    expect(
      screen.queryByRole('button', { name: 'Save app lock settings' }),
    ).toBeNull()

    // Toggling / selecting a field persists immediately via its save handler.
    fireEvent.click(screen.getByLabelText('Enable App Lock'))
    expect(handlers.onEnabledChange).toHaveBeenCalledWith(false)
    fireEvent.change(screen.getByLabelText('Idle timeout'), {
      target: { value: '30' },
    })
    expect(handlers.onIdleTimeoutChange).toHaveBeenCalledWith(30)
    fireEvent.click(
      screen.getByLabelText('Allow Touch ID unlock when available'),
    )
    expect(handlers.onBiometricChange).toHaveBeenCalledWith(true)

    // Recovery hint edits the draft on change and commits on blur (auto-save).
    fireEvent.change(screen.getByLabelText('Recovery hint'), {
      target: { value: 'new hint' },
    })
    expect(handlers.onRecoveryHintChange).toHaveBeenCalledWith('new hint')
    expect(handlers.onRecoveryHintCommit).not.toHaveBeenCalled()
    fireEvent.blur(screen.getByLabelText('Recovery hint'))
    expect(handlers.onRecoveryHintCommit).toHaveBeenCalledTimes(1)

    // The quiet "Saved" chip flashes after a successful auto-save.
    expect(await screen.findByText('Saved')).toBeInTheDocument()

    // Passcode set/clear + lock-now stay explicit action buttons.
    fireEvent.change(screen.getByLabelText('Passcode'), {
      target: { value: '5678' },
    })
    expect(handlers.onPasscodeChange).toHaveBeenCalledWith('5678')
    fireEvent.click(screen.getByRole('button', { name: 'Update passcode' }))
    fireEvent.click(screen.getByRole('button', { name: 'Clear passcode' }))
    fireEvent.click(screen.getByRole('button', { name: 'Lock now' }))
    fireEvent.click(screen.getByRole('button', { name: 'Copy' }))

    expect(handlers.onSetPasscode).toHaveBeenCalledTimes(1)
    expect(handlers.onClearPasscode).toHaveBeenCalledTimes(1)
    expect(handlers.onLockNow).toHaveBeenCalledTimes(1)
    expect(handlers.onCopyPath).toHaveBeenCalledWith(
      'settings:app-lock-config',
      '/tmp/pathkeep/app-lock.json',
    )
    expect(screen.getByText('Locked')).toBeInTheDocument()
  })

  test('does not flash the Saved chip when an auto-save is a no-op (resolves false)', async () => {
    const handlers = handlerFixture()
    handlers.onEnabledChange.mockResolvedValue(false)
    renderSection({
      ...handlers,
      currentSettings: configFixture({ enabled: false }),
    })

    fireEvent.click(screen.getByLabelText('Enable App Lock'))
    expect(handlers.onEnabledChange).toHaveBeenCalledTimes(1)
    await Promise.resolve()
    expect(
      screen.getByTestId('settings-saved-chip').getAttribute('data-visible'),
    ).toBe('false')
  })

  test('keeps the Saved chip hidden and swallows the rejection when an auto-save fails', async () => {
    // persistAppLock re-throws on a failed saveConfig (the shell already set the
    // error banner). flashOnSave must swallow that rejection so there is no
    // unhandled rejection on every failing toggle, and the chip must stay hidden.
    const handlers = handlerFixture()
    handlers.onEnabledChange.mockRejectedValue(new Error('save failed'))
    const unhandled = vi.fn()
    process.on('unhandledRejection', unhandled)
    try {
      renderSection({
        ...handlers,
        currentSettings: configFixture({ enabled: false }),
      })

      fireEvent.click(screen.getByLabelText('Enable App Lock'))
      expect(handlers.onEnabledChange).toHaveBeenCalledTimes(1)
      await Promise.resolve()
      await Promise.resolve()
      expect(
        screen.getByTestId('settings-saved-chip').getAttribute('data-visible'),
      ).toBe('false')
      expect(unhandled).not.toHaveBeenCalled()
    } finally {
      process.off('unhandledRejection', unhandled)
    }
  })

  test('shows disabled and degraded states without inventing fallback settings', () => {
    renderSection({
      action: 'Saving...',
      canEnable: false,
      currentSettings: configFixture({
        enabled: true,
        passcodeConfigured: false,
      }),
      passcode: '12',
      status: statusFixture({
        biometricAvailable: false,
        biometricState: 'unsupported',
        configPath: '',
        degradationNotes: [
          'App Lock only protects the PathKeep UI session. Archive encryption still protects data at rest.',
          'Custom backend warning',
        ],
        enabled: false,
        lastUnlockedAt: null,
        passcodeConfigured: false,
      }),
      usesTouchId: false,
    })

    // There is no per-section Save button in the all-auto-save model.
    expect(
      screen.queryByRole('button', { name: 'Save app lock settings' }),
    ).toBeNull()
    // An in-flight action freezes the auto-save fields and the explicit actions.
    expect(
      screen.getByLabelText('Allow biometric unlock when available'),
    ).toBeDisabled()
    expect(screen.getByLabelText('Enable App Lock')).toBeDisabled()
    expect(screen.getByLabelText('Idle timeout')).toBeDisabled()
    expect(screen.getByLabelText('Recovery hint')).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Save passcode' })).toBeDisabled()
    expect(
      screen.getByRole('button', { name: 'Clear passcode' }),
    ).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Lock now' })).toBeDisabled()
    expect(screen.getByText('Custom backend warning')).toBeInTheDocument()
    expect(screen.getAllByText('Not available')).toHaveLength(2)
  })

  test('renders Touch ID unavailable and localized Touch ID availability notes', () => {
    renderSection({
      currentSettings: configFixture({
        biometricEnabled: true,
        enabled: false,
      }),
      status: statusFixture({
        biometricAvailable: false,
        biometricState: 'touch-id-unavailable',
        degradationNotes: [
          'Touch ID is available on this Mac and can unlock the current PathKeep session.',
        ],
      }),
      usesTouchId: true,
    })

    expect(
      screen.getByLabelText('Allow Touch ID unlock when available'),
    ).toBeDisabled()
    expect(
      screen.getByText(
        'Touch ID is unavailable on this Mac right now, so passcode unlock stays required.',
      ),
    ).toBeVisible()
    expect(
      screen.getByText(
        'Touch ID is available on this Mac and can unlock the current PathKeep session.',
      ),
    ).toBeVisible()
  })
})

function renderSection(overrides: Partial<AppLockSectionState> = {}) {
  const state: AppLockSectionState = {
    action: null,
    canEnable: true,
    copyFeedback: null,
    currentSettings: configFixture(),
    passcode: '',
    recoveryHint: '',
    status: statusFixture(),
    usesTouchId: false,
    ...handlerFixture(),
    ...overrides,
  }

  return render(
    <I18nProvider>
      <AppLockSection navItem={navItem} state={state} />
    </I18nProvider>,
  )
}

function handlerFixture() {
  return {
    onBiometricChange: vi.fn().mockResolvedValue(true),
    onClearPasscode: vi.fn().mockResolvedValue(undefined),
    onCopyPath: vi.fn().mockResolvedValue(undefined),
    onEnabledChange: vi.fn().mockResolvedValue(true),
    onIdleTimeoutChange: vi.fn().mockResolvedValue(true),
    onLockNow: vi.fn().mockResolvedValue(undefined),
    onOpenPath: vi.fn(),
    onPasscodeChange: vi.fn(),
    onRecoveryHintChange: vi.fn(),
    onRecoveryHintCommit: vi.fn().mockResolvedValue(true),
    onSetPasscode: vi.fn().mockResolvedValue(undefined),
  }
}

function configFixture(overrides: Partial<AppLockConfig> = {}): AppLockConfig {
  return {
    biometricEnabled: false,
    enabled: false,
    idleTimeoutMinutes: 5,
    passcodeConfigured: false,
    passcodeEnabled: true,
    recoveryHint: null,
    ...overrides,
  }
}

function statusFixture(overrides: Partial<AppLockStatus> = {}): AppLockStatus {
  return {
    biometricAvailable: true,
    biometricEnabled: false,
    biometricState: 'touch-id-available',
    configPath: '/tmp/pathkeep/app-lock.json',
    degradationNotes: [],
    enabled: false,
    idleTimeoutMinutes: 5,
    lastUnlockedAt: '2026-04-25T12:00:00Z',
    locked: false,
    lockedAt: null,
    lockReason: null,
    passcodeConfigured: false,
    passcodeEnabled: true,
    recoveryHint: null,
    warnings: [],
    ...overrides,
  }
}
