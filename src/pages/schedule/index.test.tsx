/**
 * @file index.test.tsx
 * @description Route coverage for the Scheduled Backup Settings state machine.
 * @module pages/schedule
 *
 * ## Responsibilities
 * - Verify each top-level schedule state renders only the state-local actions.
 * - Cover install, remove, repair, manual fallback, redetect, and diagnostics feedback wiring.
 * - Keep browser profile scope read-only while preserving the Settings escape hatch.
 *
 * ## Not responsible for
 * - Re-testing native launchd or schtasks behavior.
 * - Snapshotting visual styles.
 *
 * ## Dependencies
 * - Mocks shell data, backend schedule commands, i18n, clipboard, and next-paint scheduling.
 *
 * ## Performance notes
 * - Uses tiny fixtures and never touches a real scheduler or archive.
 */

import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { SchedulePage } from './index'

const {
  backendMock,
  clipboardWriteTextMock,
  refreshAppDataMock,
  saveConfigMock,
  useShellDataMock,
} = vi.hoisted(() => ({
  backendMock: {
    applySchedule: vi.fn(),
    openPathInFileManager: vi.fn(),
    previewSchedule: vi.fn(),
    removeSchedule: vi.fn(),
    repairSchedule: vi.fn(),
    scheduleStatus: vi.fn(),
  },
  clipboardWriteTextMock: vi.fn(),
  refreshAppDataMock: vi.fn(),
  saveConfigMock: vi.fn(),
  useShellDataMock: vi.fn(),
}))

vi.mock('../../app/shell-data-context', () => ({
  useShellData: useShellDataMock,
}))

vi.mock('../../lib/backend-client', () => ({
  backend: backendMock,
}))

vi.mock('../../lib/i18n', () => ({
  localeTag: () => 'en-US',
  useI18n: () => ({
    language: 'en',
    t: (key: string, vars?: Record<string, string | number>) => {
      if (!vars) return key
      if ('hours' in vars) return `${key}:${vars.hours}`
      if ('minutes' in vars) return `${key}:${vars.minutes}`
      if ('time' in vars) return `${key}:${vars.time}`
      if ('path' in vars) return `${key}:${vars.path}`
      if ('current' in vars && 'total' in vars) {
        return `${key}:${vars.current}/${vars.total}`
      }
      return key
    },
  }),
}))

vi.mock('../../lib/wait-for-next-paint', () => ({
  waitForNextPaint: () => Promise.resolve(),
}))

