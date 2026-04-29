/**
 * @file schedule-flows.test.tsx
 * @description Split trust-flow regression suite for the schedule state machine.
 * @module pages/trust-flows
 *
 * ## Responsibilities
 * - Verify state-local schedule actions replace the old always-visible PME tab layout.
 * - Protect manual fallback, repair, reinstall, remove, and verification recovery paths.
 * - Confirm scheduler action failures stay inline and do not strand the user.
 *
 * ## Non-Responsibilities
 * - Does not cover import, security, settings, or audit trust flows.
 * - Does not exercise real LaunchAgent, Task Scheduler, or systemd state.
 * - Does not snapshot visual styling.
 *
 * ## Dependencies
 * - Depends on shared trust-flow helpers for seeded archive state and shell/i18n providers.
 * - Uses the backend preview harness for route-level schedule plan/status fixtures.
 *
 * ## Performance Notes
 * - Reuses the shared initialized snapshot helper so this suite stays bounded to tiny in-memory fixtures.
 */

import { act, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'

const { invoke, isTauri, subscribeToImportProgress, waitForNextPaint } =
  vi.hoisted(() => ({
    invoke: vi.fn(),
    isTauri: vi.fn(() => false),
    subscribeToImportProgress: vi.fn(() => Promise.resolve(vi.fn())),
    waitForNextPaint: vi.fn(() => Promise.resolve()),
  }))

vi.mock('@tauri-apps/api/core', () => ({
  invoke,
  isTauri,
}))

vi.mock('../../lib/ipc/import-progress', () => ({
  subscribeToImportProgress,
}))

vi.mock('../../lib/wait-for-next-paint', () => ({
  waitForNextPaint,
}))

import { backend } from '../../lib/backend-client'
import { backendTestHarness } from '../../lib/backend'
import { createNamespaceTranslator } from '../../lib/i18n'
import type { ScheduleIssue, ScheduleStatus } from '../../lib/types'
import { SchedulePage } from '../schedule'
import {
  createShellValue,
  renderTrustPage,
  resetTrustFlowHarness,
  seedInitializedSnapshot,
} from './test-helpers'

describe('schedule trust flows', () => {
  beforeEach(() => {
    resetTrustFlowHarness({ invoke, isTauri, subscribeToImportProgress })
    waitForNextPaint.mockResolvedValue(undefined)
  })

  test('renders a localized not-installed state with read-only browser scope and manual steps', async () => {
    const { snapshot } = await seedInitializedSnapshot()
    const scheduleT = createNamespaceTranslator('zh-TW', 'schedule')

    backendTestHarness.seedSchedule(schedulePlanFixture(), {
      ...scheduleStatusFixture(),
      installState: 'not-installed',
      lastSuccessfulBackupAt: null,
    })

    renderTrustPage(<SchedulePage />, {
      language: 'zh-TW',
      route: '/schedule',
      snapshot,
    })

    expect(
      await screen.findByText(scheduleT('stateNotInstalledTitle')),
    ).toBeVisible()
    expect(screen.getByText(scheduleT('preInstallConfig'))).toBeVisible()
    expect(screen.getByText(scheduleT('backupScope'))).toBeVisible()
    expect(
      screen.getByRole('link', {
        name: scheduleT('editProfilesInSettings'),
      }),
    ).toHaveAttribute('href', '/settings#settings-profiles')
    expect(
      screen.getByRole('button', { name: scheduleT('autoInstall') }),
    ).toBeVisible()
    expect(screen.getByText(scheduleT('manualInstall'))).toBeVisible()
    expect(screen.getByText('launchctl bootstrap')).toBeInTheDocument()
  })

  test('keeps installed-ok actions focused on verify, detail, update, remove, and re-detect', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedInitializedSnapshot()
    const scheduleT = createNamespaceTranslator('en', 'schedule')
    const plan = schedulePlanFixture()
    const applySchedule = vi.spyOn(backend, 'applySchedule').mockResolvedValue({
      applied: true,
      auditPath: '/Users/test/AppData/apply-audit.json',
      files: ['~/Library/LaunchAgents/com.yi-ting.pathkeep.backup.plist'],
      message: 'Applied from route test.',
      platform: 'macos',
    })

    backendTestHarness.seedSchedule(plan, scheduleStatusFixture())

    renderTrustPage(<SchedulePage />, {
      language: 'en',
      route: '/schedule',
      snapshot,
    })

    expect(
      await screen.findByText(scheduleT('stateInstalledOkTitle')),
    ).toBeVisible()
    expect(screen.getByText(scheduleT('installedSummary'))).toBeVisible()
    expect(
      screen.getByRole('button', { name: scheduleT('verifyInstallation') }),
    ).toBeVisible()
    expect(screen.getByText(scheduleT('viewInstallDetails'))).toBeVisible()

    await user.click(
      screen.getByRole('button', { name: scheduleT('modifyInstallation') }),
    )

    await waitFor(() => {
      expect(applySchedule).toHaveBeenCalledWith(plan)
    })
    expect(await screen.findByText('Applied from route test.')).toBeVisible()
  })

  test('surfaces missing schedule plans with route-local recovery copy', async () => {
    const { snapshot } = await seedInitializedSnapshot()
    const scheduleT = createNamespaceTranslator('en', 'schedule')
    vi.spyOn(backend, 'previewSchedule').mockResolvedValue(null as never)
    vi.spyOn(backend, 'scheduleStatus').mockResolvedValue(
      scheduleStatusFixture(),
    )

    renderTrustPage(<SchedulePage />, {
      language: 'en',
      route: '/schedule',
      snapshot,
    })

    expect(await screen.findByText(scheduleT('unavailableTitle'))).toBeVisible()
    expect(screen.getByText(scheduleT('unavailableBody'))).toBeVisible()
  })

  test('ignores late schedule load results after the route unmounts', async () => {
    const { snapshot } = await seedInitializedSnapshot()
    const plan = deferred<ReturnType<typeof schedulePlanFixture>>()
    const status = deferred<ReturnType<typeof scheduleStatusFixture>>()
    vi.spyOn(backend, 'previewSchedule').mockReturnValueOnce(plan.promise)
    vi.spyOn(backend, 'scheduleStatus').mockReturnValueOnce(status.promise)

    const rendered = renderTrustPage(<SchedulePage />, {
      route: '/schedule',
      snapshot,
    })
    rendered.unmount()

    await act(async () => {
      plan.resolve(schedulePlanFixture())
      status.resolve(scheduleStatusFixture())
      await Promise.resolve()
      await Promise.resolve()
    })
  })

  test('maps typed warning and error issues into the warning/error states', async () => {
    const { snapshot } = await seedInitializedSnapshot()
    const scheduleT = createNamespaceTranslator('en', 'schedule')

    backendTestHarness.seedSchedule(schedulePlanFixture(), {
      ...scheduleStatusFixture(),
      installState: 'legacy-install-detected',
      issues: [legacyIssueFixture()],
    })
    const warningRender = renderTrustPage(<SchedulePage />, {
      route: '/schedule',
      snapshot,
    })

    expect(
      await screen.findByText(scheduleT('stateInstalledWarnTitle')),
    ).toBeVisible()
    expect(screen.getByText(scheduleT('issueLegacyAgentTitle'))).toBeVisible()
    expect(
      screen.queryByRole('button', { name: scheduleT('ignoreWarning') }),
    ).not.toBeInTheDocument()
    warningRender.unmount()

    backendTestHarness.seedSchedule(schedulePlanFixture(), {
      ...scheduleStatusFixture(),
      issues: [errorIssueFixture()],
    })
    renderTrustPage(<SchedulePage />, {
      route: '/schedule',
      snapshot,
    })

    expect(
      await screen.findByText(scheduleT('stateInstalledErrorTitle')),
    ).toBeVisible()
    expect(
      screen.getByRole('button', { name: scheduleT('copyDiagnostics') }),
    ).toBeVisible()
  })

  test('surfaces apply and remove failures inline while leaving recovery actions available', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedInitializedSnapshot()
    const scheduleT = createNamespaceTranslator('en', 'schedule')
    const plan = schedulePlanFixture()
    backendTestHarness.seedSchedule(plan, scheduleStatusFixture())
    vi.spyOn(backend, 'applySchedule').mockRejectedValue(
      new Error('apply denied'),
    )
    vi.spyOn(backend, 'removeSchedule').mockRejectedValue('remove fallback')

    renderTrustPage(<SchedulePage />, {
      language: 'en',
      route: '/schedule',
      snapshot,
    })

    await screen.findByText(scheduleT('stateInstalledOkTitle'))
    await user.click(
      screen.getByRole('button', { name: scheduleT('modifyInstallation') }),
    )
    expect(await screen.findByRole('alert')).toHaveTextContent('apply denied')

    await user.click(
      screen.getByRole('button', {
        name: scheduleT('removeInstalledSchedule'),
      }),
    )
    expect(await screen.findByRole('alert')).toHaveTextContent(
      scheduleT('operationFailed'),
    )
    expect(
      screen.getByRole('button', {
        name: scheduleT('removeInstalledSchedule'),
      }),
    ).toBeEnabled()
  })

  test('reports successful schedule removal and refreshes shell data before re-detecting', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedInitializedSnapshot()
    const shellValue = createShellValue(snapshot)
    const scheduleT = createNamespaceTranslator('en', 'schedule')
    const plan = schedulePlanFixture()
    backendTestHarness.seedSchedule(plan, scheduleStatusFixture())
    const removeSchedule = vi
      .spyOn(backend, 'removeSchedule')
      .mockResolvedValue({
        applied: true,
        auditPath: '/Users/test/AppData/remove-audit.json',
        files: ['~/Library/LaunchAgents/com.yi-ting.pathkeep.backup.plist'],
        message: 'Removed from route test.',
        platform: 'macos',
      })

    renderTrustPage(<SchedulePage />, {
      language: 'en',
      route: '/schedule',
      shellValue,
      snapshot,
    })

    await screen.findByText(scheduleT('stateInstalledOkTitle'))
    await user.click(
      screen.getByRole('button', {
        name: scheduleT('removeInstalledSchedule'),
      }),
    )

    await waitFor(() => {
      expect(removeSchedule).toHaveBeenCalledWith(plan)
      expect(shellValue.refreshAppData).toHaveBeenCalledTimes(1)
    })
    expect(await screen.findByText('Removed from route test.')).toBeVisible()
  })

  test('repairs known legacy scheduler artifacts through the explicit repair action', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedInitializedSnapshot()
    const scheduleT = createNamespaceTranslator('en', 'schedule')
    const plan = schedulePlanFixture()
    backendTestHarness.seedSchedule(plan, {
      ...scheduleStatusFixture(),
      installState: 'legacy-install-detected',
      issues: [legacyIssueFixture()],
    })
    const repairSchedule = vi
      .spyOn(backend, 'repairSchedule')
      .mockResolvedValue({
        applied: true,
        auditPath: '/Users/test/AppData/repair-audit.json',
        files: [],
        message: 'Legacy task removed.',
        platform: 'macos',
      })

    renderTrustPage(<SchedulePage />, {
      language: 'en',
      route: '/schedule',
      snapshot,
    })

    await screen.findByText(scheduleT('stateInstalledWarnTitle'))
    await user.click(
      screen.getAllByRole('button', { name: scheduleT('repairLegacy') })[0],
    )

    await waitFor(() => {
      expect(repairSchedule).toHaveBeenCalledWith(plan)
    })
    expect(await screen.findByText('Legacy task removed.')).toBeVisible()
  })

  test('manual completion and verify buttons run detection without native mutation', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedInitializedSnapshot()
    const scheduleT = createNamespaceTranslator('en', 'schedule')
    backendTestHarness.seedSchedule(schedulePlanFixture(), {
      ...scheduleStatusFixture(),
      installState: 'not-installed',
      lastSuccessfulBackupAt: null,
    })
    const scheduleStatus = vi.spyOn(backend, 'scheduleStatus')

    renderTrustPage(<SchedulePage />, {
      language: 'en',
      route: '/schedule',
      snapshot,
    })

    await screen.findByText(scheduleT('stateNotInstalledTitle'))
    await user.click(
      screen.getByRole('button', { name: scheduleT('manualComplete') }),
    )
    await user.click(
      screen.getByRole('button', { name: scheduleT('verifyStep') }),
    )

    await waitFor(() => {
      expect(scheduleStatus).toHaveBeenCalledTimes(3)
    })
  })
})

