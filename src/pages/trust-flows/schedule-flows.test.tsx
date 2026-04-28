/**
 * @file schedule-flows.test.tsx
 * @description Split trust-flow regression suite for schedule PMEs, platform guidance, and direct-execution controls.
 * @module pages/trust-flows
 *
 * ## Responsibilities
 * - Verify the schedule page keeps its platform-specific guidance and PME sequencing intact.
 * - Protect keyboard reachability across Preview, Manual, Execute, and Verify tabs.
 * - Confirm direct apply/remove controls stay visible when the schedule runtime supports them.
 *
 * ## Non-Responsibilities
 * - Does not cover import, security, settings, or audit trust flows.
 * - Does not own the shared route harness or cross-suite fixtures.
 * - Does not rewrite the original mega-suite during the split phase.
 *
 * ## Dependencies
 * - Depends on shared trust-flow helpers for seeded archive state and shell/i18n providers.
 * - Keeps the Tauri core and import-progress module boundaries mocked so route modules load under test.
 *
 * ## Performance Notes
 * - Reuses the shared initialized snapshot helper so splitting the mega-suite does not multiply setup cost.
 */

import { act, screen, waitFor, within } from '@testing-library/react'
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
import { createNamespaceTranslator, createTranslator } from '../../lib/i18n'
import { platformLabelKey } from '../../lib/platform-guidance'
import { SchedulePage } from '../schedule'
import {
  expectHtmlElement,
  renderTrustPage,
  resetTrustFlowHarness,
  seedInitializedSnapshot,
} from './test-helpers'

