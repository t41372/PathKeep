/**
 * Pure helpers shared by `paper-audit-panel.tsx`.
 *
 * Lives in its own file because the React fast-refresh rule wants the
 * component file to export only components — extracting the label / time
 * formatters here keeps the panel file fast-refresh-friendly without
 * losing test coverage on the deterministic logic.
 */

export type PaperAuditTranslate = (
  key: string,
  vars?: Record<string, string | number>,
) => string

/** Map a backup-run kind onto its paper-redesign localised label. */
export function paperRunTypeLabel(
  runType: string | undefined,
  auditT: (key: string) => string,
): string {
  switch (runType) {
    case 'backup':
      return auditT('paperRunTypeBackup')
    case 'import':
      return auditT('paperRunTypeImport')
    case 'maintenance':
      return auditT('paperRunTypeMaintenance')
    default:
      return runType?.toUpperCase() ?? ''
  }
}

/**
 * Render a backup-run timestamp as a paper-style relative-time label.
 *
 * `now` is injectable so tests can pin the clock; production callers can
 * skip it and rely on `Date.now()`.
 */
export function paperWhenLabel(
  isoTimestamp: string,
  auditT: PaperAuditTranslate,
  now: () => number = Date.now,
): string {
  const parsed = Date.parse(isoTimestamp)
  if (Number.isNaN(parsed)) return ''
  const deltaMs = now() - parsed
  if (deltaMs < 60_000) return auditT('paperWhenJustNow')
  const minutes = Math.floor(deltaMs / 60_000)
  if (minutes < 60) return auditT('paperWhenMinutesAgo', { count: minutes })
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return auditT('paperWhenHoursAgo', { count: hours })
  const days = Math.floor(hours / 24)
  return auditT('paperWhenDaysAgo', { count: days })
}
