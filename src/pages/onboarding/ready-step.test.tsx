/**
 * Final onboarding review step coverage.
 *
 * ## Responsibilities
 * - Cover the schedule install, skip, and default summary branches.
 * - Verify the skip hint remains visible when setup defers scheduled backup.
 * - Verify the default-on link-preview egress disclosure is always shown.
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

    rerender(
      readyStepElement({ dueAfterHours: 1.5, scheduleSetupMode: 'install' }),
    )
    expect(
      screen.getByText('Install every 90 minutes during setup'),
    ).toBeVisible()

    rerender(readyStepElement({ dueAfterHours: 1.5, scheduleSetupMode: null }))
    expect(screen.getByText('Every 90 minutes')).toBeVisible()
  })

  test('discloses default-on link preview fetching before the first backup', () => {
    renderReadyStep()
    expect(screen.getByText('Link previews are on by default')).toBeVisible()
    expect(
      screen.getByText(
        /After each backup, PathKeep requests preview images from the sites you visited \(Bilibili via its public API\)\. This is PathKeep's only default network request, and it carries no cookies or account information\. You can turn it off or switch to on-demand later in Settings → Link previews\./,
      ),
    ).toBeVisible()
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
