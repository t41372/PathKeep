import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, useLocation } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import { mockBuildInfo, mockSnapshot } from '../../lib/backend-preview-fixtures'
import { I18nProvider } from '../../lib/i18n'
import type { AppConfig, AppSnapshot, SchedulePlan } from '../../lib/types'
import { OnboardingPage } from './index'

const shellData = vi.hoisted(() => ({
  current: {} as Record<string, unknown>,
}))

vi.mock('../../app/shell-data-context', () => ({
  useShellData: () => shellData.current,
}))

vi.mock('./welcome-step', () => ({
  WelcomeStep: ({ onBegin }: { onBegin: () => void }) => (
    <button type="button" onClick={onBegin}>
      begin
    </button>
  ),
}))

vi.mock('./browser-detection-step', () => ({
  BrowserDetectionStep: ({
    localError,
    onBack,
    onContinue,
    onOpenFullDiskAccessSettings,
    onToggleProfile,
  }: {
    localError: string | null
    onBack: () => void
    onContinue: () => void
    onOpenFullDiskAccessSettings: () => void
    onToggleProfile: (profileId: string) => void
  }) => (
    <section>
      <p>{localError}</p>
      <button type="button" onClick={onBack}>
        browser-back
      </button>
      <button type="button" onClick={onContinue}>
        browser-continue
      </button>
      <button type="button" onClick={onOpenFullDiskAccessSettings}>
        open-full-disk
      </button>
      <button type="button" onClick={() => onToggleProfile('chrome:Default')}>
        toggle-profile
      </button>
    </section>
  ),
}))

vi.mock('./storage-step', () => ({
  StorageStep: ({
    onBack,
    onContinue,
  }: {
    onBack: () => void
    onContinue: () => void
  }) => (
    <section>
      <button type="button" onClick={onBack}>
        storage-back
      </button>
      <button type="button" onClick={onContinue}>
        storage-continue
      </button>
    </section>
  ),
}))

vi.mock('./security-step', () => ({
  SecurityStep: ({
    archiveMode,
    localError,
    onBack,
    onContinue,
    onSecurityCardClick,
    onSelectArchiveMode,
    onUpdateSecurityDraft,
  }: {
    archiveMode: AppConfig['archiveMode']
    localError: string | null
    onBack: () => void
    onContinue: () => void
    onSecurityCardClick: (
      mode: AppConfig['archiveMode'],
      target: EventTarget | null,
    ) => void
    onSelectArchiveMode: (mode: AppConfig['archiveMode']) => void
    onUpdateSecurityDraft: (next: {
      confirmPassword?: string
      masterPassword?: string
      rememberKey?: boolean
    }) => void
  }) => (
    <section>
      <p>mode:{archiveMode}</p>
      <p>{localError}</p>
      <button type="button" onClick={onBack}>
        security-back
      </button>
      <button type="button" onClick={onContinue}>
        security-continue
      </button>
      <button
        type="button"
        onClick={(event) =>
          onSecurityCardClick('Plaintext', event.currentTarget)
        }
      >
        security-card-interactive
      </button>
      <button
        type="button"
        onClick={() => {
          const target = document.createElement('div')
          onSecurityCardClick('Plaintext', target)
        }}
      >
        security-card-plain
      </button>
      <button
        type="button"
        onClick={() => onSecurityCardClick('Plaintext', null)}
      >
        security-card-null-target
      </button>
      <button type="button" onClick={() => onSelectArchiveMode('Encrypted')}>
        security-select-encrypted
      </button>
      <button
        type="button"
        onClick={() =>
          onUpdateSecurityDraft({
            confirmPassword: 'mismatch',
            masterPassword: 'secret',
          })
        }
      >
        security-password-mismatch
      </button>
      <button
        type="button"
        onClick={() =>
          onUpdateSecurityDraft({
            confirmPassword: 'secret',
            masterPassword: 'secret',
            rememberKey: true,
          })
        }
      >
        security-password-match
      </button>
      <button
        type="button"
        onClick={() =>
          onUpdateSecurityDraft({
            confirmPassword: 'secret',
            masterPassword: 'secret',
            rememberKey: false,
          })
        }
      >
        security-password-match-no-remember
      </button>
    </section>
  ),
}))

vi.mock('./schedule-step', () => ({
  ScheduleStep: ({
    schedulePlan,
    schedulePreviewError,
    schedulePreviewLoading,
    onBack,
    onInstallSchedule,
    onSelectDueAfterHours,
    onSkipSchedule,
  }: {
    schedulePlan: SchedulePlan | null
    schedulePreviewError: string | null
    schedulePreviewLoading: boolean
    onBack: () => void
    onInstallSchedule: () => void
    onSelectDueAfterHours: (hours: number) => void
    onSkipSchedule: () => void
  }) => (
    <section>
      <p>{schedulePreviewLoading ? 'schedule-loading' : 'schedule-idle'}</p>
      <p>{schedulePreviewError}</p>
      <p>{schedulePlan?.label}</p>
      <button type="button" onClick={onBack}>
        schedule-back
      </button>
      <button type="button" onClick={onInstallSchedule}>
        schedule-install
      </button>
      <button type="button" onClick={onSkipSchedule}>
        schedule-skip
      </button>
      <button type="button" onClick={() => onSelectDueAfterHours(12)}>
        select-12-hours
      </button>
    </section>
  ),
}))