describe('SchedulePage', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    const navigatorWithClipboard = Object.create(navigator) as Navigator
    Object.defineProperty(navigatorWithClipboard, 'clipboard', {
      configurable: true,
      value: {
        writeText: clipboardWriteTextMock,
      },
    })
    vi.stubGlobal('navigator', navigatorWithClipboard)
    clipboardWriteTextMock.mockResolvedValue(undefined)
    refreshAppDataMock.mockResolvedValue(snapshotFixture())
    saveConfigMock.mockImplementation((config) =>
      Promise.resolve(snapshotFixture({ config })),
    )
    useShellDataMock.mockReturnValue({
      refreshAppData: refreshAppDataMock,
      refreshKey: 1,
      saveConfig: saveConfigMock,
      snapshot: snapshotFixture(),
    })
    backendMock.previewSchedule.mockResolvedValue(planFixture())
    backendMock.scheduleStatus.mockResolvedValue(statusFixture())
    backendMock.applySchedule.mockResolvedValue(applyResultFixture())
    backendMock.removeSchedule.mockResolvedValue(applyResultFixture())
    backendMock.repairSchedule.mockResolvedValue(applyResultFixture())
  })

  test('shows checking and then the unavailable fallback when detection cannot build a read model', async () => {
    const user = userEvent.setup()
    backendMock.previewSchedule.mockResolvedValueOnce(null)

    renderSchedule()

    expect(screen.getByText('schedule.detectingStatus')).toBeVisible()
    expect(await screen.findByText('schedule.unavailableTitle')).toBeVisible()
    expect(screen.getByText('schedule.unavailableBody')).toBeVisible()

    await user.click(screen.getByRole('button', { name: 'schedule.redetect' }))

    expect(
      await screen.findByText('schedule.stateInstalledOkTitle'),
    ).toBeVisible()
    expect(backendMock.previewSchedule).toHaveBeenCalledTimes(2)
  })

  test('renders not-installed setup with read-only browser scope and manual install path', async () => {
    const user = userEvent.setup()
    backendMock.scheduleStatus.mockResolvedValue(
      statusFixture({
        installState: 'not-installed',
        lastSuccessfulBackupAt: null,
      }),
    )

    renderSchedule()

    expect(
      await screen.findByText('schedule.stateNotInstalledTitle'),
    ).toBeVisible()
    expect(screen.getByText('schedule.preInstallConfig')).toBeVisible()
    expect(screen.getByText('chrome')).toBeVisible()
    expect(screen.getByText('Default')).toBeVisible()
    expect(
      screen.getByRole('link', {
        name: 'schedule.editProfilesInSettings',
      }),
    ).toHaveAttribute('href', '/settings#settings-profiles')
    expect(
      screen.getByRole('button', { name: 'schedule.autoInstall' }),
    ).toBeVisible()
    expect(screen.getByText('schedule.manualInstall')).toBeVisible()
    expect(
      screen.queryByRole('button', { name: 'schedule.verifyInstallation' }),
    ).not.toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'schedule.redetect' }))
    await waitFor(() => {
      expect(backendMock.scheduleStatus).toHaveBeenCalledTimes(2)
    })

    await user.click(screen.getByText('schedule.manualInstall'))
    await user.click(screen.getByRole('button', { name: 'common.openPath' }))

    expect(backendMock.openPathInFileManager).toHaveBeenCalledWith(
      '~/Library/LaunchAgents',
    )

    await user.click(
      screen.getByRole('button', { name: 'schedule.autoRunStep' }),
    )
    await waitFor(() => {
      expect(backendMock.applySchedule).toHaveBeenCalledTimes(1)
    })
    await user.click(
      screen.getByRole('button', { name: 'schedule.verifyStep' }),
    )
    await waitFor(() => {
      expect(backendMock.scheduleStatus).toHaveBeenCalledTimes(4)
    })
    await user.click(
      screen.getByRole('button', { name: 'schedule.manualComplete' }),
    )
    await waitFor(() => {
      expect(backendMock.scheduleStatus).toHaveBeenCalledTimes(5)
    })
  })

  test('renders installed-ok summary, verification, details, update, and removal actions', async () => {
    const user = userEvent.setup()

    renderSchedule()

    expect(
      await screen.findByText('schedule.stateInstalledOkTitle'),
    ).toBeVisible()
    expect(screen.getByText('schedule.installedSummary')).toBeVisible()
    expect(screen.getByText('schedule.availableActions')).toBeVisible()
    expect(
      screen.getByRole('button', { name: 'schedule.verifyInstallation' }),
    ).toBeVisible()
    expect(
      screen.getByRole('button', { name: 'schedule.modifyInstallation' }),
    ).toBeVisible()
    expect(
      screen.getByRole('button', { name: 'schedule.removeInstalledSchedule' }),
    ).toBeVisible()
    expect(screen.getByText('schedule.viewInstallDetails')).toBeVisible()
    expect(screen.getByText('schedule.verifyMacosLoaded')).toBeInTheDocument()
    expect(
      screen.getByText('~/Library/LaunchAgents/pathkeep.plist'),
    ).toBeInTheDocument()

    await user.click(
      screen.getByRole('button', { name: 'schedule.verifyInstallation' }),
    )
    await waitFor(() => {
      expect(backendMock.scheduleStatus).toHaveBeenCalledTimes(2)
    })
    await user.click(
      screen.getByRole('button', { name: 'schedule.modifyInstallation' }),
    )
    await waitFor(() => {
      expect(backendMock.applySchedule).toHaveBeenCalledTimes(1)
    })
    await user.click(screen.getByRole('button', { name: 'schedule.redetect' }))
    await waitFor(() => {
      expect(backendMock.scheduleStatus).toHaveBeenCalledTimes(4)
    })
  })

  test('renders minute-level custom intervals without rounding back to hour presets', async () => {
    useShellDataMock.mockReturnValue({
      refreshAppData: refreshAppDataMock,
      refreshKey: 1,
      saveConfig: saveConfigMock,
      snapshot: snapshotFixture({
        config: {
          dueAfterHours: 1.5,
          initialized: true,
          scheduleCheckIntervalHours: 6,
          selectedProfileIds: ['chrome:Default'],
        },
      }),
    })
    backendMock.scheduleStatus.mockResolvedValueOnce(
      statusFixture({
        dueAfterHours: 1.5,
      }),
    )

    renderSchedule()

    expect(
      await screen.findByText('schedule.stateInstalledOkTitle'),
    ).toBeVisible()
    expect(screen.getByText('schedule.intervalValueMinutes:90')).toBeVisible()
    expect(screen.getByLabelText('schedule.intervalCustomLabel')).toHaveValue(
      90,
    )
  })

  test('renders installed-ok empty scope and relative generated file paths', async () => {
    useShellDataMock.mockReturnValue({
      refreshAppData: refreshAppDataMock,
      refreshKey: 1,
      saveConfig: saveConfigMock,
      snapshot: snapshotFixture({
        browserProfiles: [],
        config: {
          dueAfterHours: 12,
          initialized: true,
          scheduleCheckIntervalHours: 24,
          selectedProfileIds: [],
        },
      }),
    })
    backendMock.previewSchedule.mockResolvedValueOnce(
      planFixture({
        generatedFiles: [
          {
            absolutePath: null,
            contents: '<plist version="1.0"></plist>',
            purpose: 'LaunchAgent',
            relativePath: 'launchd/pathkeep.plist',
          },
        ],
      }),
    )

    renderSchedule()

    expect(
      await screen.findByText('schedule.stateInstalledOkTitle'),
    ).toBeVisible()
    expect(screen.getByText('schedule.noProfilesSelected')).toBeVisible()
    expect(screen.getByText('launchd/pathkeep.plist')).toBeInTheDocument()
  })

  test('renders not-installed empty scope, archive guard, and generic manual fallback commands', async () => {
    const user = userEvent.setup()
    useShellDataMock.mockReturnValue({
      refreshAppData: refreshAppDataMock,
      refreshKey: 1,
      saveConfig: saveConfigMock,
      snapshot: snapshotFixture({
        browserProfiles: [],
        config: {
          dueAfterHours: 12,
          initialized: false,
          scheduleCheckIntervalHours: 24,
          selectedProfileIds: [],
        },
      }),
    })
    backendMock.previewSchedule.mockResolvedValueOnce(
      planFixture({
        applyCommands: [['launchctl', 'bootstrap', 'path with space']],
        manualStepDetails: [],
        manualSteps: ['Run launchctl manually.', 'Verify launchctl manually.'],
      }),
    )
    backendMock.scheduleStatus.mockResolvedValueOnce(
      statusFixture({
        installState: 'not-installed',
        lastSuccessfulBackupAt: null,
      }),
    )

    renderSchedule()

    expect(
      await screen.findByText('schedule.stateNotInstalledTitle'),
    ).toBeVisible()
    expect(screen.getByText('schedule.noProfilesSelected')).toBeVisible()
    expect(screen.getByText('schedule.initializeArchiveFirst')).toBeVisible()

    await user.click(screen.getByText('schedule.manualInstall'))

    expect(screen.getAllByText('schedule.manualGenericStepTitle')).toHaveLength(
      2,
    )
    expect(
      screen.getByText('launchctl bootstrap "path with space"'),
    ).toBeVisible()
    expect(
      screen.getAllByText('schedule.manualGenericStepSummary'),
    ).toHaveLength(2)
  })

  test('renders not-installed fallback scope when the shell snapshot is unavailable', async () => {
    const user = userEvent.setup()
    useShellDataMock.mockReturnValue({
      refreshAppData: refreshAppDataMock,
      refreshKey: 1,
      saveConfig: saveConfigMock,
      snapshot: null,
    })
    backendMock.previewSchedule.mockResolvedValueOnce(
      planFixture({
        manualStepDetails: [
          {
            canAutoRun: false,
            canVerify: true,
            command: null,
            directoryPath: null,
            fileContents: null,
            filePath: '~/Library/LaunchAgents/empty.plist',
            id: 'empty-file',
            summaryKey: 'schedule.manualMacosSavePlistSummary',
            titleKey: 'schedule.manualMacosSavePlistTitle',
            whyKey: 'schedule.manualMacosSavePlistWhy',
          },
        ],
      }),
    )
    backendMock.scheduleStatus.mockResolvedValueOnce(
      statusFixture({
        installState: 'not-installed',
        lastSuccessfulBackupAt: null,
      }),
    )

    renderSchedule()

    expect(
      await screen.findByText('schedule.stateNotInstalledTitle'),
    ).toBeVisible()
    expect(screen.getByText('schedule.noProfilesSelected')).toBeVisible()
    expect(screen.getByText('schedule.initializeArchiveFirst')).toBeVisible()

    await user.click(screen.getByText('schedule.manualInstall'))

    expect(screen.getByText('~/Library/LaunchAgents/empty.plist')).toBeVisible()
  })

  test('renders legacy warning as a state with one-click repair and no dismiss action', async () => {
    const user = userEvent.setup()
    backendMock.scheduleStatus.mockResolvedValue(
      statusFixture({
        installState: 'legacy-install-detected',
        issues: [legacyIssueFixture()],
      }),
    )

    renderSchedule()

    expect(
      await screen.findByText('schedule.stateInstalledWarnTitle'),
    ).toBeVisible()
    expect(screen.getByText('schedule.issueLegacyAgentTitle')).toBeVisible()
    expect(screen.getByText('schedule.issueLegacyAgentDetail')).toBeVisible()
    expect(
      screen.getByText('schedule.issueLegacyAgentConsequence'),
    ).toBeVisible()
    expect(
      screen.getByText('dev.codex.browser-history-backup.backup'),
    ).toBeVisible()
    expect(
      screen.queryByRole('button', { name: 'schedule.ignoreWarning' }),
    ).not.toBeInTheDocument()

    await user.click(screen.getByText('schedule.manualRepair'))
    await user.click(screen.getByRole('button', { name: 'common.openPath' }))
    expect(backendMock.openPathInFileManager).toHaveBeenCalledWith(
      '~/Library/LaunchAgents',
    )

    await user.click(
      screen.getByRole('button', { name: 'schedule.autoRunStep' }),
    )
    await waitFor(() => {
      expect(backendMock.repairSchedule).toHaveBeenCalledTimes(1)
    })
    await user.click(
      screen.getAllByRole('button', { name: 'schedule.repairLegacy' })[0],
    )

    await waitFor(() => {
      expect(backendMock.repairSchedule).toHaveBeenCalledTimes(2)
    })
  })

  test('allows only dismissible non-blocking warnings to be ignored', async () => {
    const user = userEvent.setup()
    backendMock.scheduleStatus.mockResolvedValueOnce(
      statusFixture({
        issues: [dismissibleIssueFixture()],
      }),
    )

    renderSchedule()

    expect(
      await screen.findByText('schedule.issueNeedsReviewTitle'),
    ).toBeVisible()
    await user.click(
      screen.getByRole('button', { name: 'schedule.ignoreWarning' }),
    )

    expect(
      screen.queryByText('schedule.issueNeedsReviewTitle'),
    ).not.toBeInTheDocument()
    expect(screen.getByText('schedule.stateInstalledOkTitle')).toBeVisible()
  })

  test('warns when the installed schedule has never completed a backup', async () => {
    useShellDataMock.mockReturnValue({
      refreshAppData: refreshAppDataMock,
      refreshKey: 1,
      saveConfig: saveConfigMock,
      snapshot: snapshotFixture({
        archiveStatus: { lastSuccessfulBackupAt: null },
      }),
    })
    backendMock.scheduleStatus.mockResolvedValueOnce(
      statusFixture({
        lastSuccessfulBackupAt: null,
      }),
    )

    renderSchedule()

    expect(
      await screen.findByText('schedule.stateInstalledWarnTitle'),
    ).toBeVisible()
    expect(screen.getByText('schedule.issueNeverRunTitle')).toBeVisible()
    expect(screen.getByText('schedule.neverRun')).toBeVisible()
  })

  test('renders generic warning fallback when the backend requests manual review without issue details', async () => {
    backendMock.scheduleStatus.mockResolvedValueOnce(
      statusFixture({
        installState: 'manual-review',
        issues: [],
      }),
    )

    renderSchedule()

    expect(
      await screen.findByText('schedule.stateInstalledWarnTitle'),
    ).toBeVisible()
    expect(screen.getByText('schedule.issueNeedsReviewTitle')).toBeVisible()
  })

  test('uses reinstall actions for non-legacy warning states', async () => {
    const user = userEvent.setup()
    backendMock.scheduleStatus.mockResolvedValue(
      statusFixture({
        installState: 'mismatch',
        issues: [mismatchIssueFixture()],
      }),
    )

    renderSchedule()

    expect(
      await screen.findByText('schedule.stateInstalledWarnTitle'),
    ).toBeVisible()
    expect(
      screen.queryByRole('button', { name: 'schedule.repairLegacy' }),
    ).not.toBeInTheDocument()
    await user.click(
      screen.getAllByRole('button', { name: 'schedule.reinstallSchedule' })[0],
    )
    await waitFor(() => {
      expect(backendMock.applySchedule).toHaveBeenCalledTimes(1)
    })

    await user.click(screen.getByText('schedule.manualInstall'))
    await user.click(
      screen.getByRole('button', { name: 'schedule.autoRunStep' }),
    )
    await waitFor(() => {
      expect(backendMock.applySchedule).toHaveBeenCalledTimes(2)
    })
    await user.click(
      screen.getByRole('button', { name: 'schedule.verifyStep' }),
    )
    await user.click(
      screen.getByRole('button', { name: 'schedule.manualComplete' }),
    )
    await user.click(
      screen.getByRole('button', { name: 'schedule.removeInstalledSchedule' }),
    )
    await waitFor(() => {
      expect(backendMock.removeSchedule).toHaveBeenCalledTimes(1)
    })
    await user.click(screen.getByRole('button', { name: 'schedule.redetect' }))
    await waitFor(() => {
      expect(backendMock.scheduleStatus).toHaveBeenCalledTimes(7)
    })
  })

  test('renders error recovery with manual removal and diagnostic copy feedback', async () => {
    const user = userEvent.setup()
    backendMock.scheduleStatus.mockResolvedValue(
      statusFixture({
        issues: [errorIssueFixture()],
      }),
    )

    renderSchedule()

    expect(
      await screen.findByText('schedule.stateInstalledErrorTitle'),
    ).toBeVisible()
    expect(
      screen.getByText('schedule.issueLaunchAgentNotLoadedTitle'),
    ).toBeVisible()
    expect(
      screen.getByRole('button', { name: 'schedule.reinstallSchedule' }),
    ).toBeVisible()
    expect(
      screen.getAllByRole('button', { name: 'schedule.manualRemove' })[0],
    ).toBeVisible()

    await user.click(
      screen.getByRole('button', { name: 'schedule.copyDiagnostics' }),
    )

    expect(await screen.findByText('schedule.diagnosticsCopied')).toBeVisible()

    await user.click(
      screen.getByRole('button', { name: 'schedule.reinstallSchedule' }),
    )
    await waitFor(() => {
      expect(backendMock.applySchedule).toHaveBeenCalledTimes(1)
    })
    await user.click(
      screen.getAllByRole('button', { name: 'schedule.manualRemove' })[0],
    )
    await waitFor(() => {
      expect(backendMock.removeSchedule).toHaveBeenCalledTimes(1)
    })
    await user.click(screen.getByRole('button', { name: 'schedule.redetect' }))
    await waitFor(() => {
      expect(backendMock.scheduleStatus).toHaveBeenCalledTimes(4)
    })

    await user.click(screen.getByText('schedule.manualRemovePath'))
    await user.click(screen.getByRole('button', { name: 'common.openPath' }))
    expect(backendMock.openPathInFileManager).toHaveBeenCalledWith(
      '~/Library/LaunchAgents',
    )
    await user.click(
      screen.getByRole('button', { name: 'schedule.autoRunStep' }),
    )
    await waitFor(() => {
      expect(backendMock.removeSchedule).toHaveBeenCalledTimes(2)
    })
    await user.click(
      screen.getByRole('button', { name: 'schedule.verifyStep' }),
    )
    await user.click(
      screen.getByRole('button', { name: 'schedule.manualComplete' }),
    )
    await waitFor(() => {
      expect(backendMock.scheduleStatus).toHaveBeenCalledTimes(7)
    })
  })

  test('renders error fallback copy when no typed issue or verification check is available', async () => {
    backendMock.scheduleStatus.mockResolvedValueOnce(
      statusFixture({
        installState: 'permission-warning',
        issues: [],
        verificationChecks: undefined,
      }),
    )

    renderSchedule()

    expect(
      await screen.findByText('schedule.stateInstalledErrorTitle'),
    ).toBeVisible()
    expect(
      screen.getByText('schedule.issueInspectionFailedTitle'),
    ).toBeVisible()
    expect(screen.getByText('schedule.noVerificationChecks')).toBeVisible()
  })

  test('saves interval changes before installing the refreshed native plan', async () => {
    const user = userEvent.setup()
    const refreshedPlan = planFixture({ label: 'PathKeep Backup refreshed' })
    backendMock.previewSchedule
      .mockResolvedValueOnce(planFixture())
      .mockResolvedValueOnce(refreshedPlan)
      .mockResolvedValue(planFixture())
    backendMock.scheduleStatus.mockResolvedValueOnce(
      statusFixture({
        installState: 'not-installed',
        lastSuccessfulBackupAt: null,
      }),
    )

    renderSchedule()

    await screen.findByText('schedule.stateNotInstalledTitle')
    const customInput = screen.getByLabelText('schedule.intervalCustomLabel')
    await waitFor(() => expect(customInput).toHaveValue(720))
    fireEvent.change(customInput, { target: { value: '90' } })
    expect(customInput).toHaveValue(90)
    await user.click(
      screen.getByRole('button', { name: 'schedule.autoInstall' }),
    )

    await waitFor(() => {
      expect(saveConfigMock).toHaveBeenCalledWith(
        expect.objectContaining({ dueAfterHours: 1.5 }),
      )
      expect(backendMock.applySchedule).toHaveBeenCalledWith(refreshedPlan)
    })
  })

  test('removes an installed schedule and refreshes shell data before re-detecting', async () => {
    const user = userEvent.setup()

    renderSchedule()

    await screen.findByText('schedule.stateInstalledOkTitle')
    await user.click(
      screen.getByRole('button', { name: 'schedule.removeInstalledSchedule' }),
    )

    await waitFor(() => {
      expect(backendMock.removeSchedule).toHaveBeenCalledTimes(1)
      expect(refreshAppDataMock).toHaveBeenCalledTimes(1)
      expect(backendMock.scheduleStatus).toHaveBeenCalledTimes(2)
    })
  })
})

