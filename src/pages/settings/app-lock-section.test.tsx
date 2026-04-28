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

  test('wires draft controls and actions to route-owned handlers', () => {
    const handlers = handlerFixture()
    renderSection({
      ...handlers,
      canEnable: true,
      configDirty: true,
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

    fireEvent.click(screen.getByLabelText('Enable App Lock'))
    fireEvent.change(screen.getByLabelText('Idle timeout'), {
      target: { value: '30' },
    })
    fireEvent.click(
      screen.getByLabelText('Allow Touch ID unlock when available'),
    )
    fireEvent.change(screen.getByLabelText('Recovery hint'), {
      target: { value: 'new hint' },
    })
    fireEvent.change(screen.getByLabelText('Passcode'), {
      target: { value: '5678' },
    })
    fireEvent.click(
      screen.getByRole('button', { name: 'Save app lock settings' }),
    )
    fireEvent.click(screen.getByRole('button', { name: 'Update passcode' }))
    fireEvent.click(screen.getByRole('button', { name: 'Clear passcode' }))
    fireEvent.click(screen.getByRole('button', { name: 'Lock now' }))
    fireEvent.click(screen.getByRole('button', { name: 'Copy' }))

    expect(handlers.onEnabledChange).toHaveBeenCalledWith(false)
    expect(handlers.onIdleTimeoutChange).toHaveBeenCalledWith(30)
    expect(handlers.onBiometricChange).toHaveBeenCalledWith(true)
    expect(handlers.onRecoveryHintChange).toHaveBeenCalledWith('new hint')
    expect(handlers.onPasscodeChange).toHaveBeenCalledWith('5678')
    expect(handlers.onSaveConfig).toHaveBeenCalledTimes(1)
    expect(handlers.onSetPasscode).toHaveBeenCalledTimes(1)
    expect(handlers.onClearPasscode).toHaveBeenCalledTimes(1)
    expect(handlers.onLockNow).toHaveBeenCalledTimes(1)
    expect(handlers.onCopyPath).toHaveBeenCalledWith(
      'settings:app-lock-config',
      '/tmp/pathkeep/app-lock.json',
    )
    expect(screen.getByText('Locked')).toBeInTheDocument()
  })

  test('shows disabled and degraded states without inventing fallback settings', () => {
    renderSection({
      action: 'Saving...',
      canEnable: false,
      configDirty: true,
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

    expect(
      screen.getByLabelText('Allow biometric unlock when available'),
    ).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Saving...' })).toBeDisabled()
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
    configDirty: false,
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
    onBiometricChange: vi.fn(),
    onClearPasscode: vi.fn().mockResolvedValue(undefined),
    onCopyPath: vi.fn().mockResolvedValue(undefined),
    onEnabledChange: vi.fn(),
    onIdleTimeoutChange: vi.fn(),
    onLockNow: vi.fn().mockResolvedValue(undefined),
    onOpenPath: vi.fn(),
    onPasscodeChange: vi.fn(),
    onRecoveryHintChange: vi.fn(),
    onSaveConfig: vi.fn().mockResolvedValue(undefined),
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
