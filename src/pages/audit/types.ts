/**
 * This module renders the Audit Ledger route, where runs, artifacts, warnings, and rollback hints stay reviewable instead of hidden behind success toasts.
 *
 * Why this file exists:
 * - Route files are where PathKeep turns design-system primitives, desktop read models, and shell scope into user-facing workflow.
 * - They should make deep links, trust copy, loading states, and repair actions obvious without forcing readers to reconstruct the whole page mentally.
 *
 * Main declarations:
 * - `AuditDetailState`
 * - `AuditFilterState`
 * - `AuditDetailTab`
 * - `Translator`
 * - `parseAuditTimestamp`
 * - `resolveBatchEventTime`
 * - `pickRelatedImportBatch`
 * - `localizedBackupWarningText`
 *
 * Source-of-truth notes:
 * - Stay aligned with `docs/design/screens-and-nav.md` for route purpose, navigation, and shared profile-scope rules.
 * - Stay aligned with `docs/design/ux-principles.md` for PME, trust warning grammar, and the no-hidden-state loading contract.
 */

import type {
  AuditRunDetail,
  BackupWarning,
  ImportBatchOverview,
} from '../../lib/types'

/**
 * Captures the state shape used by `AuditDetail`.
 *
 * Keeping this as a named declaration makes the Audit surface easier to review and test than burying the behavior inside another anonymous callback.
 */
export interface AuditDetailState {
  runId: number | null
  detail: AuditRunDetail | null
  error: string | null
}

/**
 * Captures the state shape used by `AuditFilter`.
 *
 * Keeping this as a named declaration makes the Audit surface easier to review and test than burying the behavior inside another anonymous callback.
 */
export interface AuditFilterState {
  runType: string
  severity: 'all' | 'clear' | 'warning' | 'blocked'
  sourceKind: string
  profileId: string
  artifactType: string
}

/**
 * Enumerates the tabs available on this front-end surface.
 *
 * Keeping this as a named declaration makes the Audit surface easier to review and test than burying the behavior inside another anonymous callback.
 */
export type AuditDetailTab = 'summary' | 'artifacts' | 'warnings'

/**
 * Defines the type-level contract for translator.
 *
 * Keeping this as a named declaration makes the Audit surface easier to review and test than burying the behavior inside another anonymous callback.
 */
export type Translator = (
  key: string,
  vars?: Record<string, string | number>,
) => string

/**
 * Parses audit timestamp into the shape this surface expects.
 *
 * Keeping this as a named declaration makes the Audit surface easier to review and test than burying the behavior inside another anonymous callback.
 */
export function parseAuditTimestamp(value?: string | null) {
  if (!value) return Number.NaN
  const parsed = Date.parse(value)
  return Number.isFinite(parsed) ? parsed : Number.NaN
}

/**
 * Resolves batch event time from the available inputs.
 *
 * Keeping this as a named declaration makes the Audit surface easier to review and test than burying the behavior inside another anonymous callback.
 */
export function resolveBatchEventTime(
  batch: ImportBatchOverview,
  runType: string,
): number {
  if (runType === 'rollback') {
    return parseAuditTimestamp(
      batch.revertedAt ?? batch.importedAt ?? batch.createdAt,
    )
  }

  return parseAuditTimestamp(
    batch.importedAt ?? batch.revertedAt ?? batch.createdAt,
  )
}

/**
 * Explains how pick related import batch works.
 *
 * Keeping this as a named declaration makes the Audit surface easier to review and test than burying the behavior inside another anonymous callback.
 */
export function pickRelatedImportBatch(
  detail: AuditRunDetail | null,
  recentImportBatches: ImportBatchOverview[],
) {
  if (!detail) return null
  const runType = detail.run.runType ?? 'backup'
  if (!['import', 'rollback', 'restore'].includes(runType)) return null

  const runProfileId = detail.profileScope[0] ?? null
  const runTimestamp = parseAuditTimestamp(
    detail.run.finishedAt ?? detail.run.startedAt,
  )
  const sameProfileBatches = recentImportBatches.filter(
    (batch) => !runProfileId || batch.profileId === runProfileId,
  )

  return (
    sameProfileBatches.slice().sort((left, right) => {
      const leftDistance = Math.abs(
        resolveBatchEventTime(left, runType) - runTimestamp,
      )
      const rightDistance = Math.abs(
        resolveBatchEventTime(right, runType) - runTimestamp,
      )
      return leftDistance - rightDistance
    })[0] ?? null
  )
}

/**
 * Renders one persisted run warning in the user's language.
 *
 * Backup runs persist a `code` plus typed params alongside the English
 * sentence, so this surface localizes the coded cases and interpolates their
 * numbers. Everything else — non-backup run types, staging fallbacks, git
 * failures, and any code shipped by a newer backend — falls back to the raw
 * message so a warning is never silently swallowed. Coded warnings whose
 * evidence is an error chain append that `diagnostic` verbatim: it is proof,
 * not copy, and translating it would destroy it.
 */
/**
 * Warning codes whose shipped copy names a specific browser profile.
 *
 * Every producer of these codes sets `profileId` (see `archive/ingest`,
 * `chrome/staging`, and the parser note pass-through), so an absent id means a
 * payload this build does not understand — the caller falls back to the
 * backend's own sentence rather than interpolating a blank and shipping
 * "Skipped : its history database is missing".
 */
