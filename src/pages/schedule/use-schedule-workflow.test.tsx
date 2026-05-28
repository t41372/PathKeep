/**
 * @file use-schedule-workflow.test.tsx
 * @description Unit coverage for the Scheduled Backup Settings workflow owner.
 * @module pages/schedule
 *
 * ## Responsibilities
 * - Verify initial detection, manual re-detection, native action progress, and diagnostic copy behavior.
 * - Cover recovery edges that are awkward to assert through the route renderer.
 *
 * ## Not responsible for
 * - Re-testing schedule state derivation tables.
 * - Re-testing native scheduler implementations.
 *
 * ## Dependencies
 * - Mocks shell data, the typed backend schedule client, clipboard access, and next-paint scheduling.
 *
 * ## Performance notes
 * - Uses bounded in-memory fixtures only; no archive history or native scheduler is touched.
 */

import { act, renderHook, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import type {
  AppSnapshot,
  ApplyResult,
  SchedulePlan,
  ScheduleStatus,
} from '../../lib/types'
import { useScheduleWorkflow } from './use-schedule-workflow'

const {
  backendMock,
  clipboardWriteTextMock,
  refreshAppDataMock,
  saveConfigMock,
  useShellDataMock,
} = vi.hoisted(() => ({
  backendMock: {
    applySchedule: vi.fn(),
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

vi.mock('../../lib/wait-for-next-paint', () => ({
  waitForNextPaint: () => Promise.resolve(),
}))

describe('useScheduleWorkflow', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    stubClipboard(true)
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

  test('detects the scheduler on mount and falls back to the detection time when backend checkedAt is missing', async () => {
    backendMock.scheduleStatus.mockResolvedValueOnce(
      statusFixture({
        checkedAt: null,
        dueAfterHours: 72,
        lastSuccessfulBackupAt: null,
      }),
    )
    useShellDataMock.mockReturnValue({
      refreshAppData: refreshAppDataMock,
      refreshKey: 1,
      saveConfig: saveConfigMock,
      snapshot: snapshotFixture({
        archiveStatus: { lastSuccessfulBackupAt: null },
        config: {
          initialized: true,
          scheduleCheckIntervalHours: 24,
          selectedProfileIds: ['chrome:Default'],
        },
      }),
    })

    const { result } = renderWorkflow()

    await waitForLoaded(result)

    expect(result.current.lastCheckedAt).toBeInstanceOf(Date)
    expect(result.current.draftDueAfterHours).toBe(72)
    expect(result.current.uiState).toBe('INSTALLED_WARN')
    expect(result.current.actionResult).toMatchObject({
      kind: 'detect',
      status: 'success',
    })
  })

  test('surfaces non-Error detection failures as an unavailable schedule read model', async () => {
    backendMock.previewSchedule.mockRejectedValueOnce('bridge unavailable')

    const { result } = renderWorkflow()

    await waitFor(() => {
      expect(result.current.loading).toBe(false)
      expect(result.current.error).toBe('bridge unavailable')
    })
    expect(result.current.actionResult).toMatchObject({
      kind: 'detect',
      message: 'bridge unavailable',
      status: 'error',
    })
  })

  test('persists interval changes before install and reports native apply failures inline', async () => {
    const refreshedPlan = planFixture({ label: 'PathKeep Backup refreshed' })
    backendMock.previewSchedule
      .mockResolvedValueOnce(planFixture())
      .mockResolvedValueOnce(refreshedPlan)
      .mockResolvedValue(planFixture())
    backendMock.applySchedule.mockResolvedValueOnce(
      applyResultFixture({ applied: false, message: 'launchctl denied' }),
    )
    const { result } = renderWorkflow()

    await waitForLoaded(result)
    act(() => {
      result.current.setDraftDueAfterHours(24)
    })
    await act(async () => {
      await result.current.runNativeAction('install')
    })

    expect(saveConfigMock).toHaveBeenCalledWith(
      expect.objectContaining({ dueAfterHours: 24 }),
    )
    expect(backendMock.applySchedule).toHaveBeenCalledWith(refreshedPlan)
    expect(result.current.actionResult).toMatchObject({
      kind: 'install',
      message: 'launchctl denied',
      status: 'error',
    })
  })

  test('handles remove and repair through their dedicated schedule commands', async () => {
    backendMock.removeSchedule.mockResolvedValueOnce(
      applyResultFixture({ applied: false, message: 'nothing to remove' }),
    )
    const { result } = renderWorkflow()

    await waitForLoaded(result)
    await act(async () => {
      await result.current.runNativeAction('remove')
    })
    await act(async () => {
      await result.current.runNativeAction('repair')
    })

    expect(backendMock.removeSchedule).toHaveBeenCalledTimes(1)
    expect(backendMock.repairSchedule).toHaveBeenCalledTimes(1)
    expect(result.current.actionResult).toMatchObject({
      kind: 'repair',
      status: 'success',
    })
  })

  test('fails install before native execution when interval persistence has no initialized config', async () => {
    useShellDataMock.mockReturnValue({
      refreshAppData: refreshAppDataMock,
      refreshKey: 1,
      saveConfig: saveConfigMock,
      snapshot: null,
    })
    const { result } = renderWorkflow()

    await waitForLoaded(result)
    act(() => {
      result.current.setDraftDueAfterHours(6)
    })
    await act(async () => {
      await result.current.runNativeAction('install')
    })

    expect(backendMock.applySchedule).not.toHaveBeenCalled()
    expect(result.current.actionResult).toMatchObject({
      kind: 'install',
      message: 'schedule.initializeArchiveFirst',
      status: 'error',
    })
  })

  test('leaves native actions inert when detection has no plan', async () => {
    backendMock.previewSchedule.mockResolvedValueOnce(null)
    const { result } = renderWorkflow()

    await waitForLoaded(result)
    await act(async () => {
      await result.current.runNativeAction('remove')
    })

    expect(backendMock.removeSchedule).not.toHaveBeenCalled()
    expect(result.current.actionResult).toMatchObject({
      kind: 'detect',
      status: 'success',
    })
  })

  test('copies diagnostics with a null plan and reports clipboard failures', async () => {
    backendMock.previewSchedule.mockResolvedValueOnce(null)
    const { result } = renderWorkflow()

    await waitForLoaded(result)
    await act(async () => {
      await result.current.copyDiagnostics()
    })

    expect(clipboardWriteTextMock).toHaveBeenCalledTimes(1)
    expect(JSON.parse(clipboardTextAt(0))).toEqual(
      expect.objectContaining({
        installState: 'installed',
        plan: null,
      }),
    )

    stubClipboard(false)
    await act(async () => {
      await result.current.copyDiagnostics()
    })

    expect(result.current.actionResult).toMatchObject({
      kind: 'copy-diagnostics',
      message: 'schedule.diagnosticsClipboardUnavailable',
      status: 'error',
    })
  })

  test('copies diagnostics with typed fallback arrays and relative generated file paths', async () => {
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
    backendMock.scheduleStatus.mockResolvedValueOnce(
      statusFixture({
        checkedAt: null,
        issues: undefined,
        verificationChecks: undefined,
      }),
    )
    const { result } = renderWorkflow()

    await waitForLoaded(result)
    await act(async () => {
      await result.current.copyDiagnostics()
    })

    const copied = JSON.parse(clipboardTextAt(0)) as {
      checkedAt: string
      issues: unknown[]
      plan: {
        generatedFiles: Array<{
          path: string
          purpose: string
        }>
      }
      verificationChecks: unknown[]
    }
    expect(copied).toEqual(
      expect.objectContaining({
        checkedAt: expect.any(String),
        issues: [],
        verificationChecks: [],
      }),
    )
    expect(copied.plan.generatedFiles).toEqual([
      {
        path: 'launchd/pathkeep.plist',
        purpose: 'LaunchAgent',
      },
    ])

    clipboardWriteTextMock.mockRejectedValueOnce('clipboard denied')
    await act(async () => {
      await result.current.copyDiagnostics()
    })

    expect(result.current.actionResult).toMatchObject({
      kind: 'copy-diagnostics',
      message: 'clipboard denied',
      status: 'error',
    })
  })

  test('does not copy diagnostics before status exists', async () => {
    backendMock.scheduleStatus.mockRejectedValueOnce(new Error('status failed'))
    const { result } = renderWorkflow()

    await waitForLoaded(result)
    await act(async () => {
      await result.current.copyDiagnostics()
    })

    expect(clipboardWriteTextMock).not.toHaveBeenCalled()
  })
})

function renderWorkflow() {
  return renderHook(() => useScheduleWorkflow())
}

async function waitForLoaded(result: {
  readonly current: ReturnType<typeof useScheduleWorkflow>
}) {
  await waitFor(() => {
    expect(result.current.loading).toBe(false)
  })
}

function stubClipboard(enabled: boolean) {
  const navigatorWithClipboard = Object.create(navigator) as Navigator
  Object.defineProperty(navigatorWithClipboard, 'clipboard', {
    configurable: true,
    value: enabled
      ? {
          writeText: clipboardWriteTextMock,
        }
      : undefined,
  })
  vi.stubGlobal('navigator', navigatorWithClipboard)
}

function clipboardTextAt(index: number): string {
  const value: unknown = clipboardWriteTextMock.mock.calls[index]?.[0]
  if (typeof value !== 'string') {
    throw new Error(`Expected clipboard call ${index} to contain text.`)
  }
  return value
}

function snapshotFixture(overrides: Record<string, unknown> = {}): AppSnapshot {
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
    ],
    config: {
      dueAfterHours: 12,
      initialized: true,
      scheduleCheckIntervalHours: 24,
      selectedProfileIds: ['chrome:Default'],
    },
    ...overrides,
  } as unknown as AppSnapshot
}

function planFixture(overrides: Record<string, unknown> = {}): SchedulePlan {
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
    manualStepDetails: [],
    manualSteps: [],
    platform: 'macos',
    rollbackCommands: [['launchctl', 'bootout', 'gui/501/pathkeep']],
    ...overrides,
  } as SchedulePlan
}

function statusFixture(
  overrides: Record<string, unknown> = {},
): ScheduleStatus {
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
  } as ScheduleStatus
}

function applyResultFixture(
  overrides: Record<string, unknown> = {},
): ApplyResult {
  return {
    applied: true,
    auditPath: '/tmp/pathkeep/schedule-apply.json',
    files: ['~/Library/LaunchAgents/pathkeep.plist'],
    message: 'ok',
    platform: 'macos',
    ...overrides,
  } as ApplyResult
}
