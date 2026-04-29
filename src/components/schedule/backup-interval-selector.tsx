/**
 * Shared scheduled-backup interval chip selector.
 *
 * ## Responsibilities
 * - Render the canonical interval choices with the existing chip visual grammar.
 * - Accept a custom whole-minute interval when presets are not specific enough.
 * - Keep Schedule page and Onboarding interval selection from drifting.
 * - Leave localization to the caller so each route can use its own namespace.
 *
 * ## Not responsible for
 * - Saving config changes.
 * - Installing, updating, or removing native scheduler artifacts.
 * - Deciding whether a selected interval is dirty.
 *
 * ## Dependencies
 * - `scheduledBackupIntervalOptions` for the canonical interval list.
 *
 * ## Performance notes
 * - The option list is tiny and static; the component does no data fetching or
 *   archive work.
 */

import { useEffect, useId, useState, type ChangeEvent } from 'react'
import {
  backupIntervalHoursToMinutes,
  backupIntervalMinutesToHours,
  parseCustomBackupIntervalMinutes,
  scheduledBackupIntervalOptions,
} from '../../lib/schedule-options'

interface BackupIntervalSelectorProps {
  customInvalidMessage: string
  customLabel: string
  customUnitLabel: string
  disabled?: boolean
  formatLabel: (hours: number) => string
  onChange: (hours: number) => void
  value: number
}

/**
 * Keeps the backup interval control reusable across setup and settings without
 * letting route-specific config persistence leak into the shared component.
 *
 * @param customInvalidMessage localized validation copy for non-positive or non-integer input.
 * @param customLabel localized label for the custom interval field.
 * @param customUnitLabel localized unit label shown beside the numeric field.
 * @param disabled disables every chip while the owning route is busy.
 * @param formatLabel formats each hour value in the caller's i18n namespace.
 * @param onChange receives the selected interval in hours, including fractional hours for minute-level values.
 * @param value currently selected interval in hours.
 * @returns A stable chip group for the canonical scheduled-backup intervals.
 */
export function BackupIntervalSelector({
  customInvalidMessage,
  customLabel,
  customUnitLabel,
  disabled = false,
  formatLabel,
  onChange,
  value,
}: BackupIntervalSelectorProps) {
  const [customValue, setCustomValue] = useState(() =>
    String(backupIntervalHoursToMinutes(value)),
  )
  const generatedId = useId()
  const customInputId = `${generatedId}-backup-interval-custom-input`
  const customErrorId = `${generatedId}-backup-interval-custom-error`
  const parsedCustomValue = parseCustomBackupIntervalMinutes(customValue)
  const customValueInvalid =
    customValue.trim().length > 0 && parsedCustomValue === null

  useEffect(() => {
    setCustomValue(String(backupIntervalHoursToMinutes(value)))
  }, [value])

  function handleCustomValueChange(event: ChangeEvent<HTMLInputElement>) {
    const nextValue = event.currentTarget.value
    setCustomValue(nextValue)
    const nextMinutes = parseCustomBackupIntervalMinutes(nextValue)
    if (nextMinutes !== null) {
      onChange(backupIntervalMinutesToHours(nextMinutes))
    }
  }

  function handleCustomValueBlur() {
    if (parseCustomBackupIntervalMinutes(customValue) === null) {
      setCustomValue(String(backupIntervalHoursToMinutes(value)))
    }
  }

  return (
    <div className="backup-interval-selector">
      <div className="interval-chips">
        {scheduledBackupIntervalOptions.map((hours) => (
          <button
            aria-pressed={value === hours}
            className={`interval-chip ${value === hours ? 'active' : ''}`}
            disabled={disabled}
            key={hours}
            type="button"
            onClick={() => onChange(hours)}
          >
            {formatLabel(hours)}
          </button>
        ))}
      </div>
      <label className="interval-custom-control" htmlFor={customInputId}>
        <span className="interval-custom-label">{customLabel}</span>
        <input
          aria-describedby={customValueInvalid ? customErrorId : undefined}
          aria-invalid={customValueInvalid}
          aria-label={customLabel}
          className="interval-custom-input"
          disabled={disabled}
          id={customInputId}
          inputMode="numeric"
          min={1}
          pattern="[0-9]*"
          step={1}
          type="number"
          value={customValue}
          onBlur={handleCustomValueBlur}
          onChange={handleCustomValueChange}
        />
        <span className="interval-custom-unit">{customUnitLabel}</span>
      </label>
      {customValueInvalid ? (
        <p className="interval-custom-error" id={customErrorId} role="alert">
          {customInvalidMessage}
        </p>
      ) : null}
    </div>
  )
}