function renderSchedule() {
  return render(
    <MemoryRouter>
      <SchedulePage />
    </MemoryRouter>,
  )
}

function snapshotFixture(overrides: Record<string, unknown> = {}) {
  return {
    archiveStatus: {
      lastSuccessfulBackupAt: '2026-04-25T12:00:00.000Z',
    },
    browserProfiles: [
      {
        browserName: 'chrome',
        displayName: 'Chrome Default',
        profileId: 'chrome:Default',
        profileName: 'Default',
      },
      {
        browserName: 'safari',
        displayName: 'Safari default',
        profileId: 'safari:default',
        profileName: 'default',
      },
    ],
    config: {
      dueAfterHours: 12,
      initialized: true,
      scheduleCheckIntervalHours: 24,
      selectedProfileIds: ['chrome:Default'],
    },
    ...overrides,
  }
}

function planFixture(overrides: Record<string, unknown> = {}) {
  return {
    applyCommands: [['launchctl', 'bootstrap', 'gui/501', 'pathkeep.plist']],
    applySupported: true,
    executablePath: '/Applications/PathKeep.app',
    generatedFiles: [
      {
        absolutePath: '~/Library/LaunchAgents/pathkeep.plist',
        contents: '<plist version="1.0"></plist>',
        purpose: 'LaunchAgent',
        relativePath: 'launchd/pathkeep.plist',
      },
    ],
    label: 'PathKeep Backup',
    manualStepDetails: [
      {
        canAutoRun: true,
        canVerify: true,
        command: ['launchctl', 'bootstrap', 'gui/501', 'pathkeep.plist'],
        directoryPath: '~/Library/LaunchAgents',
        fileContents: '<plist version="1.0"></plist>',
        filePath: '~/Library/LaunchAgents/pathkeep.plist',
        id: 'macos-save-plist',
        summaryKey: 'schedule.manualMacosSavePlistSummary',
        titleKey: 'schedule.manualMacosSavePlistTitle',
        whyKey: 'schedule.manualMacosSavePlistWhy',
      },
    ],
    manualSteps: [],
    platform: 'macos',
    rollbackCommands: [['launchctl', 'bootout', 'gui/501/pathkeep']],
    ...overrides,
  }
}

