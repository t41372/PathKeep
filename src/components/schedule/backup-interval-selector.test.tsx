/**
 * @file backup-interval-selector.test.tsx
 * @description Coverage for the shared scheduled-backup interval control.
 * @module components/schedule
 *
 * ## Responsibilities
 * - Verify preset interval chips remain available.
 * - Verify custom whole-minute intervals can be entered without route-specific logic.
 * - Keep invalid custom values from reaching persisted app config.
 *
 * ## Not responsible for
 * - Re-testing route-level scheduler install/update flows.
 * - Snapshotting CSS layout.
 *
 * ## Dependencies
 * - Testing Library drives the control like the Schedule and Onboarding routes do.
 *
 * ## Performance notes
 * - Pure render tests only; no backend or archive work.
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, test, vi } from 'vitest'
import {
  backupIntervalHoursToMinutes,
  backupIntervalMinutesToHours,
  parseCustomBackupIntervalMinutes,
} from '../../lib/schedule-options'
import { BackupIntervalSelector } from './backup-interval-selector'

describe('BackupIntervalSelector', () => {
  test('keeps preset chips and forwards custom whole-minute intervals', async () => {
    const user = userEvent.setup()
    const onChange = vi.fn()

    renderSelector({ onChange, value: 12 })

    await user.click(screen.getByRole('button', { name: '24h' }))
    expect(onChange).toHaveBeenLastCalledWith(24)

    const customInput = screen.getByLabelText('Custom interval')
    await user.clear(customInput)
    await user.type(customInput, '90')

    expect(onChange).toHaveBeenLastCalledWith(1.5)
  })

  test('rejects invalid custom intervals and restores the persisted value on blur', async () => {
    const user = userEvent.setup()
    const onChange = vi.fn()

    renderSelector({ onChange, value: 12 })

    const customInput = screen.getByLabelText('Custom interval')
    expect(customInput).toHaveValue(720)

    await user.clear(customInput)
    await user.type(customInput, '0')

    expect(onChange).not.toHaveBeenCalled()
    expect(screen.getByRole('alert')).toHaveTextContent(
      'Enter a whole number of minutes, 1 or more.',
    )

    await user.tab()

    expect(customInput).toHaveValue(720)
  })

  test('parses only positive safe whole minutes and converts to fractional hours', () => {
    expect(parseCustomBackupIntervalMinutes('1')).toBe(1)
    expect(parseCustomBackupIntervalMinutes('90')).toBe(90)
    expect(parseCustomBackupIntervalMinutes('0')).toBeNull()
    expect(parseCustomBackupIntervalMinutes('1.5')).toBeNull()
    expect(parseCustomBackupIntervalMinutes('abc')).toBeNull()
    expect(
      parseCustomBackupIntervalMinutes(String(Number.MAX_SAFE_INTEGER + 1)),
    ).toBeNull()
    expect(backupIntervalMinutesToHours(90)).toBe(1.5)
    expect(backupIntervalHoursToMinutes(1.5)).toBe(90)
    expect(backupIntervalHoursToMinutes(Number.NaN)).toBe(1)
    expect(backupIntervalHoursToMinutes(0)).toBe(1)
  })
})

function renderSelector({
  onChange = vi.fn(),
  value,
}: {
  onChange?: (hours: number) => void
  value: number
}) {
  return render(
    <BackupIntervalSelector
      customInvalidMessage="Enter a whole number of minutes, 1 or more."
      customLabel="Custom interval"
      customUnitLabel="minutes"
      formatLabel={(hours) => `${hours}h`}
      value={value}
      onChange={onChange}
    />,
  )
}
