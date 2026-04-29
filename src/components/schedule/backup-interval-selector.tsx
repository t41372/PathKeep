/**
 * Shared scheduled-backup interval chip selector.
 *
 * ## Responsibilities
 * - Render the canonical interval choices with the existing chip visual grammar.
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

import { scheduledBackupIntervalOptions } from '../../lib/schedule-options'

interface BackupIntervalSelectorProps {
  disabled?: boolean
  formatLabel: (hours: number) => string
  onChange: (hours: number) => void
  value: number
}

/**
 * Keeps the backup interval control reusable across setup and settings without
 * letting route-specific config persistence leak into the shared component.
 *
 * @param disabled disables every chip while the owning route is busy.
 * @param formatLabel formats each hour value in the caller's i18n namespace.
 * @param onChange receives the selected interval in hours.
 * @param value currently selected interval in hours.
 * @returns A stable chip group for the canonical scheduled-backup intervals.
 */
export function BackupIntervalSelector({
  disabled = false,
  formatLabel,
  onChange,
  value,
}: BackupIntervalSelectorProps) {
  return (
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
  )
}