function statusFixture(overrides: Record<string, unknown> = {}) {
  return {
    applySupported: true,
    auditPath: '/tmp/pathkeep/schedule-audit.json',
    checkIntervalHours: 24,
    checkedAt: '2026-04-29T12:00:00.000Z',
    detectedFiles: ['~/Library/LaunchAgents/pathkeep.plist'],
    dueAfterHours: 12,
    installState: 'installed',
    issues: [],
    label: 'PathKeep Backup',
    lastSuccessfulBackupAt: '2026-04-25T12:00:00.000Z',
    manualStepDetails: [],
    manualSteps: [],
    platform: 'macos',
    verificationChecks: [
      {
        detailKey: 'schedule.verifyMacosLoadedOk',
        evidence: ['launchctl print gui/501/pathkeep'],
        key: 'macos-loaded',
        labelKey: 'schedule.verifyMacosLoaded',
        status: 'ok',
      },
    ],
    warnings: [],
    ...overrides,
  }
}

function legacyIssueFixture() {
  return {
    code: 'legacy-launch-agent',
    consequenceKey: 'schedule.issueLegacyAgentConsequence',
    detailKey: 'schedule.issueLegacyAgentDetail',
    dismissible: false,
    evidence: ['dev.codex.browser-history-backup.backup'],
    repairAction: 'repair-legacy',
    severity: 'warning',
    titleKey: 'schedule.issueLegacyAgentTitle',
  }
}

