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

import { render, screen, within } from '@testing-library/react'
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

function renderPanel({
  actionError = null,
  busy = null,
  copyFeedback = null,
  executionResult = null,
  installDescription = t('schedule.installedDescription'),
  lastBackupLabel = '3 days ago',
  latestAuditPath = scheduleStatus.auditPath ?? null,
  onApply = vi.fn(),
  onCopyValue = vi.fn().mockResolvedValue(undefined),
  onOpenPath = vi.fn().mockResolvedValue(undefined),
  onRemove = vi.fn(),
  plan = schedulePlan,
  pmeTab = 'preview',
  setPmeTab = vi.fn(),
  snapshotInitialized = true,
  status = scheduleStatus,
}: Partial<Parameters<typeof SchedulePmePanel>[0]> = {}) {
  return render(
    <SchedulePmePanel
      actionError={actionError}
      busy={busy}
      copyFeedback={copyFeedback}
      executionResult={executionResult}
      installDescription={installDescription}
      lastBackupLabel={lastBackupLabel}
      latestAuditPath={latestAuditPath}
      onApply={onApply}
      onCopyValue={onCopyValue}
      onOpenPath={onOpenPath}
      onRemove={onRemove}
      plan={plan}
      pmeTab={pmeTab}
      setPmeTab={setPmeTab}
      snapshotInitialized={snapshotInitialized}
      status={status}
      t={t}
    />,
  )
}

