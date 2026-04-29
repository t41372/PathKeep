/**
 * @file index.test.tsx
 * @description Route-shell coverage for the Schedule page loader and mutation wiring.
 * @module pages/schedule
 *
 * ## Responsibilities
 * - Verify loading, unavailable, rendered fallback, and apply/remove mutation branches.
 * - Keep schedule route state covered without mounting the full PME artifact viewer.
 *
 * ## Not responsible for
 * - Re-testing `SchedulePmePanel` render details.
 *
 * ## Dependencies
 * - Mocks shell data, backend schedule commands, i18n, and the PME child panel.
 *
 * ## Performance notes
 * - Uses tiny plan/status fixtures and does not touch launchd or the filesystem.
 */

import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { SchedulePage } from './index'

const { backendMock, refreshAppDataMock, saveConfigMock, useShellDataMock } =
  vi.hoisted(() => ({
    backendMock: {
      applySchedule: vi.fn(),
      openPathInFileManager: vi.fn(),
      previewSchedule: vi.fn(),
      removeSchedule: vi.fn(),
      scheduleStatus: vi.fn(),
    },
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
    t: (key: string) => key,
  }),
}))

vi.mock('../../lib/wait-for-next-paint', () => ({
  waitForNextPaint: () => Promise.resolve(),
}))

vi.mock('./pme-panel', () => ({
  SchedulePmePanel: ({
    actionError,
    latestAuditPath,
    onApply,
    onRemove,
    snapshotInitialized,
  }: {
    actionError: string | null
    latestAuditPath: string | null
    onApply: () => void
    onRemove: () => void
    snapshotInitialized: boolean
  }) => (
    <div data-testid="schedule-pme">
      <span>{snapshotInitialized ? 'snapshot-ready' : 'snapshot-missing'}</span>
      <span>{latestAuditPath ?? 'no-audit'}</span>
      {actionError ? <p role="alert">{actionError}</p> : null}
      <button type="button" onClick={onApply}>
        apply
      </button>
      <button type="button" onClick={onRemove}>
        remove
      </button>
    </div>
  ),
}))