describe('schedule trust flows', () => {
  beforeEach(() => {
    resetTrustFlowHarness({ invoke, isTauri, subscribeToImportProgress })
    waitForNextPaint.mockResolvedValue(undefined)
  })

  test('renders Windows scheduler guidance and keeps PME tabs keyboard reachable', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedInitializedSnapshot()
    const scheduleT = createNamespaceTranslator('zh-TW', 'schedule')
    const zhTwT = createTranslator('zh-TW')
    const previewSpy = vi.spyOn(backend, 'previewSchedule').mockResolvedValue({
      platform: 'windows',
      label: 'com.yi-ting.pathkeep.backup',
      executablePath: 'C:/Program Files/PathKeep/pathkeep.exe',
      generatedFiles: [
        {
          relativePath: 'schedule/com.yi-ting.pathkeep.task.xml',
          absolutePath:
            'C:/Users/test/AppData/Local/com.yi-ting.pathkeep/schedule/com.yi-ting.pathkeep.task.xml',
          purpose: 'Task Scheduler XML',
          contents:
            '<Task><Settings><StartWhenAvailable>true</StartWhenAvailable></Settings></Task>',
        },
      ],
      manualSteps: ['Review the XML before import.'],
      applyCommands: [
        ['schtasks', '/Create', '/XML', 'com.yi-ting.pathkeep.task.xml'],
      ],
      rollbackCommands: [
        ['schtasks', '/Delete', '/TN', 'com.yi-ting.pathkeep.backup', '/F'],
      ],
      applySupported: false,
    })
    const statusSpy = vi.spyOn(backend, 'scheduleStatus').mockResolvedValue({
      platform: 'windows',
      label: 'com.yi-ting.pathkeep.backup',
      dueAfterHours: 48,
      checkIntervalHours: 6,
      applySupported: false,
      installState: 'manual-review',
      detectedFiles: [],
      manualSteps: ['Import the XML in Task Scheduler.'],
      auditPath: null,
      lastSuccessfulBackupAt: null,
      warnings: ['Review StartWhenAvailable before trusting the schedule.'],
    })

    renderTrustPage(<SchedulePage />, {
      language: 'zh-TW',
      route: '/schedule',
      snapshot,
    })

    const workflowPanel = expectHtmlElement(
      (await screen.findByText(scheduleT('pmeTitle'))).closest('.panel'),
    )

    expect(
      await screen.findByRole('heading', {
        name: zhTwT(platformLabelKey('windows')),
      }),
    ).toBeVisible()
    expect(
      await screen.findByText(scheduleT('intervalValue', { hours: 48 })),
    ).toBeVisible()
    expect(
      await screen.findByText(scheduleT('verificationValue', { hours: 6 })),
    ).toBeVisible()

    const previewTab = within(workflowPanel).getByRole('button', {
      name: zhTwT('common.previewTab'),
    })
    const manualTab = within(workflowPanel).getByRole('button', {
      name: zhTwT('common.manualTab'),
    })
    const executeTab = within(workflowPanel).getByRole('button', {
      name: zhTwT('common.executeTab'),
    })
    const verifyTab = within(workflowPanel).getByRole('button', {
      name: zhTwT('common.verifyTab'),
    })

    previewTab.focus()
    expect(previewTab).toHaveFocus()
    await user.tab()
    expect(manualTab).toHaveFocus()
    await user.tab()
    expect(executeTab).toHaveFocus()
    await user.tab()
    expect(verifyTab).toHaveFocus()

    await user.click(verifyTab)
    expect(
      (
        await within(workflowPanel).findAllByText(
          'Review StartWhenAvailable before trusting the schedule.',
        )
      )[0],
    ).toBeVisible()

    previewSpy.mockRestore()
    statusSpy.mockRestore()
  })

  test('shows apply and remove controls when the schedule supports direct execution', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedInitializedSnapshot()
    const scheduleT = createNamespaceTranslator('en', 'schedule')
    const enT = createTranslator('en')

    backendTestHarness.seedSchedule(
      {
        platform: 'macos',
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
        manualSteps: ['Review the LaunchAgent install.'],
        applyCommands: [['launchctl', 'bootstrap']],
        rollbackCommands: [['launchctl', 'bootout']],
        applySupported: true,
      },
      {
        platform: 'macos',
        label: 'com.yi-ting.pathkeep.backup',
        dueAfterHours: 72,
        checkIntervalHours: 6,
        applySupported: true,
        installState: 'installed',
        detectedFiles: [
          '~/Library/LaunchAgents/com.yi-ting.pathkeep.backup.plist',
        ],
        manualSteps: [
          'Remove the LaunchAgent if you no longer want automation.',
        ],
        auditPath: null,
        lastSuccessfulBackupAt: null,
        warnings: [],
      },
    )

    renderTrustPage(<SchedulePage />, {
      language: 'en',
      route: '/schedule',
      snapshot,
    })

    const workflowPanel = expectHtmlElement(
      (await screen.findByText(scheduleT('pmeTitle'))).closest('.panel'),
    )

    await user.click(
      within(workflowPanel).getByRole('button', {
        name: enT('common.executeTab'),
      }),
    )

    expect(await screen.findByText('launchctl bootstrap')).toBeVisible()
    expect(
      screen.getByRole('button', { name: scheduleT('applySchedule') }),
    ).toBeEnabled()
    expect(
      screen.getByRole('button', { name: scheduleT('removeSchedule') }),
    ).toBeEnabled()
  })

  test('surfaces schedule load failures with the route error state', async () => {
    const { snapshot } = await seedInitializedSnapshot()
    const scheduleT = createNamespaceTranslator('en', 'schedule')
    vi.spyOn(backend, 'previewSchedule').mockRejectedValue(
      new Error('Preview exploded'),
    )

    renderTrustPage(<SchedulePage />, {
      language: 'en',
      route: '/schedule',
      snapshot,
    })

    expect(await screen.findByText(scheduleT('unavailableTitle'))).toBeVisible()
    expect(screen.getByText('Preview exploded')).toBeVisible()
  })

  test('falls back to localized schedule error copy for non-error failures', async () => {
    const { snapshot } = await seedInitializedSnapshot()
    const scheduleT = createNamespaceTranslator('en', 'schedule')
    vi.spyOn(backend, 'previewSchedule').mockRejectedValue('offline')

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

    const successRender = renderTrustPage(<SchedulePage />, {
      route: '/schedule',
      snapshot,
    })
    successRender.unmount()

    await act(async () => {
      plan.resolve(schedulePlanFixture())
      status.resolve(scheduleStatusFixture())
      await Promise.resolve()
      await Promise.resolve()
    })

    const rejectedPlan = deferred<ReturnType<typeof schedulePlanFixture>>()
    vi.spyOn(backend, 'previewSchedule').mockReturnValueOnce(
      rejectedPlan.promise,
    )
    vi.spyOn(backend, 'scheduleStatus').mockResolvedValueOnce(
      scheduleStatusFixture(),
    )

    const failureRender = renderTrustPage(<SchedulePage />, {
      route: '/schedule',
      snapshot,
    })
    failureRender.unmount()

    await act(async () => {
      rejectedPlan.reject(new Error('late preview failure'))
      await Promise.resolve()
      await Promise.resolve()
    })
  })

  test('renders route-level schedule install-state descriptions', async () => {
    const { snapshot } = await seedInitializedSnapshot()
    const scheduleT = createNamespaceTranslator('en', 'schedule')
    const cases = [
      {
        installState: 'not-installed' as const,
        badge: scheduleT('notInstalledBadge'),
        description: scheduleT('notInstalledDescription'),
      },
      {
        installState: 'mismatch' as const,
        badge: scheduleT('attentionBadge'),
        description: scheduleT('mismatchDescription'),
      },
      {
        installState: 'permission-warning' as const,
        badge: scheduleT('attentionBadge'),
        description: scheduleT('permissionWarningDescription'),
      },
      {
        installState: 'legacy-install-detected' as const,
        badge: scheduleT('attentionBadge'),
        description: scheduleT('legacyInstallDescription'),
      },
    ]

    for (const testCase of cases) {
      backendTestHarness.seedSchedule(schedulePlanFixture(), {
        ...scheduleStatusFixture(),
        installState: testCase.installState,
      })
      const rendered = renderTrustPage(<SchedulePage />, {
        route: '/schedule',
        snapshot,
      })

      expect(
        (await screen.findAllByText(testCase.badge)).length,
      ).toBeGreaterThan(0)
      expect(screen.getAllByText(testCase.description).length).toBeGreaterThan(
        0,
      )
      rendered.unmount()
    }
  })

  test('surfaces non-error apply failures without losing the execute controls', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedInitializedSnapshot()
    const scheduleT = createNamespaceTranslator('en', 'schedule')
    const enT = createTranslator('en')
    const plan = schedulePlanFixture()

    backendTestHarness.seedSchedule(plan, scheduleStatusFixture())
    const applySchedule = vi
      .spyOn(backend, 'applySchedule')
      .mockRejectedValue('offline')

    renderTrustPage(<SchedulePage />, {
      language: 'en',
      route: '/schedule',
      snapshot,
    })

    await screen.findByTestId('schedule-page')
    await user.click(
      screen.getAllByRole('button', { name: enT('common.executeTab') })[0],
    )
    await user.click(
      await screen.findByRole('button', { name: scheduleT('applySchedule') }),
    )

    await waitFor(() => {
      expect(applySchedule).toHaveBeenCalledWith(plan)
    })
    expect(await screen.findByRole('alert')).toHaveTextContent(
      enT('common.unavailable'),
    )
    expect(
      screen.getByRole('button', { name: scheduleT('removeSchedule') }),
    ).toBeEnabled()
  })

  test('surfaces Error apply failures and non-error remove failures', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedInitializedSnapshot()
    const scheduleT = createNamespaceTranslator('en', 'schedule')
    const enT = createTranslator('en')
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

    await screen.findByTestId('schedule-page')
    await user.click(
      screen.getAllByRole('button', { name: enT('common.executeTab') })[0],
    )
    await user.click(
      await screen.findByRole('button', { name: scheduleT('applySchedule') }),
    )
    expect(await screen.findByRole('alert')).toHaveTextContent('apply denied')

    await user.click(
      screen.getByRole('button', { name: scheduleT('removeSchedule') }),
    )
    expect(await screen.findByRole('alert')).toHaveTextContent(
      enT('common.unavailable'),
    )
  })

  test('reports successful schedule removal and refreshes app data', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedInitializedSnapshot()
    const scheduleT = createNamespaceTranslator('en', 'schedule')
    const enT = createTranslator('en')
    const plan = schedulePlanFixture()

    backendTestHarness.seedSchedule(plan, scheduleStatusFixture())
    const removeSchedule = vi
      .spyOn(backend, 'removeSchedule')
      .mockResolvedValue({
        applied: true,
        platform: 'macos',
        files: ['~/Library/LaunchAgents/com.yi-ting.pathkeep.backup.plist'],
        auditPath: '/Users/test/AppData/remove-audit.json',
        message: 'Removed from route test.',
      })

    renderTrustPage(<SchedulePage />, {
      language: 'en',
      route: '/schedule',
      snapshot,
    })

    await screen.findByTestId('schedule-page')
    await user.click(
      screen.getAllByRole('button', { name: enT('common.executeTab') })[0],
    )
    await user.click(
      await screen.findByRole('button', { name: scheduleT('removeSchedule') }),
    )

    await waitFor(() => {
      expect(removeSchedule).toHaveBeenCalledWith(plan)
    })
    expect(await screen.findByText('Removed from route test.')).toBeVisible()
  })

  test('routes top-level tab shortcuts and direct execution side effects', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedInitializedSnapshot()
    const scheduleT = createNamespaceTranslator('en', 'schedule')
    const enT = createTranslator('en')
    const plan = {
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
      manualSteps: ['Review the LaunchAgent install.'],
      applyCommands: [['launchctl', 'bootstrap']],
      rollbackCommands: [['launchctl', 'bootout']],
      applySupported: true,
    }
    const status = {
      platform: 'macos' as const,
      label: 'com.yi-ting.pathkeep.backup',
      dueAfterHours: 72,
      checkIntervalHours: 6,
      applySupported: true,
      installState: 'installed' as const,
      detectedFiles: [
        '~/Library/LaunchAgents/com.yi-ting.pathkeep.backup.plist',
      ],
      manualSteps: ['Remove the LaunchAgent if you no longer want automation.'],
      auditPath: '/Users/test/AppData/schedule-audit.json',
      lastSuccessfulBackupAt: '2026-04-10T12:00:00Z',
      warnings: ['Existing schedule was refreshed after the last backup.'],
    }
    backendTestHarness.seedSchedule(plan, status)
    const applySchedule = vi.spyOn(backend, 'applySchedule').mockResolvedValue({
      applied: true,
      platform: 'macos',
      files: ['~/Library/LaunchAgents/com.yi-ting.pathkeep.backup.plist'],
      auditPath: '/Users/test/AppData/apply-audit.json',
      message: 'Applied from route test.',
    })
    const removeSchedule = vi
      .spyOn(backend, 'removeSchedule')
      .mockRejectedValue(new Error('remove denied'))
    const openPath = vi
      .spyOn(backend, 'openPathInFileManager')
      .mockResolvedValue('/Users/test/AppData/apply-audit.json')
    const writeText = vi
      .spyOn(navigator.clipboard, 'writeText')
      .mockResolvedValue(undefined)

    renderTrustPage(<SchedulePage />, {
      language: 'en',
      route: '/schedule',
      snapshot,
    })

    await screen.findByTestId('schedule-page')
    await user.click(
      screen.getAllByRole('button', { name: enT('common.manualTab') })[0],
    )
    expect(
      await screen.findByText(
        'Remove the LaunchAgent if you no longer want automation.',
      ),
    ).toBeVisible()
    await user.click(
      screen.getAllByRole('button', { name: enT('common.previewTab') })[0],
    )
    expect(await screen.findByText(scheduleT('previewBoundary'))).toBeVisible()
    await user.click(
      screen.getAllByRole('button', { name: enT('common.verifyTab') })[0],
    )
    expect(
      (
        await screen.findAllByText(
          'Existing schedule was refreshed after the last backup.',
        )
      )[0],
    ).toBeVisible()
    await user.click(
      screen.getAllByRole('button', { name: enT('common.executeTab') })[0],
    )

    await user.click(
      await screen.findByRole('button', { name: scheduleT('applySchedule') }),
    )

    await waitFor(() => {
      expect(applySchedule).toHaveBeenCalledWith(plan)
    })
    expect(waitForNextPaint).toHaveBeenCalled()
    expect(await screen.findByText('Applied from route test.')).toBeVisible()

    await user.click(
      screen.getByRole('button', { name: enT('common.openPath') }),
    )
    await user.click(
      screen.getByRole('button', { name: enT('common.copyAction') }),
    )
    expect(openPath).toHaveBeenCalledWith(
      '/Users/test/AppData/apply-audit.json',
    )
    expect(writeText).toHaveBeenCalledWith(
      '/Users/test/AppData/apply-audit.json',
    )

    await user.click(
      screen.getByRole('button', { name: scheduleT('removeSchedule') }),
    )
    await waitFor(() => {
      expect(removeSchedule).toHaveBeenCalledWith(plan)
    })
    expect(await screen.findByRole('alert')).toHaveTextContent('remove denied')
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
    manualSteps: ['Review the LaunchAgent install.'],
    applyCommands: [['launchctl', 'bootstrap']],
    rollbackCommands: [['launchctl', 'bootout']],
    applySupported: true,
  }
}

function scheduleStatusFixture() {
  return {
    platform: 'macos' as const,
    label: 'com.yi-ting.pathkeep.backup',
    dueAfterHours: 72,
    checkIntervalHours: 6,
    applySupported: true,
    installState: 'installed' as const,
    detectedFiles: ['~/Library/LaunchAgents/com.yi-ting.pathkeep.backup.plist'],
    manualSteps: ['Remove the LaunchAgent if you no longer want automation.'],
    auditPath: null,
    lastSuccessfulBackupAt: null,
    warnings: [],
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
