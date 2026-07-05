/**
 * Diagnostic-report builder for the backup-failure toast.
 *
 * Kept in its own module (not the component file) so it stays a pure,
 * directly-unit-testable function and does not break React Fast Refresh, which
 * requires component files to export only components.
 */

import { formatBuildVersionTitle } from '@/lib/build-info'
import type { ShellErrorKind } from '@/app/shell-data-context'
import type { AppBuildInfo } from '@/lib/types'

/**
 * Builds the plain-text diagnostic report copied to the clipboard / shown in the
 * failure toast's details panel — the actionable bundle a user pastes into a bug
 * report instead of hunting through a log folder.
 */
export function buildBackupDiagnosticReport(input: {
  message: string
  rawError: string | null
  errorKind: ShellErrorKind
  buildInfo: AppBuildInfo | null
  timestamp: string
  userAgent: string
}): string {
  const version = formatBuildVersionTitle(input.buildInfo) ?? 'unknown'
  return [
    'PathKeep backup failure',
    `Time:     ${input.timestamp}`,
    `Version:  ${version}`,
    `Platform: ${input.userAgent}`,
    `Kind:     ${input.errorKind ?? 'generic'}`,
    `Message:  ${input.message}`,
    `Detail:   ${input.rawError ?? '(none)'}`,
  ].join('\n')
}
