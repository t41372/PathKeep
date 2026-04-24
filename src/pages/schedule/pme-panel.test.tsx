/**
 * @file pme-panel.test.tsx
 * @description Focused coverage for the extracted Schedule PME panel.
 * @module pages/schedule
 *
 * ## Responsibilities
 * - Verify Preview, Execute, and Verify tab rendering for the extracted PME owner.
 * - Keep direct apply/remove affordances and verify-path review rows honest without mounting the full route.
 *
 * ## Not responsible for
 * - Re-testing route-level loading, platform callouts, or backend fetch orchestration.
 * - Re-testing shell-owned refresh behavior.
 *
 * ## Dependencies
 * - Depends on the shipped root translator for schedule/common/audit strings.
 *
 * ## Performance notes
 * - Focused render tests avoid replaying the full schedule route for render-only PME coverage.
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, test, vi } from 'vitest'
import { createTranslator } from '../../lib/i18n'
import type { SchedulePlan, ScheduleStatus } from '../../lib/types'
import { SchedulePmePanel } from './pme-panel'

const t = createTranslator('en')

const schedulePlan: SchedulePlan = {
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
}

const scheduleStatus: ScheduleStatus = {
  platform: 'macos',
  label: 'com.yi-ting.pathkeep.backup',
  dueAfterHours: 72,
  checkIntervalHours: 6,
  applySupported: true,
  installState: 'installed',
  detectedFiles: ['~/Library/LaunchAgents/com.yi-ting.pathkeep.backup.plist'],
  manualSteps: ['Remove the LaunchAgent if you no longer want automation.'],
  auditPath: '/Users/test/AppData/schedule-audit.json',
  lastSuccessfulBackupAt: '2026-04-10T12:00:00Z',
  warnings: [],
}

describe('schedule pme panel', () => {
  test('renders preview generated files and manual review rows', () => {
    const onCopyValue = vi.fn().mockResolvedValue(undefined)
    const onOpenPath = vi.fn().mockResolvedValue(undefined)
    const { rerender } = render(
      <SchedulePmePanel
        actionError={null}
        busy={null}
        copyFeedback={null}
        executionResult={null}
        installDescription={t('schedule.installedDescription')}
        lastBackupLabel="3 days ago"
        latestAuditPath={scheduleStatus.auditPath ?? null}
        onApply={vi.fn()}
        onCopyValue={onCopyValue}
        onOpenPath={onOpenPath}
        onRemove={vi.fn()}
        plan={schedulePlan}
        pmeTab="preview"
        setPmeTab={vi.fn()}
        snapshotInitialized={true}
        status={scheduleStatus}
        t={t}
      />,
    )
    expect(screen.getByText(t('schedule.previewBoundary'))).toBeVisible()
    expect(screen.getAllByText('LaunchAgent plist')[0]).toBeVisible()

    rerender(
      <SchedulePmePanel
        actionError={null}
        busy={null}
        copyFeedback={null}
        executionResult={null}
        installDescription={t('schedule.installedDescription')}
        lastBackupLabel="3 days ago"
        latestAuditPath={scheduleStatus.auditPath ?? null}
        onApply={vi.fn()}
        onCopyValue={onCopyValue}
        onOpenPath={onOpenPath}
        onRemove={vi.fn()}
        plan={schedulePlan}
        pmeTab="manual"
        setPmeTab={vi.fn()}
        snapshotInitialized={true}
        status={scheduleStatus}
        t={t}
      />,
    )

    expect(
      screen.getByText(
        'Remove the LaunchAgent if you no longer want automation.',
      ),
    ).toBeVisible()
    expect(screen.getByText(scheduleStatus.auditPath ?? '')).toBeVisible()
  })

  test('renders execute and verify states with direct controls and latest audit path', async () => {
    const user = userEvent.setup()
    const onApply = vi.fn()
    const onRemove = vi.fn()
    const onCopyValue = vi.fn().mockResolvedValue(undefined)
    const onOpenPath = vi.fn().mockResolvedValue(undefined)

    const { rerender } = render(
      <SchedulePmePanel
        actionError={null}
        busy={null}
        copyFeedback={null}
        executionResult={{
          mode: 'apply',
          result: {
            applied: true,
            platform: 'macos',
            files: ['~/Library/LaunchAgents/com.yi-ting.pathkeep.backup.plist'],
            auditPath: '/Users/test/AppData/schedule-audit.json',
            message: 'Applied schedule successfully.',
          },
        }}
        installDescription={t('schedule.installedDescription')}
        lastBackupLabel="3 days ago"
        latestAuditPath={scheduleStatus.auditPath ?? null}
        onApply={onApply}
        onCopyValue={onCopyValue}
        onOpenPath={onOpenPath}
        onRemove={onRemove}
        plan={schedulePlan}
        pmeTab="execute"
        setPmeTab={vi.fn()}
        snapshotInitialized={true}
        status={scheduleStatus}
        t={t}
      />,
    )

    expect(screen.getByText('launchctl bootstrap')).toBeVisible()
    expect(
      screen.getByRole('button', { name: t('schedule.applySchedule') }),
    ).toBeEnabled()
    expect(
      screen.getByRole('button', { name: t('schedule.removeSchedule') }),
    ).toBeEnabled()

    await user.click(
      screen.getByRole('button', { name: t('schedule.applySchedule') }),
    )
    await user.click(
      screen.getByRole('button', { name: t('schedule.removeSchedule') }),
    )

    expect(onApply).toHaveBeenCalledTimes(1)
    expect(onRemove).toHaveBeenCalledTimes(1)

    rerender(
      <SchedulePmePanel
        actionError={null}
        busy={null}
        copyFeedback={null}
        executionResult={null}
        installDescription={t('schedule.installedDescription')}
        lastBackupLabel="3 days ago"
        latestAuditPath={scheduleStatus.auditPath ?? null}
        onApply={onApply}
        onCopyValue={onCopyValue}
        onOpenPath={onOpenPath}
        onRemove={onRemove}
        plan={schedulePlan}
        pmeTab="verify"
        setPmeTab={vi.fn()}
        snapshotInitialized={true}
        status={scheduleStatus}
        t={t}
      />,
    )

    expect(screen.getAllByText(t('common.verifyTab')).length).toBeGreaterThan(0)
    expect(screen.getByText('3 days ago')).toBeVisible()
    expect(screen.getByText(scheduleStatus.auditPath ?? '')).toBeVisible()
  })
})
