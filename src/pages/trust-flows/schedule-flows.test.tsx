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

import { screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'

const { invoke, isTauri, subscribeToImportProgress } = vi.hoisted(() => ({
  invoke: vi.fn(),
  isTauri: vi.fn(() => false),
  subscribeToImportProgress: vi.fn(() => Promise.resolve(vi.fn())),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke,
  isTauri,
}))

vi.mock('../../lib/ipc/import-progress', () => ({
  subscribeToImportProgress,
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
})