const PROFILE_SCOPED_WARNING_CODES = new Set([
  'safari-full-disk-access-skip',
  'profile-history-unreadable-skip',
  'profile-not-detected-skip',
  'staging-fallback-recovered-copy',
  'missing-table',
  'missing-source',
  'baseline-support',
  'profile-search-terms-captured',
])

/**
 * Renders the profile-scoped warning codes once `profileId` is known present.
 *
 * Split out so the presence check happens ONCE, in a position TypeScript can
 * narrow, instead of every case re-defaulting to an empty string.
 */
function localizedProfileWarningText(
  code: string,
  profileId: string,
  count: string,
  rawCount: number,
  diagnostic: string,
  t: Translator,
): string {
  switch (code) {
    case 'safari-full-disk-access-skip':
      return t('audit.warningSafariFullDiskAccessSkip', { profileId })
    case 'profile-history-unreadable-skip':
      return `${t('audit.warningProfileHistoryUnreadableSkip', { profileId })} ${diagnostic}`
    case 'profile-not-detected-skip':
      return t('audit.warningProfileNotDetectedSkip', { profileId })
    case 'staging-fallback-recovered-copy':
      return `${t('audit.warningStagingFallbackRecoveredCopy', { profileId })} ${diagnostic}`
    // Parser-side per-profile notes keep the parser's own stable codes; the
    // exact table/source evidence stays in the appended diagnostic.
    case 'missing-table':
      return `${t('audit.warningParserMissingTable', { profileId })} ${diagnostic}`
    case 'missing-source':
      return `${t('audit.warningParserMissingSource', { profileId })} ${diagnostic}`
    case 'baseline-support':
      return t('audit.warningParserBaselineSupport', { profileId })
    default:
      // Plural pairs, not "row(s)": the ledger reads back to a person.
      return t(
        rawCount === 1
          ? 'audit.warningProfileSearchTermsCapturedOne'
          : 'audit.warningProfileSearchTermsCapturedMany',
        { count, profileId },
      )
  }
}

export function localizedBackupWarningText(
  warning: BackupWarning | undefined,
  fallbackMessage: string,
  t: Translator,
  language: string,
): string {
  if (!warning?.code) {
    return fallbackMessage
  }

  const rawCount = warning.count ?? 0
  const count = rawCount.toLocaleString(language)
  const succeeded = (warning.succeeded ?? 0).toLocaleString(language)
  const blobs = (warning.blobs ?? 0).toLocaleString(language)
  const bytes = (warning.bytes ?? 0).toLocaleString(language)
  const diagnostic = warning.diagnostic ?? warning.message
  // Codes whose copy interpolates a value the backend always sends. If it is
  // somehow absent we fall back to the backend's own sentence (which already
  // names the profile / job) rather than rendering a hole — "Skipped : its
  // history database is missing" reads like a bug. Same rule the unknown-code
  // branch below uses: never fabricate, never render blank.
  if (PROFILE_SCOPED_WARNING_CODES.has(warning.code)) {
    return warning.profileId
      ? localizedProfileWarningText(
          warning.code,
          warning.profileId,
          count,
          rawCount,
          diagnostic,
          t,
        )
      : fallbackMessage
  }
  if (warning.code === 'ai-autoindex-queued-while-paused') {
    return warning.jobId
      ? t('audit.warningAiAutoIndexQueuedWhilePaused', { jobId: warning.jobId })
      : fallbackMessage
  }

  switch (warning.code) {
    case 'og-refetch-summary':
      return t(
        rawCount === 1
          ? 'audit.warningOgRefetchSummaryOne'
          : 'audit.warningOgRefetchSummaryMany',
        { count, succeeded },
      )
    case 'og-refetch-failed':
      return `${t('audit.warningOgRefetchFailed')} ${diagnostic}`
    case 'og-prefetch-summary':
      return t(
        rawCount === 1
          ? 'audit.warningOgPrefetchSummaryOne'
          : 'audit.warningOgPrefetchSummaryMany',
        { count, succeeded },
      )
    case 'og-prefetch-failed':
      return `${t('audit.warningOgPrefetchFailed')} ${diagnostic}`
    case 'og-cleanup-summary':
      return t(
        rawCount === 1
          ? 'audit.warningOgCleanupSummaryOne'
          : 'audit.warningOgCleanupSummaryMany',
        { count, blobs, bytes },
      )
    case 'og-cleanup-failed':
      return `${t('audit.warningOgCleanupFailed')} ${diagnostic}`
    case 'intelligence-refresh-failed':
      return `${t('audit.warningIntelligenceRefreshFailed')} ${diagnostic}`
    case 'ai-autoindex-enqueue-failed':
      return `${t('audit.warningAiAutoIndexEnqueueFailed')} ${diagnostic}`
    case 'ai-autoindex-provider-not-ready':
      return `${t('audit.warningAiAutoIndexProviderNotReady')} ${diagnostic}`
    case 'git-history-skipped':
      return `${t('audit.warningGitHistorySkipped')} ${diagnostic}`
    case 'source-evidence-rebuild-needed':
      return `${t('audit.warningSourceEvidenceRebuildNeeded')} ${diagnostic}`
    case 'search-projection-rebuild-needed':
      return `${t('audit.warningSearchProjectionRebuildNeeded')} ${diagnostic}`
    default:
      return fallbackMessage
  }
}
