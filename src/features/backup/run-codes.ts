/**
 * Which recorded codes are warnings (something was skipped or failed) and
 * which are notes (side work that went fine, or what an undo or restore of an
 * import did), and the fix each one points to. Runs and imports record the
 * same kind of entries. Kept apart from the sentences so the shell's backup
 * toast can count warnings without loading the Backup screen's code.
 */
import type { BackupReport, BackupWarning } from '@/lib/types'

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
  // A backup's per-profile outcome codes (`ProfileOutcome`); the older codes
  // above stay for runs recorded before them.
  'source-profile-not-discovered': {},
  'source-unreadable': { evidence: 'diagnostic' },
  'source-permission-denied': {
    action: 'full-disk-access',
    evidence: 'diagnostic',
  },
  'source-read-failed': { evidence: 'diagnostic' },
  'source-stream-failed': { evidence: 'diagnostic' },
  'source-favicons-unavailable': { evidence: 'diagnostic' },
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
  // The work is saved; the diagnostic says which record could not be written.
  'review-record-failed': { evidence: 'diagnostic' },
  'skipped-missing-timestamp': {},
  'browser-skipped-missing-url-row': {},
  // The message names the file; the diagnostic only has the parser's error.
  'parse-failed': { evidence: 'message' },
  'no-importable-files': {},
} as const satisfies Record<string, Rule>

export const NOTES = {
  'source-ids-reset': {},
  // Rows the browser cannot show either: their page row is gone.
  'source-visits-without-page': {},
  'profile-search-terms-captured': {},
  'og-prefetch-summary': {},
  'og-refetch-summary': {},
  'og-cleanup-summary': {},
  'ai-autoindex-queued-while-paused': { action: 'background-settings' },
  // Written to an import's record by each undo and restore of it.
  'batch-reverted': {},
  'batch-restored': {},
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

/** What a run, a backup report or an import recorded, coded or not. */
export interface Recorded {
  warnings: string[]
  warningDetails?: BackupWarning[]
}

/**
 * The recorded entries, coded or not. Every entry is written as a sentence
 * and, since codes exist, as a coded entry with the same sentence. Entries
 * from before then only have the sentence, and an import from then that was
 * undone or restored later has both kinds, so each sentence without a coded
 * twin is shown as it is, before the coded ones (it is older).
 */
export function entriesOf(recorded: Recorded): BackupWarning[] {
  const coded = recorded.warningDetails ?? []
  const said = new Set(coded.map((entry) => entry.message))
  return [
    ...recorded.warnings
      .filter((message) => !said.has(message))
      .map((message) => ({ code: '', message })),
    ...coded,
  ]
}

/**
 * Whether the backup skipped Safari because PathKeep may not read its history
 * (macOS Full Disk Access). Read from the profile's outcome, never from text.
 */
export function skippedSafariForFullDiskAccess(report: BackupReport) {
  return report.profiles.some(
    (profile) =>
      profile.profileId.startsWith('safari:') &&
      profile.outcome.status === 'unavailable' &&
      profile.outcome.code === 'source-permission-denied',
  )
}

/** A profile the backup could not read, other than Safari waiting for Full Disk Access. */
export function leftOutOtherThanSafari(report: BackupReport) {
  return report.profiles.some(
    (profile) =>
      profile.outcome.status !== 'complete' &&
      !(
        profile.profileId.startsWith('safari:') &&
        profile.outcome.code === 'source-permission-denied'
      ),
  )
}

/** How many entries are warnings, for the toast when a backup finishes. */
export function countRunWarnings(recorded: Recorded) {
  return entriesOf(recorded).filter((warning) => !isNote(warning.code)).length
}
