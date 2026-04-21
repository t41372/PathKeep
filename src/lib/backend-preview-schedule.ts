/**
 * @file backend-preview-schedule.ts
 * @description Browser-preview schedule helpers that keep manual-review scheduler payloads consistent across Settings, Schedule, and tests.
 * @module lib/backend-preview-schedule
 *
 * ## Responsibilities
 * - Build deterministic schedule plans for each supported desktop platform in browser-preview mode.
 * - Derive truthful read-only schedule status payloads when preview mode cannot inspect native installers.
 * - Let tests override preview schedule plan/status pairs without inventing route-local mock state.
 *
 * ## Not responsible for
 * - Applying or removing native schedules on the host machine.
 * - Dispatching backend commands; `backend.ts` still owns the preview command switch.
 * - Owning unrelated security, import, or retention workflow fixtures.
 *
 * ## Dependencies
 * - Depends on the shared preview state shape from `./backend-preview-state`.
 * - Depends on typed schedule contracts from `./types`.
 *
 * ## Performance notes
 * - These helpers run synchronously during preview command handling, so they stay deterministic and bounded to small static payloads.
 */

import type { MockBackendState } from './backend-preview-state'
import type { SchedulePlan, ScheduleStatus } from './types'

/**
 * Coerces preview schedule requests into the small set of platforms this fixture surface knows how to describe.
 *
 * That keeps the rest of the helpers simple and makes fallback behavior explicit when tests pass unknown values.
 */
export function normalizeMockPlatform(
  platform?: unknown,
): 'macos' | 'windows' | 'linux' {
  if (platform === 'windows') return 'windows'
  if (platform === 'linux') return 'linux'
  return 'macos'
}

/**
 * Builds the manual-review schedule artifact bundle that the UI previews before native installation.
 *
 * Browser preview cannot talk to LaunchAgents, Task Scheduler, or systemd, so this helper mirrors the
 * desktop contract with deterministic files and commands rather than leaving each route to improvise.
 */
export function buildMockSchedulePlan(platform?: unknown): SchedulePlan {
  const resolvedPlatform = normalizeMockPlatform(platform)
  if (resolvedPlatform === 'windows') {
    return {
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
            '<Task><Triggers><TimeTrigger /></Triggers><Settings><StartWhenAvailable>true</StartWhenAvailable></Settings></Task>',
        },
      ],
      manualSteps: [
        'Review the generated Task Scheduler XML.',
        'Import it manually in Task Scheduler if you do not want PathKeep to apply it.',
      ],
      applyCommands: [
        ['schtasks', '/Create', '/XML', 'com.yi-ting.pathkeep.task.xml'],
      ],
      rollbackCommands: [
        ['schtasks', '/Delete', '/TN', 'com.yi-ting.pathkeep.backup', '/F'],
      ],
      applySupported: false,
    }
  }

  if (resolvedPlatform === 'linux') {
    return {
      platform: 'linux',
      label: 'com.yi-ting.pathkeep.backup',
      executablePath: '/usr/bin/pathkeep',
      generatedFiles: [
        {
          relativePath: 'schedule/com.yi-ting.pathkeep.service',
          absolutePath:
            '/home/test/.config/systemd/user/com.yi-ting.pathkeep.service',
          purpose: 'systemd user service',
          contents:
            '[Unit]\nDescription=PathKeep backup\n[Service]\nExecStart=/usr/bin/pathkeep backup',
        },
        {
          relativePath: 'schedule/com.yi-ting.pathkeep.timer',
          absolutePath:
            '/home/test/.config/systemd/user/com.yi-ting.pathkeep.timer',
          purpose: 'systemd user timer',
          contents:
            '[Timer]\nOnCalendar=hourly\nPersistent=true\n[Install]\nWantedBy=timers.target',
        },
      ],
      manualSteps: [
        'Review the generated systemd user unit files.',
        'Copy them into ~/.config/systemd/user and run systemctl --user daemon-reload.',
      ],
      applyCommands: [
        [
          'systemctl',
          '--user',
          'enable',
          '--now',
          'com.yi-ting.pathkeep.timer',
        ],
      ],
      rollbackCommands: [
        [
          'systemctl',
          '--user',
          'disable',
          '--now',
          'com.yi-ting.pathkeep.timer',
        ],
      ],
      applySupported: false,
    }
  }

  return {
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
    manualSteps: [
      'Open the desktop build to verify the LaunchAgent artifact and install status.',
    ],
    applyCommands: [
      [
        'launchctl',
        'bootstrap',
        'gui/501',
        'com.yi-ting.pathkeep.backup.plist',
      ],
    ],
    rollbackCommands: [
      ['launchctl', 'bootout', 'gui/501', 'com.yi-ting.pathkeep.backup'],
    ],
    applySupported: false,
  }
}