vi.mock('./ai-step', () => ({
  AiStep: ({
    onBack,
    onSetUpAi,
    onSkip,
  }: {
    onBack: () => void
    onSetUpAi: () => void
    onSkip: () => void
  }) => (
    <section>
      <button type="button" onClick={onBack}>
        ai-back
      </button>
      <button type="button" onClick={onSetUpAi}>
        ai-setup
      </button>
      <button type="button" onClick={onSkip}>
        ai-skip
      </button>
    </section>
  ),
}))

vi.mock('./ready-step', () => ({
  ReadyStep: ({
    localError,
    onBack,
    onFinish,
    onOpenFullDiskAccessSettings,
  }: {
    localError: string | null
    onBack: () => void
    onFinish: () => void
    onOpenFullDiskAccessSettings: () => void
  }) => (
    <section>
      <p>{localError}</p>
      <button type="button" onClick={onBack}>
        ready-back
      </button>
      <button type="button" onClick={onFinish}>
        finish
      </button>
      <button type="button" onClick={onOpenFullDiskAccessSettings}>
        ready-open-full-disk
      </button>
    </section>
  ),
}))

describe('OnboardingPage', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    shellData.current = shellDataFixture()
    vi.spyOn(backend, 'previewSchedule').mockResolvedValue(
      schedulePlanFixture(),
    )
    vi.spyOn(backend, 'scheduleStatus').mockResolvedValue(
      scheduleStatusFixture(),
    )
    vi.spyOn(backend, 'applySchedule').mockResolvedValue({
      applied: true,
      files: [],
      message: 'installed',
      platform: 'macos',
    })
    vi.spyOn(backend, 'openExternalUrl').mockResolvedValue('opened')
    vi.spyOn(backend, 'keyringStoreDatabaseKey').mockResolvedValue(
      keyringStatusFixture(),
    )
  })

  test('renders loading, error, and empty gates before snapshot hydration', () => {
    shellData.current = shellDataFixture({
      loading: true,
      snapshot: null,
    })
    const { rerender } = renderPage()
    expect(screen.getByTestId('onboarding-page')).toHaveTextContent(
      'Loading setup…',
    )

    shellData.current = shellDataFixture({
      error: 'bootstrap failed',
      snapshot: null,
    })
    rerender(pageElement())
    expect(screen.getByText('bootstrap failed')).toBeInTheDocument()

    shellData.current = shellDataFixture({
      snapshot: null,
    })
    rerender(pageElement())
    expect(screen.getByText('Getting things ready…')).toBeInTheDocument()
  })

  test('drives step navigation, schedule preview, profile toggles, and security validation', async () => {
    const user = userEvent.setup()
    const saveConfig = vi.fn((config: AppConfig) =>
      Promise.resolve({
        ...snapshotFixture(),
        config,
      }),
    )
    shellData.current = shellDataFixture({ saveConfig })
    renderPage()

    await user.click(screen.getByRole('button', { name: 'begin' }))
    await user.click(screen.getByRole('button', { name: 'browser-back' }))
    expect(screen.getByRole('button', { name: 'begin' })).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'begin' }))
    await user.click(screen.getByRole('button', { name: 'toggle-profile' }))
    expect(saveConfig).toHaveBeenLastCalledWith(
      expect.objectContaining({
        selectedProfileIds: expect.not.arrayContaining(['chrome:Default']),
      }),
    )

    await user.click(screen.getByRole('button', { name: 'browser-continue' }))
    await user.click(screen.getByRole('button', { name: 'storage-back' }))
    expect(
      screen.getByRole('button', { name: 'browser-continue' }),
    ).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'browser-continue' }))
    await user.click(screen.getByRole('button', { name: 'storage-continue' }))
    await user.click(
      screen.getByRole('button', { name: 'security-card-interactive' }),
    )
    await user.click(
      screen.getByRole('button', { name: 'security-card-plain' }),
    )
    await user.click(
      screen.getByRole('button', { name: 'security-card-null-target' }),
    )
    expect(saveConfig).toHaveBeenLastCalledWith(
      expect.objectContaining({ archiveMode: 'Plaintext' }),
    )

    await user.click(
      screen.getByRole('button', { name: 'security-select-encrypted' }),
    )
    await user.click(screen.getByRole('button', { name: 'security-back' }))
    expect(
      screen.getByRole('button', { name: 'storage-continue' }),
    ).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'storage-continue' }))
    await user.click(
      screen.getByRole('button', { name: 'security-select-encrypted' }),
    )
    await user.click(
      screen.getByRole('button', { name: 'security-password-mismatch' }),
    )
    await user.click(screen.getByRole('button', { name: 'security-continue' }))
    expect(
      screen.getByText("Passwords don't match. Try again."),
    ).toBeInTheDocument()

    await user.click(
      screen.getByRole('button', { name: 'security-password-match' }),
    )
    await user.click(screen.getByRole('button', { name: 'security-continue' }))
    expect(backend.previewSchedule).toHaveBeenCalledTimes(1)
    await screen.findByText('PathKeep backup')

    await user.click(screen.getByRole('button', { name: 'select-12-hours' }))
    expect(saveConfig).toHaveBeenLastCalledWith(
      expect.objectContaining({ dueAfterHours: 12 }),
    )
    await user.click(screen.getByRole('button', { name: 'Schedule' }))
    expect(
      screen.getByRole('button', { name: 'schedule-install' }),
    ).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Storage ✓' }))
    expect(
      screen.getByRole('button', { name: 'storage-continue' }),
    ).toBeInTheDocument()
  })

  test('blocks finish when ready-state browser selection or encrypted password state becomes invalid', async () => {
    const user = userEvent.setup()
    shellData.current = shellDataFixture({
      snapshot: snapshotFixture({ archiveMode: 'Plaintext' }),
    })
    const { rerender } = renderPage()

    await user.click(screen.getByRole('button', { name: 'begin' }))
    await user.click(screen.getByRole('button', { name: 'browser-continue' }))
    await user.click(screen.getByRole('button', { name: 'storage-continue' }))
    await user.click(screen.getByRole('button', { name: 'security-continue' }))
    await waitFor(() => expect(backend.previewSchedule).toHaveBeenCalled())
    await user.click(screen.getByRole('button', { name: 'schedule-install' }))
    await user.click(screen.getByRole('button', { name: 'ai-skip' }))

    // Ready → Back lands on the optional AI step (the route owner wires ReadyStep.onBack to it).
    await user.click(screen.getByRole('button', { name: 'ready-back' }))
    expect(screen.getByRole('button', { name: 'ai-skip' })).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'ai-skip' }))

    shellData.current = shellDataFixture({
      snapshot: snapshotFixture({
        archiveMode: 'Encrypted',
        selectedProfileIds: [],
      }),
    })
    rerender(pageElement())
    await user.click(screen.getByRole('button', { name: 'finish' }))
    expect(
      screen.getByText('Pick at least one browser profile to back up.'),
    ).toBeInTheDocument()

    shellData.current = shellDataFixture({
      snapshot: snapshotWithBlockedProfile({
        archiveMode: 'Encrypted',
        selectedProfileIds: ['safari:Blocked'],
      }),
    })
    rerender(pageElement())
    await user.click(screen.getByRole('button', { name: 'finish' }))
    expect(
      screen.getByText(
        'The selected browser profiles are not readable yet. Grant access first, or go back and choose a readable source.',
      ),
    ).toBeInTheDocument()

    shellData.current = shellDataFixture({
      snapshot: snapshotFixture({ archiveMode: 'Encrypted' }),
    })
    rerender(pageElement())
    await user.click(screen.getByRole('button', { name: 'finish' }))
    expect(
      screen.getByText('Enter a master password to use encrypted mode.'),
    ).toBeInTheDocument()
  })

  test('revalidates mismatched encrypted passwords again at finish time', async () => {
    const user = userEvent.setup()
    const { rerender } = renderPage()

    await user.click(screen.getByRole('button', { name: 'begin' }))
    await user.click(screen.getByRole('button', { name: 'browser-continue' }))
    await user.click(screen.getByRole('button', { name: 'storage-continue' }))
    await user.click(
      screen.getByRole('button', { name: 'security-password-mismatch' }),
    )

    shellData.current = shellDataFixture({
      snapshot: snapshotFixture({ archiveMode: 'Plaintext' }),
    })
    rerender(pageElement())
    await user.click(screen.getByRole('button', { name: 'security-continue' }))
    await waitFor(() => expect(backend.previewSchedule).toHaveBeenCalled())
    await user.click(screen.getByRole('button', { name: 'schedule-install' }))
    await user.click(screen.getByRole('button', { name: 'ai-skip' }))

    shellData.current = shellDataFixture({
      snapshot: snapshotFixture({ archiveMode: 'Encrypted' }),
    })
    rerender(pageElement())
    await user.click(screen.getByRole('button', { name: 'finish' }))

    expect(
      screen.getByText("Passwords don't match. Try again."),
    ).toBeInTheDocument()
  })

  test('surfaces schedule preview and Full Disk Access failures', async () => {
    const user = userEvent.setup()
    vi.spyOn(backend, 'previewSchedule').mockRejectedValueOnce(
      new Error('scheduler failed'),
    )
    vi.spyOn(backend, 'openExternalUrl').mockRejectedValueOnce(
      new Error('cannot open settings'),
    )
    renderPage()

    await user.click(screen.getByRole('button', { name: 'begin' }))
    await user.click(screen.getByRole('button', { name: 'open-full-disk' }))
    expect(
      await screen.findByText(
        'Could not open System Settings. Go to System Settings → Privacy & Security → Full Disk Access manually.',
      ),
    ).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'browser-continue' }))
    await user.click(screen.getByRole('button', { name: 'storage-continue' }))
    await user.click(
      screen.getByRole('button', { name: 'security-password-match' }),
    )
    await user.click(screen.getByRole('button', { name: 'security-continue' }))
    expect(await screen.findByText('scheduler failed')).toBeInTheDocument()
  })

  test('uses the fallback copy for non-Error schedule preview failures', async () => {
    const user = userEvent.setup()
    vi.spyOn(backend, 'previewSchedule').mockRejectedValueOnce(
      'scheduler fallback',
    )
    renderPage()

    await user.click(screen.getByRole('button', { name: 'begin' }))
    await user.click(screen.getByRole('button', { name: 'browser-continue' }))
    await user.click(screen.getByRole('button', { name: 'storage-continue' }))
    await user.click(
      screen.getByRole('button', { name: 'security-password-match' }),
    )
    await user.click(screen.getByRole('button', { name: 'security-continue' }))

    expect(await screen.findByText('scheduler fallback')).toBeInTheDocument()
  })

  test('ignores schedule preview success and failure after leaving the schedule step', async () => {
    const user = userEvent.setup()
    const successPreview = deferred<SchedulePlan>()
    vi.spyOn(backend, 'previewSchedule').mockReturnValueOnce(
      successPreview.promise,
    )
    const successRender = renderPage()

    await user.click(screen.getByRole('button', { name: 'begin' }))
    await user.click(screen.getByRole('button', { name: 'browser-continue' }))
    await user.click(screen.getByRole('button', { name: 'storage-continue' }))
    await user.click(
      screen.getByRole('button', { name: 'security-password-match' }),
    )
    await user.click(screen.getByRole('button', { name: 'security-continue' }))
    await waitFor(() => expect(backend.previewSchedule).toHaveBeenCalled())
    successRender.unmount()
    successPreview.resolve(schedulePlanFixture())
    await successPreview.promise

    vi.restoreAllMocks()
    vi.spyOn(backend, 'openExternalUrl').mockResolvedValue('opened')
    vi.spyOn(backend, 'keyringStoreDatabaseKey').mockResolvedValue(
      keyringStatusFixture(),
    )
    vi.spyOn(backend, 'scheduleStatus').mockResolvedValue(
      scheduleStatusFixture(),
    )
    const failedPreview = deferred<SchedulePlan>()
    vi.spyOn(backend, 'previewSchedule').mockReturnValueOnce(
      failedPreview.promise,
    )
    const failureRender = renderPage()

    await user.click(screen.getByRole('button', { name: 'begin' }))
    await user.click(screen.getByRole('button', { name: 'browser-continue' }))
    await user.click(screen.getByRole('button', { name: 'storage-continue' }))
    await user.click(
      screen.getByRole('button', { name: 'security-password-match' }),
    )
    await user.click(screen.getByRole('button', { name: 'security-continue' }))
    await waitFor(() => expect(backend.previewSchedule).toHaveBeenCalled())
    failureRender.unmount()
    failedPreview.reject(new Error('late scheduler failure'))
    await Promise.allSettled([failedPreview.promise])
  })

  test('finishes encrypted onboarding, stores the key when requested, and reports finish failures', async () => {
    const user = userEvent.setup()
    const initializeArchive = vi.fn().mockResolvedValue(undefined)
    const runBackup = vi.fn().mockResolvedValue(undefined)
    shellData.current = shellDataFixture({
      initializeArchive,
      runBackup,
    })
    const successRender = renderPage()

    await advanceToReady(user)
    await user.click(screen.getByRole('button', { name: 'finish' }))

    expect(initializeArchive).toHaveBeenCalledWith(
      expect.objectContaining({ archiveMode: 'Encrypted' }),
      'secret',
    )
    expect(backend.keyringStoreDatabaseKey).toHaveBeenCalledWith('secret')
    expect(backend.applySchedule).toHaveBeenCalledWith(schedulePlanFixture())
    expect(runBackup).toHaveBeenCalledTimes(1)

    successRender.unmount()
    vi.restoreAllMocks()
    vi.spyOn(backend, 'previewSchedule').mockResolvedValue(
      schedulePlanFixture(),
    )
    vi.spyOn(backend, 'openExternalUrl').mockResolvedValue('opened')
    vi.spyOn(backend, 'keyringStoreDatabaseKey').mockResolvedValue(
      keyringStatusFixture(),
    )
    vi.spyOn(backend, 'scheduleStatus').mockResolvedValue(
      scheduleStatusFixture(),
    )
    vi.spyOn(backend, 'applySchedule').mockResolvedValue({
      applied: true,
      files: [],
      message: 'installed',
      platform: 'macos',
    })
    shellData.current = shellDataFixture({
      initializeArchive: vi.fn().mockRejectedValue(new Error('disk exploded')),
      runBackup: vi.fn(),
    })
    renderPage()

    await advanceToReady(user)
    await user.click(screen.getByRole('button', { name: 'finish' }))
    expect(await screen.findByText('disk exploded')).toBeInTheDocument()
  })

  test('skips schedule setup without applying a native schedule', async () => {
    const user = userEvent.setup()
    const initializeArchive = vi.fn().mockResolvedValue(undefined)
    const runBackup = vi.fn().mockResolvedValue(undefined)
    shellData.current = shellDataFixture({
      initializeArchive,
      runBackup,
    })
    renderPage()

    await user.click(screen.getByRole('button', { name: 'begin' }))
    await user.click(screen.getByRole('button', { name: 'browser-continue' }))
    await user.click(screen.getByRole('button', { name: 'storage-continue' }))
    await user.click(
      screen.getByRole('button', { name: 'security-password-match' }),
    )
    await user.click(screen.getByRole('button', { name: 'security-continue' }))
    await waitFor(() => expect(backend.previewSchedule).toHaveBeenCalled())
    await user.click(screen.getByRole('button', { name: 'schedule-skip' }))
    await user.click(screen.getByRole('button', { name: 'ai-skip' }))
    await user.click(screen.getByRole('button', { name: 'finish' }))

    expect(backend.applySchedule).not.toHaveBeenCalled()
    expect(initializeArchive).toHaveBeenCalledTimes(1)
    expect(runBackup).toHaveBeenCalledTimes(1)
  })

  test('blocks finish when schedule install fails during setup', async () => {
    const user = userEvent.setup()
    const runBackup = vi.fn().mockResolvedValue(undefined)
    vi.spyOn(backend, 'applySchedule').mockRejectedValueOnce(
      new Error('launchd denied'),
    )
    shellData.current = shellDataFixture({
      runBackup,
    })
    renderPage()

    await advanceToReady(user)
    await user.click(screen.getByRole('button', { name: 'finish' }))

    expect(
      await screen.findByText(
        "Couldn't install the schedule. Skip this step, then install it later from System → Scheduled Backup Settings.",
      ),
    ).toBeInTheDocument()
    expect(runBackup).not.toHaveBeenCalled()
  })

  test('refreshes the schedule plan at finish if setup proceeds before preview resolves', async () => {
    const user = userEvent.setup()
    const pendingPreview = deferred<SchedulePlan>()
    vi.spyOn(backend, 'previewSchedule')
      .mockReturnValueOnce(pendingPreview.promise)
      .mockResolvedValueOnce(schedulePlanFixture())
    renderPage()

    await user.click(screen.getByRole('button', { name: 'begin' }))
    await user.click(screen.getByRole('button', { name: 'browser-continue' }))
    await user.click(screen.getByRole('button', { name: 'storage-continue' }))
    await user.click(
      screen.getByRole('button', { name: 'security-password-match' }),
    )
    await user.click(screen.getByRole('button', { name: 'security-continue' }))
    await user.click(screen.getByRole('button', { name: 'schedule-install' }))
    await user.click(screen.getByRole('button', { name: 'ai-skip' }))
    await user.click(screen.getByRole('button', { name: 'finish' }))

    expect(backend.previewSchedule).toHaveBeenCalledTimes(2)
    expect(backend.applySchedule).toHaveBeenCalledWith(schedulePlanFixture())

    pendingPreview.resolve(schedulePlanFixture())
    await pendingPreview.promise
  })

  test('maps Safari permission failures from finish into the actionable recovery copy', async () => {
    const user = userEvent.setup()
    shellData.current = shellDataFixture({
      initializeArchive: vi
        .fn()
        .mockRejectedValue(
          new Error('Safari History.db needs Full Disk Access'),
        ),
      runBackup: vi.fn(),
    })
    renderPage()

    await advanceToReady(user)
    await user.click(screen.getByRole('button', { name: 'finish' }))

    expect(
      await screen.findByText(
        'Safari is not readable yet. Grant Full Disk Access to PathKeep or the running development process, then run the backup again.',
      ),
    ).toBeInTheDocument()
  })

  test('finishes an already initialized plaintext archive without reinitializing it', async () => {
    const user = userEvent.setup()
    const initializeArchive = vi.fn()
    const runBackup = vi.fn().mockResolvedValue(undefined)
    shellData.current = shellDataFixture({
      initializeArchive,
      runBackup,
      snapshot: snapshotFixture({
        archiveMode: 'Plaintext',
        initialized: true,
      }),
    })
    renderPage()

    await user.click(screen.getByRole('button', { name: 'begin' }))
    await user.click(screen.getByRole('button', { name: 'browser-continue' }))
    await user.click(screen.getByRole('button', { name: 'storage-continue' }))
    await user.click(screen.getByRole('button', { name: 'security-continue' }))
    await waitFor(() => expect(backend.previewSchedule).toHaveBeenCalled())
    await user.click(screen.getByRole('button', { name: 'schedule-install' }))
    await user.click(screen.getByRole('button', { name: 'ai-skip' }))
    await user.click(screen.getByRole('button', { name: 'finish' }))

    expect(initializeArchive).not.toHaveBeenCalled()
    expect(runBackup).toHaveBeenCalledTimes(1)
  })

  test('surfaces the keychain-specific error when storing the database key fails after a successful archive init', async () => {
    // Reproduces the "keyring went unavailable between probe and use" race
    // (locked Secret Service, sudden logout, etc). initializeArchive
    // already succeeded so the archive itself is fine — the user just
    // needs the keychain-specific copy, not the generic finish error.
    const user = userEvent.setup()
    const initializeArchive = vi.fn().mockResolvedValue(undefined)
    const runBackup = vi.fn().mockResolvedValue(undefined)
    vi.spyOn(backend, 'keyringStoreDatabaseKey').mockRejectedValueOnce(
      new Error('keychain locked'),
    )
    shellData.current = shellDataFixture({
      initializeArchive,
      runBackup,
    })
    renderPage()

    await advanceToReady(user)
    await user.click(screen.getByRole('button', { name: 'finish' }))

    expect(
      await screen.findByText(
        'PathKeep could not write the password to the system keychain. Uncheck the option to continue without it, or open Settings → Security after setup to retry.',
      ),
    ).toBeInTheDocument()
    expect(initializeArchive).toHaveBeenCalledTimes(1)
    // The catch arm must short-circuit before the schedule install / backup
    // path so the user can retry without re-running the whole pipeline.
    expect(backend.applySchedule).not.toHaveBeenCalled()
    expect(runBackup).not.toHaveBeenCalled()
  })

  test('blocks the browser step continue when no profile is selected and steers the message by access state', async () => {
    // handleBrowsersContinue is the only place the "select a profile" gate
    // fires in mid-flow (advanceing past the welcome step). We need both
    // arms of the access-issue ternary so the message tells the user
    // whether to grant permission or pick a different profile.
    const user = userEvent.setup()
    shellData.current = shellDataFixture({
      snapshot: snapshotFixture({ selectedProfileIds: [] }),
    })
    const { rerender } = renderPage()

    await user.click(screen.getByRole('button', { name: 'begin' }))
    await user.click(screen.getByRole('button', { name: 'browser-continue' }))
    expect(
      screen.getByText('Pick at least one browser profile to back up.'),
    ).toBeInTheDocument()

    shellData.current = shellDataFixture({
      snapshot: snapshotWithBlockedProfile({
        selectedProfileIds: ['safari:Blocked'],
      }),
    })
    rerender(pageElement())
    // Even with one selected profile, if it's not readable yet the
    // continue gate must fire with the Full-Disk-Access copy instead.
    await user.click(screen.getByRole('button', { name: 'browser-continue' }))
    expect(
      screen.getByText(
        'The selected browser profiles are not readable yet. Grant access first, or go back and choose a readable source.',
      ),
    ).toBeInTheDocument()
  })

  test('blocks security continue when encrypted mode has an empty master password', async () => {
    // handleSecurityContinue must catch the empty-password case before
    // routing to the schedule step. Without this guard the user would
    // discover the failure only at finish time, after running the
    // schedule preview fan-out for nothing.
    const user = userEvent.setup()
    renderPage()

    await user.click(screen.getByRole('button', { name: 'begin' }))
    await user.click(screen.getByRole('button', { name: 'browser-continue' }))
    await user.click(screen.getByRole('button', { name: 'storage-continue' }))
    await user.click(screen.getByRole('button', { name: 'security-continue' }))

    expect(
      screen.getByText('Enter a master password to use encrypted mode.'),
    ).toBeInTheDocument()
    // We never advanced to the schedule step, so the preview must not run.
    expect(backend.previewSchedule).not.toHaveBeenCalled()
  })

  test('adds a newly selected profile rather than overwriting the existing selection', async () => {
    // The existing toggle test takes the "remove" arm because the fixture
    // already has chrome:Default selected. We need the inverse so the
    // ternary's "add" arm is hit too — without this guard the route owner
    // could silently overwrite the selection list with a single entry.
    const user = userEvent.setup()
    const saveConfig = vi.fn((config: AppConfig) =>
      Promise.resolve({
        ...snapshotFixture(),
        config,
      }),
    )
    shellData.current = shellDataFixture({
      saveConfig,
      snapshot: snapshotFixture({
        // Pre-populate with a different profile so the toggle on
        // chrome:Default takes the "add" arm.
        selectedProfileIds: ['firefox:OtherProfile'],
      }),
    })
    renderPage()

    await user.click(screen.getByRole('button', { name: 'begin' }))
    await user.click(screen.getByRole('button', { name: 'toggle-profile' }))
    expect(saveConfig).toHaveBeenLastCalledWith(
      expect.objectContaining({
        selectedProfileIds: ['firefox:OtherProfile', 'chrome:Default'],
      }),
    )
  })

  test('initializes a plaintext archive with a null master password and skips the keyring write', async () => {
    // handleFinish must pass `null` (not the typed-but-unused password) to
    // initializeArchive when the user picked Plaintext, and must skip the
    // keyring call entirely. This is the only path through the ternary on
    // L287 — every other test takes the Encrypted arm.
    const user = userEvent.setup()
    const initializeArchive = vi.fn().mockResolvedValue(undefined)
    const runBackup = vi.fn().mockResolvedValue(undefined)
    shellData.current = shellDataFixture({
      initializeArchive,
      runBackup,
      snapshot: snapshotFixture({
        archiveMode: 'Plaintext',
        initialized: false,
        selectedProfileIds: ['chrome:Default'],
      }),
    })
    renderPage()

    await user.click(screen.getByRole('button', { name: 'begin' }))
    await user.click(screen.getByRole('button', { name: 'browser-continue' }))
    await user.click(screen.getByRole('button', { name: 'storage-continue' }))
    await user.click(screen.getByRole('button', { name: 'security-continue' }))
    await waitFor(() => expect(backend.previewSchedule).toHaveBeenCalled())
    await user.click(screen.getByRole('button', { name: 'schedule-install' }))
    await user.click(screen.getByRole('button', { name: 'ai-skip' }))
    await user.click(screen.getByRole('button', { name: 'finish' }))

    expect(initializeArchive).toHaveBeenCalledWith(
      expect.objectContaining({ archiveMode: 'Plaintext' }),
      null,
    )
    expect(backend.keyringStoreDatabaseKey).not.toHaveBeenCalled()
    expect(runBackup).toHaveBeenCalledTimes(1)
  })

  test('skips keyring storage when encrypted mode is selected but the user did not opt into the keychain', async () => {
    // Covers the encrypted-without-remember branch: handleFinish should
    // initialize with the password but skip backend.keyringStoreDatabaseKey
    // entirely. Without this guard the route would touch the keychain even
    // for users who explicitly declined the "remember" option.
    const user = userEvent.setup()
    const initializeArchive = vi.fn().mockResolvedValue(undefined)
    const runBackup = vi.fn().mockResolvedValue(undefined)
    shellData.current = shellDataFixture({
      initializeArchive,
      runBackup,
    })
    renderPage()

    await user.click(screen.getByRole('button', { name: 'begin' }))
    await user.click(screen.getByRole('button', { name: 'browser-continue' }))
    await user.click(screen.getByRole('button', { name: 'storage-continue' }))
    // Set matching passwords WITHOUT rememberKey using the dedicated mock
    // button so the encrypted-finish flow takes the
    // `if (encrypted && rememberKey)` false branch.
    await user.click(
      screen.getByRole('button', {
        name: 'security-password-match-no-remember',
      }),
    )
    await user.click(screen.getByRole('button', { name: 'security-continue' }))
    await waitFor(() => expect(backend.previewSchedule).toHaveBeenCalled())
    await user.click(screen.getByRole('button', { name: 'schedule-install' }))
    await user.click(screen.getByRole('button', { name: 'ai-skip' }))
    await user.click(screen.getByRole('button', { name: 'finish' }))

    expect(initializeArchive).toHaveBeenCalledWith(
      expect.objectContaining({ archiveMode: 'Encrypted' }),
      'secret',
    )
    expect(backend.keyringStoreDatabaseKey).not.toHaveBeenCalled()
    expect(runBackup).toHaveBeenCalledTimes(1)
  })

  test('returns from the schedule step back to the security step via the step header', async () => {
    // The route owner wires schedule-step `onBack` to `setStep(3)`. This
    // path is the only way to walk from step 4 → step 3 without going
    // through the stepper buttons, so it deserves its own guard.
    const user = userEvent.setup()
    renderPage()

    await user.click(screen.getByRole('button', { name: 'begin' }))
    await user.click(screen.getByRole('button', { name: 'browser-continue' }))
    await user.click(screen.getByRole('button', { name: 'storage-continue' }))
    await user.click(
      screen.getByRole('button', { name: 'security-password-match' }),
    )
    await user.click(screen.getByRole('button', { name: 'security-continue' }))
    await waitFor(() => expect(backend.previewSchedule).toHaveBeenCalled())

    await user.click(screen.getByRole('button', { name: 'schedule-back' }))
    expect(
      screen.getByRole('button', { name: 'security-continue' }),
    ).toBeInTheDocument()
  })

  test('inserts the optional AI step between Schedule and Ready; Skip advances to the final review', async () => {
    const user = userEvent.setup()
    renderPage()

    await user.click(screen.getByRole('button', { name: 'begin' }))
    await user.click(screen.getByRole('button', { name: 'browser-continue' }))
    await user.click(screen.getByRole('button', { name: 'storage-continue' }))
    await user.click(
      screen.getByRole('button', { name: 'security-password-match' }),
    )
    await user.click(screen.getByRole('button', { name: 'security-continue' }))
    await waitFor(() => expect(backend.previewSchedule).toHaveBeenCalled())
    await user.click(screen.getByRole('button', { name: 'schedule-install' }))

    // Schedule advances to the AI step (not straight to Ready).
    expect(screen.getByRole('button', { name: 'ai-skip' })).toBeInTheDocument()
    expect(
      screen.queryByRole('button', { name: 'finish' }),
    ).not.toBeInTheDocument()

    // AI back returns to the schedule step.
    await user.click(screen.getByRole('button', { name: 'ai-back' }))
    expect(
      screen.getByRole('button', { name: 'schedule-install' }),
    ).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'schedule-install' }))
    await user.click(screen.getByRole('button', { name: 'ai-skip' }))
    // Skip advances to the Ready step.
    expect(screen.getByRole('button', { name: 'finish' })).toBeInTheDocument()
  })

  test('deep-links "Set up AI in Settings" to the AI settings section', async () => {
    const user = userEvent.setup()
    render(
      <MemoryRouter initialEntries={['/onboarding']}>
        <I18nProvider>
          <OnboardingPage />
          <LocationProbe />
        </I18nProvider>
      </MemoryRouter>,
    )

    await user.click(screen.getByRole('button', { name: 'begin' }))
    await user.click(screen.getByRole('button', { name: 'browser-continue' }))
    await user.click(screen.getByRole('button', { name: 'storage-continue' }))
    await user.click(
      screen.getByRole('button', { name: 'security-password-match' }),
    )
    await user.click(screen.getByRole('button', { name: 'security-continue' }))
    await waitFor(() => expect(backend.previewSchedule).toHaveBeenCalled())
    await user.click(screen.getByRole('button', { name: 'schedule-skip' }))
    await user.click(screen.getByRole('button', { name: 'ai-setup' }))

    expect(screen.getByTestId('location-probe')).toHaveTextContent(
      '/settings#settings-ai',
    )
  })
})

