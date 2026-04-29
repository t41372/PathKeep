/**
 * Final onboarding review step coverage.
 *
 * ## Responsibilities
 * - Cover the schedule install, skip, and default summary branches.
 * - Verify the skip hint remains visible when setup defers scheduled backup.
 *
 * ## Not responsible for
 * - Re-testing the full onboarding route state machine.
 * - Re-testing native scheduler install behavior.
 *
 * ## Dependencies
 * - Uses the shipped i18n provider so visible setup copy is covered through
 *   the same catalog as production.
 *
 * ## Performance notes
 * - Pure render coverage only; no backend, archive, or scheduler work.
 */

import { render, screen } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import { ReadyStep, type ReadyStepProps } from './ready-step'

describe('ReadyStep', () => {
  test('renders default, install, and skipped schedule summaries', () => {
    const { rerender } = renderReadyStep({ scheduleSetupMode: null })
    expect(screen.getByText('Every 12 hours')).toBeVisible()

    rerender(readyStepElement({ scheduleSetupMode: 'install' }))
    expect(
      screen.getByText('Install every 12 hours during setup'),
    ).toBeVisible()

    rerender(readyStepElement({ scheduleSetupMode: 'skip' }))
    expect(screen.getByText('Skipped for now')).toBeVisible()
    expect(screen.getByText('Scheduled backup skipped')).toBeVisible()
    expect(screen.getByText(/System → Scheduled Backup Settings/)).toBeVisible()
  })
})

function renderReadyStep(overrides: Partial<ReadyStepProps> = {}) {
  return render(readyStepElement(overrides))
}

function readyStepElement(overrides: Partial<ReadyStepProps> = {}) {
  return (
    <I18nProvider>
      <ReadyStep {...readyStepProps(overrides)} />
    </I18nProvider>
  )
}

function readyStepProps(
  overrides: Partial<ReadyStepProps> = {},
): ReadyStepProps {
  return {
    appRoot: '/tmp/pathkeep',
    archiveMode: 'Encrypted',
    busyAction: null,
    dueAfterHours: 12,
    localError: null,
    onBack: vi.fn(),
    onFinish: vi.fn(),
    onOpenFullDiskAccessSettings: vi.fn(),
    scheduleSetupMode: null,
    selectedAccessIssueCount: 0,
    selectedCount: 1,
    ...overrides,
  }
}
