/**
 * Which recorded codes are warnings (something was skipped or failed) and
 * which are notes (side work that went fine), and the fix each one points to.
 * Kept apart from the sentences so the shell's backup toast can count
 * warnings without loading the Backup screen's code.
 */
import type { BackupWarning } from '@/lib/types'

export type RunAction =
  | 'full-disk-access'
  | 'ai-settings'
  | 'background-settings'

export interface Rule {
  action?: RunAction
  /** Raw text shown under the sentence, for codes whose detail is evidence. */
  evidence?: 'diagnostic' | 'message'
}

export const WARNINGS = {
  'safari-full-disk-access-skip': { action: 'full-disk-access' },
  'profile-history-unreadable-skip': { evidence: 'diagnostic' },
  'profile-not-detected-skip': {},
  'staging-fallback-recovered-copy': { evidence: 'diagnostic' },
  'git-history-skipped': { evidence: 'diagnostic' },
  'intelligence-refresh-failed': { evidence: 'diagnostic' },
  'ai-autoindex-enqueue-failed': { evidence: 'diagnostic' },
  'ai-autoindex-provider-not-ready': {
    action: 'ai-settings',
    evidence: 'diagnostic',
  },
  'enrichment-reoffer-failed': { evidence: 'message' },
  'og-refetch-failed': { evidence: 'diagnostic' },
  'og-prefetch-failed': { evidence: 'diagnostic' },
  'og-cleanup-failed': { evidence: 'diagnostic' },
  'skipped-missing-timestamp': {},
  'browser-skipped-missing-url-row': {},
  // The message names the file; the diagnostic only has the parser's error.
  'parse-failed': { evidence: 'message' },
  'no-importable-files': {},
} as const satisfies Record<string, Rule>

export const NOTES = {
  'profile-search-terms-captured': {},
  'og-prefetch-summary': {},
  'og-refetch-summary': {},
  'og-cleanup-summary': {},
  'ai-autoindex-queued-while-paused': { action: 'background-settings' },
} as const satisfies Record<string, Rule>

type WarningCode = keyof typeof WARNINGS
type NoteCode = keyof typeof NOTES

/** Codes the Takeout parser crate reports, prefixed by the import flow. */
export const PARSER_PREFIX = 'parser-'

export function isWarning(code: string): code is WarningCode {
  return Object.hasOwn(WARNINGS, code)
}

export function isNote(code: string): code is NoteCode {
  return Object.hasOwn(NOTES, code)
}

/** What a run or a backup report recorded, coded or not. */
interface Recorded {
  warnings: string[]
  warningDetails?: BackupWarning[]
}

/** A run's entries; runs recorded before codes existed only have the text. */
export function entriesOf(recorded: Recorded): BackupWarning[] {
  return (
    recorded.warningDetails ??
    recorded.warnings.map((message) => ({ code: '', message }))
  )
}

/** How many entries are warnings, for the toast when a backup finishes. */
export function countRunWarnings(recorded: Recorded) {
  return entriesOf(recorded).filter((warning) => !isNote(warning.code)).length
}