/** Surfaces the current router location so navigation side effects are assertable. */
function LocationProbe() {
  const location = useLocation()
  return (
    <span data-testid="location-probe">
      {`${location.pathname}${location.hash}`}
    </span>
  )
}

async function advanceToReady(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole('button', { name: 'begin' }))
  await user.click(screen.getByRole('button', { name: 'browser-continue' }))
  await user.click(screen.getByRole('button', { name: 'storage-continue' }))
  await user.click(
    screen.getByRole('button', { name: 'security-password-match' }),
  )
  await user.click(screen.getByRole('button', { name: 'security-continue' }))
  await waitFor(() => expect(backend.previewSchedule).toHaveBeenCalled())
  await user.click(screen.getByRole('button', { name: 'schedule-install' }))
  // The optional AI step sits between Schedule and Ready; skip it to reach the final review.
  await user.click(screen.getByRole('button', { name: 'ai-skip' }))
}

function renderPage() {
  return render(pageElement())
}

function pageElement() {
  return (
    <MemoryRouter>
      <I18nProvider>
        <OnboardingPage />
      </I18nProvider>
    </MemoryRouter>
  )
}

function shellDataFixture(overrides: Record<string, unknown> = {}) {
  return {
    buildInfo: mockBuildInfo,
    busyAction: null,
    error: null,
    initializeArchive: vi.fn().mockResolvedValue(undefined),
    loading: false,
    runBackup: vi.fn().mockResolvedValue(undefined),
    saveConfig: vi.fn((config: AppConfig) =>
      Promise.resolve({
        ...snapshotFixture(),
        config,
      }),
    ),
    snapshot: snapshotFixture(),
    ...overrides,
  }
}

