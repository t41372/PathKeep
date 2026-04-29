/**
 * @file schedule-ui-state.test.ts
 * @description Table tests for the Scheduled Backup Settings UI state machine.
 * @module pages/schedule
 *
 * ## Responsibilities
 * - Lock backend install-state and typed-issue mapping to the five route states.
 * - Verify dismissible warning filtering does not hide hard scheduler faults.
 *
 * ## Not responsible for
 * - Rendering route copy or native scheduler behavior.
 *
 * ## Dependencies
 * - Pure state derivation helpers from `schedule-ui-state`.
 *
 * ## Performance notes
 * - Runs constant-time table assertions over tiny read-model fixtures.
 */

import { describe, expect, test } from 'vitest'
import {
  deriveScheduleUiState,
  visibleScheduleIssues,
} from './schedule-ui-state'
import type { ScheduleIssue, ScheduleStatus } from '../../lib/types'

describe('deriveScheduleUiState', () => {
  test.each([
    [true, statusFixture(), false, 'CHECKING'],
    [false, null, false, 'CHECKING'],
    [
      false,
      statusFixture({ installState: 'not-installed' }),
      false,
      'NOT_INSTALLED',
    ],
    [false, statusFixture(), false, 'INSTALLED_OK'],
    [false, statusFixture(), true, 'INSTALLED_WARN'],
    [
      false,
      statusFixture({ installState: 'manual-review' }),
      false,
      'INSTALLED_WARN',
    ],
    [
      false,
      statusFixture({ installState: 'mismatch' }),
      false,
      'INSTALLED_WARN',
    ],
    [
      false,
      statusFixture({ installState: 'legacy-install-detected' }),
      false,
      'INSTALLED_WARN',
    ],
    [
      false,
      statusFixture({ issues: [issueFixture('legacy', 'warning', false)] }),
      false,
      'INSTALLED_WARN',
    ],
    [
      false,
      statusFixture({ installState: 'permission-warning' }),
      false,
      'INSTALLED_ERROR',
    ],
    [
      false,
      statusFixture({ issues: [issueFixture('loaded', 'error', false)] }),
      false,
      'INSTALLED_ERROR',
    ],
  ])(
    'maps loading=%s status=%s hasNeverRun=%s to %s',
    (loading, status, hasNeverRun, expected) => {
      expect(
        deriveScheduleUiState({
          hasNeverRun,
          loading,
          status,
        }),
      ).toBe(expected)
    },
  )
})

describe('visibleScheduleIssues', () => {
  test('filters dismissed soft warnings while preserving hard issues', () => {
    const dismissedWarning = issueFixture('soft-warning', 'warning', true)
    const hardError = issueFixture('hard-error', 'error', false)

    expect(
      visibleScheduleIssues(
        [dismissedWarning, hardError],
        new Set(['soft-warning', 'hard-error']),
      ),
    ).toEqual([hardError])
  })
})

function statusFixture(
  overrides: Partial<ScheduleStatus> = {},
): ScheduleStatus {
  return {
    applySupported: true,
    checkIntervalHours: 24,
    detectedFiles: [],
    dueAfterHours: 24,
    installState: 'installed',
    label: 'PathKeep Backup',
    manualSteps: [],
    platform: 'macos',
    warnings: [],
    ...overrides,
  }
}

function issueFixture(
  code: string,
  severity: ScheduleIssue['severity'],
  dismissible: boolean,
): ScheduleIssue {
  return {
    code,
    consequenceKey: 'schedule.issueNeedsReviewDetail',
    detailKey: 'schedule.issueNeedsReviewDetail',
    dismissible,
    evidence: [],
    severity,
    titleKey: 'schedule.issueNeedsReviewTitle',
  }
}