function schedulePlanFixture() {
  return {
    platform: 'macos' as const,
    label: 'com.yi-ting.pathkeep.backup',
    executablePath: '/Applications/PathKeep.app',
    generatedFiles: [
      {
        relativePath: 'schedule/com.yi-ting.pathkeep.backup.plist',
        absolutePath:
          '/Users/test/Library/LaunchAgents/com.yi-ting.pathkeep.backup.plist',
        purpose: 'LaunchAgent plist',
        contents:
          '<?xml version="1.0"?><plist><dict><key>Label</key><string>com.yi-ting.pathkeep.backup</string></dict></plist>',
      },
    ],
    manualStepDetails: [
      {
        canAutoRun: true,
        canVerify: true,
        command: ['launchctl', 'bootstrap'],
        directoryPath: '/Users/test/Library/LaunchAgents',
        fileContents:
          '<?xml version="1.0"?><plist><dict><key>Label</key><string>com.yi-ting.pathkeep.backup</string></dict></plist>',
        filePath:
          '/Users/test/Library/LaunchAgents/com.yi-ting.pathkeep.backup.plist',
        id: 'macos-save-plist',
        summaryKey: 'schedule.manualMacosSavePlistSummary',
        titleKey: 'schedule.manualMacosSavePlistTitle',
        whyKey: 'schedule.manualMacosSavePlistWhy',
      },
    ],
    manualSteps: ['Review the LaunchAgent install.'],
    applyCommands: [['launchctl', 'bootstrap']],
    rollbackCommands: [['launchctl', 'bootout']],
    applySupported: true,
  }
}

function scheduleStatusFixture(overrides: Partial<ScheduleStatus> = {}) {
  return {
    platform: 'macos' as const,
    label: 'com.yi-ting.pathkeep.backup',
    dueAfterHours: 72,
    checkIntervalHours: 6,
    applySupported: true,
    installState: 'installed' as const,
    detectedFiles: ['~/Library/LaunchAgents/com.yi-ting.pathkeep.backup.plist'],
    manualSteps: ['Remove the LaunchAgent if you no longer want automation.'],
    auditPath: '/Users/test/AppData/schedule-audit.json',
    checkedAt: '2026-04-29T12:00:00.000Z',
    issues: [],
    lastSuccessfulBackupAt: '2026-04-10T12:00:00Z',
    verificationChecks: [],
    warnings: [],
    ...overrides,
  }
}

function legacyIssueFixture(): ScheduleIssue {
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

function errorIssueFixture(): ScheduleIssue {
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

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, reject, resolve }
}