describe('SchedulePage', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    refreshAppDataMock.mockResolvedValue(undefined)
    saveConfigMock.mockImplementation((config) =>
      Promise.resolve({
        config,
      }),
    )
    useShellDataMock.mockReturnValue({
      refreshAppData: refreshAppDataMock,
      refreshKey: 1,
      saveConfig: saveConfigMock,
      snapshot: {
        archiveStatus: {
          lastSuccessfulBackupAt: '2026-04-25T12:00:00.000Z',
        },
        config: {
          dueAfterHours: 12,
          initialized: true,
          scheduleCheckIntervalHours: 24,
          selectedProfileIds: ['chrome:Default'],
        },
      },
    })
    backendMock.previewSchedule.mockResolvedValue(planFixture())
    backendMock.scheduleStatus.mockResolvedValue(statusFixture())
    backendMock.applySchedule.mockResolvedValue(applyResultFixture())
    backendMock.removeSchedule.mockResolvedValue(applyResultFixture())
  })

  test('shows loading before schedule preview resolves, then unavailable fallback copy', async () => {
    backendMock.previewSchedule.mockResolvedValueOnce(null)
    backendMock.scheduleStatus.mockResolvedValueOnce(statusFixture())

    render(<SchedulePage />)

    expect(screen.getByText('schedule.loadingPreview')).toBeVisible()
    expect(await screen.findByText('schedule.unavailableBody')).toBeVisible()
  })

  test('renders without a shell snapshot and keeps profile/audit fallbacks visible', async () => {
    useShellDataMock.mockReturnValue({
      refreshAppData: refreshAppDataMock,
      refreshKey: 1,
      saveConfig: saveConfigMock,
      snapshot: null,
    })
    backendMock.scheduleStatus.mockResolvedValueOnce(
      statusFixture({
        auditPath: null,
        lastSuccessfulBackupAt: null,
      }),
    )

    render(<SchedulePage />)

    expect(await screen.findByTestId('schedule-page')).toBeVisible()
    expect(screen.getAllByText('common.notAvailable').length).toBeGreaterThan(0)
    expect(screen.getByText('snapshot-missing')).toBeVisible()
    expect(screen.getByText('no-audit')).toBeVisible()
  })

  test('surfaces an initialize-first error if apply tries to persist without config', async () => {
    const user = userEvent.setup()
    useShellDataMock.mockReturnValue({
      refreshAppData: refreshAppDataMock,
      refreshKey: 1,
      saveConfig: saveConfigMock,
      snapshot: null,
    })

    render(<SchedulePage />)

    await screen.findByTestId('schedule-page')
    await user.click(
      screen.getAllByRole('button', {
        name: 'schedule.intervalChipLabel',
      })[2],
    )
    await user.click(screen.getByRole('button', { name: 'apply' }))

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'schedule.initializeArchiveFirst',
    )
    expect(saveConfigMock).not.toHaveBeenCalled()
    expect(backendMock.applySchedule).not.toHaveBeenCalled()
  })

  test.each([
    [
      'manual-review',
      'schedule.manualReviewBadge',
      'schedule.manualReviewDescription',
    ],
    [
      'not-installed',
      'schedule.notInstalledBadge',
      'schedule.notInstalledDescription',
    ],
    ['mismatch', 'schedule.attentionBadge', 'schedule.mismatchDescription'],
    [
      'permission-warning',
      'schedule.attentionBadge',
      'schedule.permissionWarningDescription',
    ],
    [
      'legacy-install-detected',
      'schedule.attentionBadge',
      'schedule.legacyInstallDescription',
    ],
  ])('renders %s install state copy', async (installState, badge, body) => {
    backendMock.scheduleStatus.mockResolvedValueOnce(
      statusFixture({ installState }),
    )

    render(<SchedulePage />)

    expect(await screen.findAllByText(badge)).not.toHaveLength(0)
    expect(screen.getAllByText(body)).not.toHaveLength(0)
  })

  test('surfaces apply and remove failures without leaving the route busy', async () => {
    const user = userEvent.setup()
    backendMock.applySchedule.mockRejectedValueOnce('apply failed')
    backendMock.removeSchedule.mockRejectedValueOnce(new Error('remove failed'))

    render(<SchedulePage />)

    await screen.findByTestId('schedule-pme')
    await user.click(screen.getByRole('button', { name: 'apply' }))
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'common.unavailable',
    )

    await user.click(screen.getByRole('button', { name: 'remove' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('remove failed')
  })

  test('saves interval changes before applying the refreshed schedule plan', async () => {
    const user = userEvent.setup()
    const refreshedPlan = {
      ...planFixture(),
      label: 'PathKeep Backup refreshed',
    }
    backendMock.previewSchedule
      .mockResolvedValueOnce(planFixture())
      .mockResolvedValueOnce(refreshedPlan)

    render(<SchedulePage />)

    await screen.findByTestId('schedule-page')
    await user.click(
      screen.getAllByRole('button', {
        name: 'schedule.intervalChipLabel',
      })[2],
    )
    await user.click(
      screen.getByRole('button', {
        name: 'schedule.saveAndUpdateSchedule',
      }),
    )

    expect(saveConfigMock).toHaveBeenCalledWith(
      expect.objectContaining({ dueAfterHours: 24 }),
    )
    expect(backendMock.applySchedule).toHaveBeenCalledWith(refreshedPlan)
  })

  test('uses install wording when interval changes before any schedule is installed', async () => {
    const user = userEvent.setup()
    backendMock.scheduleStatus.mockResolvedValueOnce(
      statusFixture({ installState: 'not-installed' }),
    )

    render(<SchedulePage />)

    await screen.findByTestId('schedule-page')
    await user.click(
      screen.getAllByRole('button', {
        name: 'schedule.intervalChipLabel',
      })[2],
    )

    expect(
      screen.getByRole('button', {
        name: 'schedule.saveAndInstallSchedule',
      }),
    ).toBeVisible()
  })

  test('saves interval changes without applying the native schedule', async () => {
    const user = userEvent.setup()

    render(<SchedulePage />)

    await screen.findByTestId('schedule-page')
    await user.click(
      screen.getAllByRole('button', {
        name: 'schedule.intervalChipLabel',
      })[2],
    )
    await user.click(
      screen.getByRole('button', { name: 'schedule.saveInterval' }),
    )

    expect(saveConfigMock).toHaveBeenCalledWith(
      expect.objectContaining({ dueAfterHours: 24 }),
    )
    expect(backendMock.applySchedule).not.toHaveBeenCalled()
  })

  test('surfaces save interval failures from the main configuration panel', async () => {
    const user = userEvent.setup()
    saveConfigMock.mockRejectedValueOnce(new Error('save failed'))

    render(<SchedulePage />)

    await screen.findByTestId('schedule-page')
    await user.click(
      screen.getAllByRole('button', {
        name: 'schedule.intervalChipLabel',
      })[2],
    )
    await user.click(
      screen.getByRole('button', { name: 'schedule.saveInterval' }),
    )
    expect(await screen.findByRole('alert')).toHaveTextContent('save failed')

    saveConfigMock.mockRejectedValueOnce('save fallback')
    await user.click(
      screen.getByRole('button', { name: 'schedule.saveInterval' }),
    )
    expect(await screen.findByText('common.unavailable')).toBeInTheDocument()
  })

  test('removes the installed schedule from the main configuration panel', async () => {
    const user = userEvent.setup()

    render(<SchedulePage />)

    await screen.findByTestId('schedule-page')
    await user.click(
      screen.getByRole('button', { name: 'schedule.removeInstalledSchedule' }),
    )

    expect(backendMock.removeSchedule).toHaveBeenCalledTimes(1)
    expect(refreshAppDataMock).toHaveBeenCalledTimes(1)
  })

  test('ignores late loader completion after unmount', async () => {
    let resolvePlan: (value: ReturnType<typeof planFixture>) => void = () => {}
    backendMock.previewSchedule.mockReturnValueOnce(
      new Promise((resolve) => {
        resolvePlan = resolve
      }),
    )
    backendMock.scheduleStatus.mockResolvedValueOnce(statusFixture())

    const { unmount } = render(<SchedulePage />)
    unmount()

    await act(async () => {
      resolvePlan(planFixture())
      await Promise.resolve()
    })
  })
})

function planFixture() {
  return {
    applyCommands: [['pathkeep', 'schedule', 'apply']],
    applySupported: true,
    executablePath: '/Applications/PathKeep.app',
    generatedFiles: [],
    label: 'PathKeep Backup',
    manualSteps: [],
    platform: 'macos',
    rollbackCommands: [['pathkeep', 'schedule', 'remove']],
  }
}

function statusFixture(overrides: Record<string, unknown> = {}) {
  return {
    applySupported: true,
    auditPath: '/tmp/pathkeep/schedule-audit.json',
    checkIntervalHours: 24,
    detectedFiles: [],
    dueAfterHours: 12,
    installState: 'installed',
    label: 'PathKeep Backup',
    lastSuccessfulBackupAt: '2026-04-25T12:00:00.000Z',
    manualSteps: [],
    platform: 'macos',
    warnings: [],
    ...overrides,
  }
}

function applyResultFixture() {
  return {
    applied: true,
    auditPath: '/tmp/pathkeep/schedule-apply.json',
    files: [],
    message: 'ok',
    platform: 'macos',
  }
}