describe('schedule pme panel', () => {
  test('renders preview generated files, fallback, and tab changes', async () => {
    const user = userEvent.setup()
    const onCopyValue = vi.fn().mockResolvedValue(undefined)
    const onOpenPath = vi.fn().mockResolvedValue(undefined)
    const setPmeTab = vi.fn()
    const { rerender } = renderPanel({
      onCopyValue,
      onOpenPath,
      setPmeTab,
    })

    expect(screen.getByText(t('schedule.previewBoundary'))).toBeVisible()
    expect(screen.getAllByText('LaunchAgent plist')[0]).toBeVisible()

    await user.click(
      screen.getByRole('button', { name: t('common.manualTab') }),
    )
    await user.click(
      screen.getByRole('button', { name: t('common.executeTab') }),
    )
    await user.click(
      screen.getByRole('button', { name: t('common.verifyTab') }),
    )
    expect(setPmeTab).toHaveBeenNthCalledWith(1, 'manual')
    expect(setPmeTab).toHaveBeenNthCalledWith(2, 'execute')
    expect(setPmeTab).toHaveBeenNthCalledWith(3, 'verify')

    const previewDetails = screen.getByText('LaunchAgent plist', {
      selector: 'summary',
    })
    await user.click(previewDetails)
    await user.click(
      within(
        previewDetails.closest('.code-panel') ?? document.body,
      ).getAllByRole('button', { name: t('common.copyAction') })[0],
    )
    await user.click(screen.getByRole('button', { name: t('common.openPath') }))
    expect(onCopyValue).toHaveBeenCalledWith(
      'contents:schedule/com.yi-ting.pathkeep.backup.plist',
      schedulePlan.generatedFiles[0]?.contents,
    )
    expect(onOpenPath).toHaveBeenCalledWith(
      schedulePlan.generatedFiles[0]?.absolutePath,
    )

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
        plan={{ ...schedulePlan, generatedFiles: [] }}
        pmeTab="preview"
        setPmeTab={vi.fn()}
        snapshotInitialized={true}
        status={scheduleStatus}
        t={t}
      />,
    )

    expect(screen.getByText(t('schedule.noGeneratedFiles'))).toBeVisible()
  })

  test('renders manual rows and wires detected-file and audit actions', async () => {
    const user = userEvent.setup()
    const onCopyValue = vi.fn().mockResolvedValue(undefined)
    const onOpenPath = vi.fn().mockResolvedValue(undefined)

    const { rerender } = renderPanel({
      onCopyValue,
      onOpenPath,
      pmeTab: 'manual',
    })

    expect(
      screen.getByText(
        'Remove the LaunchAgent if you no longer want automation.',
      ),
    ).toBeVisible()
    expect(screen.getByText(scheduleStatus.auditPath ?? '')).toBeVisible()

    const openButtons = screen.getAllByRole('button', {
      name: t('common.openPath'),
    })
    const copyButtons = screen.getAllByRole('button', {
      name: t('common.copyAction'),
    })

    await user.click(openButtons[0])
    await user.click(copyButtons[0])
    await user.click(openButtons[1])
    await user.click(copyButtons[1])

    expect(onOpenPath).toHaveBeenCalledWith(scheduleStatus.detectedFiles[0])
    expect(onOpenPath).toHaveBeenCalledWith(scheduleStatus.auditPath)
    expect(onCopyValue).toHaveBeenCalledWith(
      `schedule:detected:${scheduleStatus.detectedFiles[0]}`,
      scheduleStatus.detectedFiles[0],
    )
    expect(onCopyValue).toHaveBeenCalledWith(
      'schedule:audit-path',
      scheduleStatus.auditPath,
    )

    rerender(
      <SchedulePmePanel
        actionError={null}
        busy={null}
        copyFeedback={null}
        executionResult={null}
        installDescription={t('schedule.installedDescription')}
        lastBackupLabel="3 days ago"
        latestAuditPath={null}
        onApply={vi.fn()}
        onCopyValue={onCopyValue}
        onOpenPath={onOpenPath}
        onRemove={vi.fn()}
        plan={schedulePlan}
        pmeTab="manual"
        setPmeTab={vi.fn()}
        snapshotInitialized={true}
        status={{ ...scheduleStatus, auditPath: null }}
        t={t}
      />,
    )
    expect(
      screen.queryByText(scheduleStatus.auditPath ?? ''),
    ).not.toBeInTheDocument()
  })

  test('renders execute and verify states with direct controls and latest audit path', async () => {
    const user = userEvent.setup()
    const onApply = vi.fn()
    const onRemove = vi.fn()
    const onCopyValue = vi.fn().mockResolvedValue(undefined)
    const onOpenPath = vi.fn().mockResolvedValue(undefined)

    const executionResult = {
      mode: 'apply' as const,
      result: {
        applied: true,
        platform: 'macos' as const,
        files: ['~/Library/LaunchAgents/com.yi-ting.pathkeep.backup.plist'],
        auditPath: '/Users/test/AppData/schedule-audit.json',
        message: 'Applied schedule successfully.',
      },
    }
    const { rerender } = renderPanel({
      executionResult,
      onApply,
      onCopyValue,
      onOpenPath,
      onRemove,
      pmeTab: 'execute',
      plan: {
        ...schedulePlan,
        applyCommands: [['launchctl', 'bootstrap', 'path with space']],
      },
    })

    expect(
      screen.getByText('launchctl bootstrap "path with space"'),
    ).toBeVisible()
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

    await user.click(screen.getByRole('button', { name: t('common.openPath') }))
    await user.click(
      screen.getByRole('button', { name: t('common.copyAction') }),
    )
    expect(onOpenPath).toHaveBeenCalledWith(executionResult.result.auditPath)
    expect(onCopyValue).toHaveBeenCalledWith(
      'schedule:execution-audit',
      executionResult.result.auditPath,
    )

    rerender(
      <SchedulePmePanel
        actionError={null}
        busy={null}
        copyFeedback={null}
        executionResult={{
          mode: 'remove',
          result: {
            applied: false,
            platform: 'macos',
            files: [],
            auditPath: '/Users/test/AppData/remove-audit.json',
            message: 'Removed schedule successfully.',
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
    expect(
      screen.getAllByText(t('schedule.removeSchedule')).length,
    ).toBeGreaterThan(0)
    expect(screen.getByText('Removed schedule successfully.')).toBeVisible()

    await user.click(
      screen.getAllByRole('button', { name: t('common.openPath') })[0],
    )
    await user.click(
      screen.getAllByRole('button', { name: t('common.copyAction') })[0],
    )
    await user.click(
      screen.getAllByRole('button', { name: t('common.openPath') })[1],
    )
    await user.click(
      screen.getAllByRole('button', { name: t('common.copyAction') })[1],
    )
    expect(onOpenPath).toHaveBeenCalledWith(scheduleStatus.detectedFiles[0])
    expect(onOpenPath).toHaveBeenCalledWith(scheduleStatus.auditPath)
    expect(onCopyValue).toHaveBeenCalledWith(
      `schedule:verify-detected:${scheduleStatus.detectedFiles[0]}`,
      scheduleStatus.detectedFiles[0],
    )
    expect(onCopyValue).toHaveBeenCalledWith(
      'schedule:latest-audit',
      scheduleStatus.auditPath,
    )

    rerender(
      <SchedulePmePanel
        actionError={null}
        busy={null}
        copyFeedback={null}
        executionResult={executionResult}
        installDescription={t('schedule.installedDescription')}
        lastBackupLabel="3 days ago"
        latestAuditPath={null}
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
    expect(screen.getByText('Applied schedule successfully.')).toBeVisible()
  })

  test('renders remove execution results without relying on a rerender transition', () => {
    renderPanel({
      executionResult: {
        mode: 'remove',
        result: {
          applied: false,
          auditPath: '/Users/test/AppData/remove-audit.json',
          files: [],
          message: 'Removed schedule successfully.',
          platform: 'macos',
        },
      },
      pmeTab: 'execute',
    })

    expect(
      screen.getAllByText(t('schedule.removeSchedule')).length,
    ).toBeGreaterThan(0)
    expect(screen.getByText('Removed schedule successfully.')).toBeVisible()
  })

  test('renders execute guardrails for errors, busy labels, and unsupported plans', () => {
    const onApply = vi.fn()
    const onRemove = vi.fn()
    const { rerender } = renderPanel({
      actionError: 'launchctl denied access',
      busy: t('schedule.applySchedule'),
      onApply,
      onRemove,
      pmeTab: 'execute',
    })

    expect(screen.getByRole('alert')).toHaveTextContent(
      'launchctl denied access',
    )
    expect(
      screen.getByRole('button', { name: t('schedule.applySchedule') }),
    ).toBeDisabled()
    expect(
      screen.getByRole('button', { name: t('schedule.removeSchedule') }),
    ).toBeDisabled()

    rerender(
      <SchedulePmePanel
        actionError={null}
        busy={t('schedule.removeSchedule')}
        copyFeedback={null}
        executionResult={null}
        installDescription={t('schedule.notInstalledDescription')}
        lastBackupLabel={t('common.notAvailable')}
        latestAuditPath={null}
        onApply={onApply}
        onCopyValue={vi.fn().mockResolvedValue(undefined)}
        onOpenPath={vi.fn().mockResolvedValue(undefined)}
        onRemove={onRemove}
        plan={{ ...schedulePlan, applySupported: false }}
        pmeTab="execute"
        setPmeTab={vi.fn()}
        snapshotInitialized={false}
        status={{
          ...scheduleStatus,
          auditPath: null,
          detectedFiles: [],
          installState: 'not-installed',
        }}
        t={t}
      />,
    )

    expect(screen.getByText(t('schedule.initializeArchiveFirst'))).toBeVisible()
    expect(
      screen.getByRole('button', { name: t('schedule.applySchedule') }),
    ).toBeDisabled()
    expect(
      screen.getByRole('button', { name: t('schedule.removeSchedule') }),
    ).toBeDisabled()

    rerender(
      <SchedulePmePanel
        actionError={null}
        busy={null}
        copyFeedback={null}
        executionResult={null}
        installDescription={t('schedule.notInstalledDescription')}
        lastBackupLabel={t('common.notAvailable')}
        latestAuditPath={null}
        onApply={onApply}
        onCopyValue={vi.fn().mockResolvedValue(undefined)}
        onOpenPath={vi.fn().mockResolvedValue(undefined)}
        onRemove={onRemove}
        plan={schedulePlan}
        pmeTab="execute"
        setPmeTab={vi.fn()}
        snapshotInitialized={true}
        status={{
          ...scheduleStatus,
          auditPath: null,
          detectedFiles: [],
          installState: 'not-installed',
        }}
        t={t}
      />,
    )
    expect(
      screen.getByRole('button', { name: t('schedule.removeSchedule') }),
    ).toBeDisabled()

    rerender(
      <SchedulePmePanel
        actionError={null}
        busy={null}
        copyFeedback={null}
        executionResult={null}
        installDescription={t('schedule.notInstalledDescription')}
        lastBackupLabel={t('common.notAvailable')}
        latestAuditPath={null}
        onApply={onApply}
        onCopyValue={vi.fn().mockResolvedValue(undefined)}
        onOpenPath={vi.fn().mockResolvedValue(undefined)}
        onRemove={onRemove}
        plan={schedulePlan}
        pmeTab="execute"
        setPmeTab={vi.fn()}
        snapshotInitialized={true}
        status={{
          ...scheduleStatus,
          auditPath: null,
          installState: 'not-installed',
        }}
        t={t}
      />,
    )
    expect(
      screen.getByRole('button', { name: t('schedule.removeSchedule') }),
    ).toBeEnabled()
  })

  test('renders verify attention states, empty file status, and warnings', () => {
    const { rerender } = renderPanel({
      installDescription: t('schedule.manualReviewDescription'),
      latestAuditPath: null,
      pmeTab: 'verify',
      status: {
        ...scheduleStatus,
        auditPath: null,
        detectedFiles: [],
        installState: 'manual-review',
        warnings: ['Review StartWhenAvailable before trusting the schedule.'],
      },
    })

    expect(screen.getByText(t('schedule.manualReviewBadge'))).toBeVisible()
    expect(screen.getByText(t('common.notAvailable'))).toBeVisible()
    expect(
      screen.getByText(
        'Review StartWhenAvailable before trusting the schedule.',
      ),
    ).toBeVisible()
    expect(screen.queryByText(t('common.statusClear'))).not.toBeInTheDocument()

    rerender(
      <SchedulePmePanel
        actionError={null}
        busy={null}
        copyFeedback={null}
        executionResult={null}
        installDescription={t('schedule.notInstalledDescription')}
        lastBackupLabel={t('common.notAvailable')}
        latestAuditPath={null}
        onApply={vi.fn()}
        onCopyValue={vi.fn().mockResolvedValue(undefined)}
        onOpenPath={vi.fn().mockResolvedValue(undefined)}
        onRemove={vi.fn()}
        plan={schedulePlan}
        pmeTab="verify"
        setPmeTab={vi.fn()}
        snapshotInitialized={true}
        status={{
          ...scheduleStatus,
          detectedFiles: [],
          installState: 'not-installed',
          warnings: [],
        }}
        t={t}
      />,
    )
    expect(screen.getByText(t('schedule.notInstalledBadge'))).toBeVisible()
    expect(screen.getByText(t('common.statusClear'))).toBeVisible()

    rerender(
      <SchedulePmePanel
        actionError={null}
        busy={null}
        copyFeedback={null}
        executionResult={null}
        installDescription={t('schedule.mismatchDescription')}
        lastBackupLabel={t('common.notAvailable')}
        latestAuditPath={null}
        onApply={vi.fn()}
        onCopyValue={vi.fn().mockResolvedValue(undefined)}
        onOpenPath={vi.fn().mockResolvedValue(undefined)}
        onRemove={vi.fn()}
        plan={schedulePlan}
        pmeTab="verify"
        setPmeTab={vi.fn()}
        snapshotInitialized={true}
        status={{
          ...scheduleStatus,
          detectedFiles: [],
          installState: 'mismatch',
          warnings: [],
        }}
        t={t}
      />,
    )
    expect(screen.getByText(t('schedule.attentionBadge'))).toBeVisible()
  })
})