/**
 * Reconstructs the truthful degraded schedule status that browser-preview can show without native host access.
 *
 * The UI still needs one consistent payload for warnings, install state, and next steps, even when preview mode
 * cannot inspect real platform state.
 */
export function buildMockScheduleStatus(
  state: MockBackendState,
  platform?: unknown,
): ScheduleStatus {
  const resolvedPlatform = normalizeMockPlatform(platform)
  return {
    platform: resolvedPlatform,
    label: 'com.yi-ting.pathkeep.backup',
    dueAfterHours: state.snapshot.config.dueAfterHours,
    checkIntervalHours: state.snapshot.config.scheduleCheckIntervalHours,
    applySupported: false,
    installState: 'manual-review',
    detectedFiles: [],
    manualSteps:
      resolvedPlatform === 'windows'
        ? [
            'Browser preview mode cannot inspect Task Scheduler directly.',
            'Review the XML, then import it manually if you want to test the plan.',
          ]
        : resolvedPlatform === 'linux'
          ? [
              'Browser preview mode cannot inspect systemd user services directly.',
              'Review the generated units, then run the documented systemctl --user commands manually.',
            ]
          : [
              'Browser preview mode cannot inspect the installed native schedule state.',
              'Open the desktop build to verify the LaunchAgent artifact and install status.',
            ],
    auditPath: null,
    lastSuccessfulBackupAt: state.snapshot.archiveStatus.lastSuccessfulBackupAt,
    warnings: [
      resolvedPlatform === 'windows'
        ? 'Browser preview mode keeps Task Scheduler verification read-only. Use the desktop app or Task Scheduler to inspect the real install state.'
        : resolvedPlatform === 'linux'
          ? 'Browser preview mode keeps systemd verification read-only. Use the desktop app or systemctl --user to inspect the real install state.'
          : 'Browser preview mode keeps schedule verification read-only. Use the desktop app for the real platform status.',
    ],
  }
}

/**
 * Stores a coherent schedule plan/status pair in preview state after tests override the default fixture.
 *
 * Keeping this logic here prevents route tests from patching the two override maps inconsistently and accidentally
 * creating impossible preview combinations.
 */
export function overrideMockSchedule(
  state: MockBackendState,
  plan: SchedulePlan,
  status?: ScheduleStatus,
) {
  const resolvedPlanPlatform = normalizeMockPlatform(plan.platform)
  state.schedulePlanOverrides[resolvedPlanPlatform] = structuredClone(plan)
  if (status) {
    const resolvedStatusPlatform = normalizeMockPlatform(status.platform)
    state.scheduleStatusOverrides[resolvedStatusPlatform] =
      structuredClone(status)
    return
  }

  const fallbackStatus = buildMockScheduleStatus(state, plan.platform)
  const resolvedStatus: ScheduleStatus = {
    ...fallbackStatus,
    platform: plan.platform,
    label: plan.label,
    applySupported: plan.applySupported,
    detectedFiles: plan.generatedFiles
      .map((file) => file.absolutePath ?? file.relativePath)
      .filter((value): value is string => Boolean(value)),
    manualSteps:
      plan.manualSteps.length > 0
        ? structuredClone(plan.manualSteps)
        : fallbackStatus.manualSteps,
    installState: plan.applySupported ? 'installed' : 'manual-review',
    warnings: [],
  }
  state.scheduleStatusOverrides[resolvedPlanPlatform] = resolvedStatus
}
