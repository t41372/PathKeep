import { describe, expect, test, vi } from 'vitest'
import {
  dueAfterOptions,
  localizeScheduleManualStep,
  onboardingStepKeys,
  schedulePlatformLabel,
} from './shared'

describe('onboarding shared helpers', () => {
  test('exports the stable onboarding step and backup interval contracts', () => {
    expect(onboardingStepKeys).toEqual([
      'stepWelcome',
      'stepBrowsers',
      'stepStorage',
      'stepSecurity',
      'stepSchedule',
      'stepReady',
    ])
    expect(dueAfterOptions).toEqual([6, 12, 24, 72])
  })

  test('localizes known scheduler platforms and preserves unknown platform ids', () => {
    const t = vi.fn((key: string) => `translated:${key}`)

    expect(schedulePlatformLabel('macos', t)).toBe(
      'translated:platform.macosLabel',
    )
    expect(schedulePlatformLabel('windows', t)).toBe(
      'translated:platform.windowsLabel',
    )
    expect(schedulePlatformLabel('linux', t)).toBe(
      'translated:platform.linuxLabel',
    )
    expect(schedulePlatformLabel('freebsd', t)).toBe('freebsd')
  })

  test('rewrites backend scheduler manual steps into onboarding-owned copy', () => {
    const t = vi.fn((key: string, vars?: Record<string, string | number>) =>
      vars?.label ? `${key}:${vars.label}` : key,
    )
    const label = 'com.pathkeep.backup'

    expect(
      localizeScheduleManualStep(
        `Save the plist to ~/Library/LaunchAgents/${label}.plist.`,
        label,
        t,
      ),
    ).toBe(`scheduleManualStepLaunchAgentSave:${label}`)
    expect(
      localizeScheduleManualStep(
        `Run \`launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/${label}.plist\` to load the new schedule.`,
        label,
        t,
      ),
    ).toBe(`scheduleManualStepLaunchAgentBootstrap:${label}`)
    expect(
      localizeScheduleManualStep(
        'Open the desktop build to verify the LaunchAgent artifact and install status.',
        label,
        t,
      ),
    ).toBe('scheduleManualStepLaunchAgentReviewInstalled')
    expect(
      localizeScheduleManualStep(
        'Remove the LaunchAgent if you no longer want automation.',
        label,
        t,
      ),
    ).toBe('scheduleManualStepLaunchAgentRemove')
    expect(
      localizeScheduleManualStep(
        'Save the XML file and import it in Task Scheduler.',
        label,
        t,
      ),
    ).toBe('scheduleManualStepWindowsSaveXml')
    expect(
      localizeScheduleManualStep(
        `Alternatively run \`schtasks /Create /TN ${label} /XML ${label}.task.xml\`.`,
        label,
        t,
      ),
    ).toBe(`scheduleManualStepWindowsCreateTask:${label}`)
    expect(
      localizeScheduleManualStep(
        'Copy the files to ~/.config/systemd/user/.',
        label,
        t,
      ),
    ).toBe('scheduleManualStepLinuxCopy')
    expect(
      localizeScheduleManualStep(
        'Run `systemctl --user daemon-reload`.',
        label,
        t,
      ),
    ).toBe('scheduleManualStepLinuxReload')
    expect(
      localizeScheduleManualStep(
        `Run \`systemctl --user enable --now ${label}.timer\`.`,
        label,
        t,
      ),
    ).toBe(`scheduleManualStepLinuxEnable:${label}`)
    expect(
      localizeScheduleManualStep(
        `Run \`systemctl --user list-timers ${label}.timer\` to verify the next scheduled run.`,
        label,
        t,
      ),
    ).toBe(`scheduleManualStepLinuxVerify:${label}`)
    expect(
      localizeScheduleManualStep('Keep custom backend note.', label, t),
    ).toBe('Keep custom backend note.')
  })
})