function dismissibleIssueFixture() {
  return {
    code: 'non-blocking-review',
    consequenceKey: 'schedule.issueNeedsReviewDetail',
    detailKey: 'schedule.issueNeedsReviewDetail',
    dismissible: true,
    evidence: [],
    repairAction: null,
    severity: 'warning',
    titleKey: 'schedule.issueNeedsReviewTitle',
  }
}

function mismatchIssueFixture() {
  return {
    code: 'config-mismatch',
    consequenceKey: 'schedule.issueConfigMismatchConsequence',
    detailKey: 'schedule.issueConfigMismatchDetail',
    dismissible: false,
    evidence: ['Interval mismatch'],
    repairAction: 'reinstall',
    severity: 'warning',
    titleKey: 'schedule.issueConfigMismatchTitle',
  }
}

function errorIssueFixture() {
  return {
    code: 'macos-launch-agent-not-loaded',
    consequenceKey: 'schedule.issueLaunchAgentNotLoadedConsequence',
    detailKey: 'schedule.issueLaunchAgentNotLoadedDetail',
    dismissible: false,
    evidence: ['launchctl print failed'],
    repairAction: 'reinstall',
    severity: 'error',
    titleKey: 'schedule.issueLaunchAgentNotLoadedTitle',
  }
}

function applyResultFixture() {
  return {
    applied: true,
    auditPath: '/tmp/pathkeep/schedule-apply.json',
    files: ['~/Library/LaunchAgents/pathkeep.plist'],
    message: 'ok',
    platform: 'macos',
  }
}
