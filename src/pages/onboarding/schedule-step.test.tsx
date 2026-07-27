/**
 * @file schedule-step.test.tsx
 * @description Presentational coverage for the onboarding schedule step.
 * @module pages/onboarding
 *
 * ## Responsibilities
 * - Verify interval chip selections are forwarded to the route owner.
 * - Keep schedule preview rendering covered outside the full onboarding flow.
 *
 * ## Not responsible for
 * - Re-testing schedule preview command behavior.
 * - Re-testing the full onboarding route state machine.
 *
 * ## Dependencies
 * - Uses the shipped i18n provider because the component reads onboarding copy directly.
 *
 * ## Performance notes
 * - Pure render and click coverage only.
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import type { SchedulePlan, ScheduleStatus } from '../../lib/types'
import { ScheduleStep } from './schedule-step'

describe('ScheduleStep', () => {
  test('forwards interval chip selections', async () => {
    const user = userEvent.setup()
    const onSelectDueAfterHours = vi.fn()

    render(
      <I18nProvider>
        <ScheduleStep
          busyAction={null}
          dueAfterHours={24}
          onBack={vi.fn()}
          onInstallSchedule={vi.fn()}
          onSelectDueAfterHours={onSelectDueAfterHours}
          onSkipSchedule={vi.fn()}
          schedulePlan={null}
          schedulePreviewError={null}
          schedulePreviewLoading={false}
          scheduleStatus={null}
        />
      </I18nProvider>,
    )

    const twelveHourChip = screen
      .getAllByRole('button')
      .find((button) => button.textContent?.includes('12'))
    expect(twelveHourChip).toBeInstanceOf(HTMLButtonElement)
    await user.click(twelveHourChip as HTMLButtonElement)

    expect(onSelectDueAfterHours).toHaveBeenCalledWith(12)
  })

  test('forwards custom interval input changes', async () => {
    const user = userEvent.setup()
    const onSelectDueAfterHours = vi.fn()

    render(
      <I18nProvider>
        <ScheduleStep
          busyAction={null}
          dueAfterHours={24}
          onBack={vi.fn()}
          onInstallSchedule={vi.fn()}
          onSelectDueAfterHours={onSelectDueAfterHours}
          onSkipSchedule={vi.fn()}
          schedulePlan={null}
          schedulePreviewError={null}
          schedulePreviewLoading={false}
          scheduleStatus={null}
        />
      </I18nProvider>,
    )

    const customInput = screen.getByLabelText('Custom interval')
    await user.clear(customInput)
    await user.type(customInput, '90')

    expect(onSelectDueAfterHours).toHaveBeenLastCalledWith(1.5)
  })

  test('renders schedule preview loading and error states', () => {
    const { rerender } = render(
      <I18nProvider>
        <ScheduleStep
          busyAction={null}
          dueAfterHours={24}
          onBack={vi.fn()}
          onInstallSchedule={vi.fn()}
          onSelectDueAfterHours={vi.fn()}
          onSkipSchedule={vi.fn()}
          schedulePlan={null}
          schedulePreviewError={null}
          schedulePreviewLoading
          scheduleStatus={null}
        />
      </I18nProvider>,
    )

    expect(screen.getByText('Generating schedule preview…')).toBeVisible()

    rerender(
      <I18nProvider>
        <ScheduleStep
          busyAction={null}
          dueAfterHours={24}
          onBack={vi.fn()}
          onInstallSchedule={vi.fn()}
          onSelectDueAfterHours={vi.fn()}
          onSkipSchedule={vi.fn()}
          schedulePlan={null}
          schedulePreviewError="preview failed"
          schedulePreviewLoading={false}
          scheduleStatus={null}
        />
      </I18nProvider>,
    )

    expect(screen.getByRole('alert')).toHaveTextContent('preview failed')
  })

  test('labels schedule states that need attention', () => {
    const { rerender } = render(
      <I18nProvider>
        <ScheduleStep
          busyAction={null}
          dueAfterHours={24}
          onBack={vi.fn()}
          onInstallSchedule={vi.fn()}
          onSelectDueAfterHours={vi.fn()}
          onSkipSchedule={vi.fn()}
          schedulePlan={schedulePlanFixture()}
          schedulePreviewError={null}
          schedulePreviewLoading={false}
          scheduleStatus={scheduleStatusFixture('legacy-install-detected')}
        />
      </I18nProvider>,
    )

    expect(screen.getByText('Needs attention')).toBeVisible()

    rerender(
      <I18nProvider>
        <ScheduleStep
          busyAction={null}
          dueAfterHours={24}
          onBack={vi.fn()}
          onInstallSchedule={vi.fn()}
          onSelectDueAfterHours={vi.fn()}
          onSkipSchedule={vi.fn()}
          schedulePlan={schedulePlanFixture()}
          schedulePreviewError={null}
          schedulePreviewLoading={false}
          scheduleStatus={scheduleStatusFixture('installed')}
        />
      </I18nProvider>,
    )
    expect(screen.getByText('Installed')).toBeVisible()

    rerender(
      <I18nProvider>
        <ScheduleStep
          busyAction={null}
          dueAfterHours={24}
          onBack={vi.fn()}
          onInstallSchedule={vi.fn()}
          onSelectDueAfterHours={vi.fn()}
          onSkipSchedule={vi.fn()}
          schedulePlan={schedulePlanFixture()}
          schedulePreviewError={null}
          schedulePreviewLoading={false}
          scheduleStatus={scheduleStatusFixture('not-installed')}
        />
      </I18nProvider>,
    )
    expect(screen.getByText('Not installed')).toBeVisible()

    rerender(
      <I18nProvider>
        <ScheduleStep
          busyAction={null}
          dueAfterHours={24}
          onBack={vi.fn()}
          onInstallSchedule={vi.fn()}
          onSelectDueAfterHours={vi.fn()}
          onSkipSchedule={vi.fn()}
          schedulePlan={schedulePlanFixture()}
          schedulePreviewError={null}
          schedulePreviewLoading={false}
          scheduleStatus={scheduleStatusFixture('manual-review')}
        />
      </I18nProvider>,
    )
    expect(screen.getByText('Manual setup')).toBeVisible()

    rerender(
      <I18nProvider>
        <ScheduleStep
          busyAction={null}
          dueAfterHours={24}
          onBack={vi.fn()}
          onInstallSchedule={vi.fn()}
          onSelectDueAfterHours={vi.fn()}
          onSkipSchedule={vi.fn()}
          schedulePlan={schedulePlanFixture()}
          schedulePreviewError={null}
          schedulePreviewLoading={false}
          scheduleStatus={null}
        />
      </I18nProvider>,
    )
    expect(screen.queryByText('Install state')).not.toBeInTheDocument()
  })

  test('renders typed manual step details instead of backend English prose', () => {
    render(
      <I18nProvider>
        <ScheduleStep
          busyAction={null}
          dueAfterHours={24}
          onBack={vi.fn()}
          onInstallSchedule={vi.fn()}
          onSelectDueAfterHours={vi.fn()}
          onSkipSchedule={vi.fn()}
          schedulePlan={{
            ...schedulePlanFixture(),
            // Prose that no onboarding matcher could ever recognise; the typed
            // details channel is what the step must render.
            manualSteps: ['Save the plist somewhere (drifted prose).'],
            manualStepDetails: [
              {
                id: 'macos-save-plist',
                titleKey: 'schedule.manualMacosSavePlistTitle',
                summaryKey: 'schedule.manualMacosSavePlistSummary',
                whyKey: 'schedule.manualMacosSavePlistWhy',
                command: ['launchctl', 'bootstrap', 'gui/501 path with space'],
                canAutoRun: true,
                canVerify: true,
              },
              {
                id: 'macos-load',
                titleKey: 'schedule.manualMacosLoadTitle',
                summaryKey: 'schedule.manualMacosLoadSummary',
                whyKey: 'schedule.manualMacosLoadWhy',
                command: null,
                canAutoRun: false,
                canVerify: true,
              },
            ],
          }}
          schedulePreviewError={null}
          schedulePreviewLoading={false}
          scheduleStatus={null}
        />
      </I18nProvider>,
    )

    expect(screen.getByText('Save the LaunchAgent plist')).toBeVisible()
    expect(
      screen.getByText(
        'Create or replace the plist in your user LaunchAgents folder.',
      ),
    ).toBeVisible()
    expect(
      screen.getByText('launchctl bootstrap "gui/501 path with space"'),
    ).toBeVisible()
    // A step without a command renders its copy and nothing else.
    expect(screen.getByText('Load the LaunchAgent')).toBeVisible()
    expect(
      screen.getByText(
        'Register the plist with launchd for the current user session.',
      ),
    ).toBeVisible()
    expect(
      screen.queryByText('Save the plist somewhere (drifted prose).'),
    ).toBeNull()
  })

  test('falls back to raw manual steps when the backend sends no typed details', () => {
    render(
      <I18nProvider>
        <ScheduleStep
          busyAction={null}
          dueAfterHours={24}
          onBack={vi.fn()}
          onInstallSchedule={vi.fn()}
          onSelectDueAfterHours={vi.fn()}
          onSkipSchedule={vi.fn()}
          schedulePlan={{
            ...schedulePlanFixture(),
            manualSteps: ['A step only an older backend knows about.'],
          }}
          schedulePreviewError={null}
          schedulePreviewLoading={false}
          scheduleStatus={null}
        />
      </I18nProvider>,
    )

    expect(
      screen.getByText('A step only an older backend knows about.'),
    ).toBeVisible()
  })
})

function schedulePlanFixture(): SchedulePlan {
  return {
    applyCommands: [],
    applySupported: true,
    executablePath: '/Applications/PathKeep.app',
    generatedFiles: [],
    label: 'PathKeep backup',
    manualSteps: [],
    platform: 'macos',
    rollbackCommands: [],
  }
}

function scheduleStatusFixture(
  installState: ScheduleStatus['installState'],
): ScheduleStatus {
  return {
    applySupported: true,
    auditPath: null,
    checkIntervalHours: 6,
    detectedFiles: [],
    dueAfterHours: 24,
    installState,
    label: 'PathKeep backup',
    lastSuccessfulBackupAt: null,
    manualSteps: [],
    platform: 'macos',
    warnings: [],
  }
}