function snapshotFixture(configPatch: Partial<AppConfig> = {}): AppSnapshot {
  return {
    ...mockSnapshot,
    config: {
      ...mockSnapshot.config,
      archiveMode: 'Encrypted',
      initialized: false,
      selectedProfileIds: ['chrome:Default'],
      ...configPatch,
    },
    browserProfiles: [mockSnapshot.browserProfiles[0]],
  }
}

function snapshotWithBlockedProfile(configPatch: Partial<AppConfig> = {}) {
  const blockedProfile = {
    ...mockSnapshot.browserProfiles[0],
    accessIssue: 'full-disk-access',
    historyReadable: false,
    profileId: 'safari:Blocked',
  }

  return {
    ...snapshotFixture(configPatch),
    browserProfiles: [blockedProfile],
  }
}

function schedulePlanFixture(): SchedulePlan {
  return {
    applyCommands: [['launchctl', 'bootstrap']],
    applySupported: true,
    executablePath: '/Applications/PathKeep.app',
    generatedFiles: [
      {
        contents: '<plist />',
        purpose: 'launch-agent',
        relativePath: 'com.pathkeep.backup.plist',
      },
    ],
    label: 'PathKeep backup',
    manualSteps: [],
    platform: 'macos',
    rollbackCommands: [['launchctl', 'bootout']],
  }
}

function scheduleStatusFixture() {
  return {
    applySupported: true,
    auditPath: null,
    checkIntervalHours: 6,
    detectedFiles: [],
    dueAfterHours: 72,
    installState: 'not-installed',
    label: 'PathKeep backup',
    lastSuccessfulBackupAt: null,
    manualSteps: [],
    platform: 'macos',
    warnings: [],
  }
}

function keyringStatusFixture() {
  return {
    available: true,
    backend: 'stronghold',
    message: null,
    storedSecret: true,
  }
}

function deferred<T>() {
  let resolve: (value: T) => void = () => {}
  let reject: (error: unknown) => void = () => {}
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })

  return { promise, reject, resolve }
}
