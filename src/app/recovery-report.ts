/**
 * Reads the report behind an `archive_recovery_required` error: the backend's
 * way of saying the archive needs the recovery screen (see
 * `vault-worker::app::archive_recovery_required_error`).
 */
import type { ArchiveRecoveryReport } from '@/lib/types'

const RECOVERY_PREFIX = 'archive_recovery_required: '

export function parseRecoveryReport(
  error: unknown,
): ArchiveRecoveryReport | null {
  const message =
    error instanceof Error
      ? error.message
      : typeof error === 'string'
        ? error
        : ''
  const at = message.indexOf(RECOVERY_PREFIX)
  if (at < 0) return null
  try {
    return JSON.parse(
      message.slice(at + RECOVERY_PREFIX.length),
    ) as ArchiveRecoveryReport
  } catch {
    return null
  }
}
