/**
 * @file shell-data-helpers.ts
 * @description Pure helper functions that keep the shell-data provider focused on state orchestration instead of inline policy math.
 * @module app/shell-data
 *
 * ## Responsibilities
 * - Hold shell bootstrap fallback decisions that do not need React state.
 * - Normalize runtime polling scope, queue counts, and backup-progress overlay mapping.
 * - Keep app-lock error detection and best-effort keyring unlock policy in one readable owner.
 *
 * ## Not responsible for
 * - Performing backend calls or mutating React state.
 * - Owning provider actions such as save, backup, or unlock flows.
 *
 * ## Dependencies
 * - Depends on the shared shell-data context types and front-end backend response types.
 * - Consumed by `shell-data.tsx` and the shell-data action owner.
 *
 * ## Performance notes
 * - These helpers stay allocation-light and synchronous because they run on shell bootstrap and recurring runtime polling paths.
 */

import type {
  AppSnapshot,
  BackupProgressEvent,
  DashboardSnapshot,
} from '../lib/types'
import type { BusyOverlayState, ShellRuntimeStatus } from './shell-data-context'

/**
 * The shell only needs the translator contract, not the full i18n context object.
 *
 * Keeping this small prevents helper callers from dragging extra context types into
 * pure logic paths that only need to map keys to already-shipped strings.
 */
export type ShellTranslator = (
  key: string,
  vars?: Record<string, string | number>,
) => string

/**
 * Detects backend errors that really mean "the app is locked" so the shell can
 * jump back to the explicit App Lock flow instead of showing a generic read error.
 *
 * The regex is intentionally narrow: this helper should only catch known unlock
 * guidance and leave unrelated archive failures visible.
 */
export function isAppLockError(error: unknown) {
  return (
    error instanceof Error &&
    /currently locked|unlock the app|unlock pathkeep/i.test(error.message)
  )
}

/**
 * Builds a dashboard placeholder for first-run or not-yet-initialized archives.
 *
 * This preserves enough shell structure to reach onboarding even when the dashboard
 * read model fails before the archive exists.
 */
export function buildUninitializedDashboardFallback(
  snapshot: AppSnapshot,
): DashboardSnapshot {
  return {
    generatedAt: new Date().toISOString(),
    totalProfiles: 0,
    totalUrls: 0,
    totalVisits: 0,
    totalDownloads: 0,
    lastSuccessfulBackupAt: null,
    recentRuns: snapshot.recentRuns,
    storage: {
      archiveDatabaseBytes: 0,
      sourceEvidenceDatabaseBytes: 0,
      searchDatabaseBytes: 0,
      intelligenceDatabaseBytes: 0,
      manifestBytes: 0,
      snapshotBytes: 0,
      exportBytes: 0,
      stagingBytes: 0,
      quarantineBytes: 0,
      semanticSidecarBytes: 0,
      intelligenceBlobBytes: 0,
    },
    nextAction: null,
  }
}

/**
 * Decides whether bootstrap should attempt a best-effort keyring unlock.
 *
 * This only returns true when the archive is encrypted, currently locked, and the
 * user explicitly opted into remembering the database key in a platform keyring.
 */
export function shouldAttemptKeyringAutoUnlock(snapshot: AppSnapshot) {
  return (
    snapshot.archiveStatus.encrypted &&
    !snapshot.archiveStatus.unlocked &&
    snapshot.config.rememberDatabaseKeyInKeyring &&
    snapshot.keyringStatus.available &&
    snapshot.keyringStatus.storedSecret
  )
}

/**
 * Returns the neutral runtime status used whenever the archive is locked,
 * uninitialized, or polling has not started yet.
 *
 * The shell reuses this shape frequently, so a single helper keeps the idle
 * state contract consistent across bootstrap, lock, and refresh flows.
 */
export function emptyRuntimeStatus(): ShellRuntimeStatus {
  return {
    aiQueue: null,
    intelligence: null,
    loading: false,
    error: null,
  }
}

/**
 * Builds the dedupe key used by shell runtime polling.
 *
 * The key intentionally ignores view-only shell state and only changes when the
 * unlocked archive path, selected profile scope, or queue pause state changes.
 */
export function runtimeStatusScopeKey(snapshot: AppSnapshot | null) {
  if (!snapshot?.config.initialized || !snapshot.archiveStatus.unlocked) {
    return 'locked-or-uninitialized'
  }

  return [
    snapshot.archiveStatus.databasePath,
    snapshot.config.selectedProfileIds.join(','),
    snapshot.config.ai.jobQueuePaused ? 'paused' : 'live',
  ].join('|')
}

/**
 * Counts all active runtime jobs that should keep shell polling on the shorter cadence.
 *
 * This is the shell's "stay warm or back off" heuristic, so it intentionally sums
 * AI queue work and deterministic runtime queue work into one number.
 */
export function countActiveRuntimeJobs(status: ShellRuntimeStatus) {
  return (
    (status.aiQueue?.queued ?? 0) +
    (status.aiQueue?.running ?? 0) +
    (status.intelligence?.queue.queued ?? 0) +
    (status.intelligence?.queue.running ?? 0)
  )
}

/**
 * Returns the shared three-step backup grammar shown in the shell busy overlay.
 *
 * The labels stay centralized so manual backup flows and live progress events use
 * the same ordering and wording.
 */
export function backupStepLabels(t: ShellTranslator) {
  return [
    t('shell.backupStepPrepare'),
    t('shell.backupStepArchive'),
    t('shell.backupStepRefresh'),
  ]
}

/**
 * Converts a low-level backup progress event into the human-readable busy-overlay state.
 *
 * The shell uses this to keep PME feedback honest: users should see which phase the
 * run is in, which profile is active, and an approximate completion number without
 * reading raw worker events.
 */
export function buildBackupOverlay(
  progress: BackupProgressEvent,
  t: ShellTranslator,
): BusyOverlayState {
  const backupSteps = backupStepLabels(t)
  const stepProgress =
    progress.totalSteps > 0
      ? (Math.min(progress.step + 1, progress.totalSteps) /
          progress.totalSteps) *
        100
      : null
  const profileCurrent =
    progress.phase === 'stage-profile' || progress.phase === 'ingest-profile'
      ? progress.completedProfiles + 1
      : progress.completedProfiles
  const profileDetail =
    progress.profileId && progress.totalProfiles > 0
      ? t('shell.backupProfileProgress', {
          profileId: progress.profileId,
          current: profileCurrent,
          total: progress.totalProfiles,
        })
      : null
  const progressLabel =
    progress.totalProfiles > 0
      ? `${profileCurrent.toLocaleString()} / ${progress.totalProfiles.toLocaleString()}`
      : `${Math.min(progress.step + 1, progress.totalSteps).toLocaleString()} / ${progress.totalSteps.toLocaleString()}`

  switch (progress.phase) {
    case 'prepare': {
      const detail = t('shell.runningManualBackupDetail')
      return {
        label: t('shell.runningManualBackup'),
        detail,
        progressLabel,
        progressValue: stepProgress,
        steps: backupSteps,
        activeStep: 0,
        logLines: [detail],
      }
    }
    case 'stage-profile':
    case 'ingest-profile': {
      const detail = profileDetail ?? t('shell.backupWritingArchiveDetail')
      return {
        label: t('shell.backupWritingArchive'),
        detail,
        progressLabel,
        progressValue:
          progress.totalProfiles > 0
            ? (profileCurrent / progress.totalProfiles) * 100
            : stepProgress,
        steps: backupSteps,
        activeStep: 1,
        logLines: [detail],
      }
    }
    case 'finalize': {
      const detail = t('shell.backupFinalizeProgress', {
        current: progress.completedProfiles,
        total: progress.totalProfiles,
      })
      return {
        label: t('shell.refreshingArchiveViews'),
        detail,
        progressLabel,
        progressValue:
          progress.totalProfiles > 0
            ? (progress.completedProfiles / progress.totalProfiles) * 100
            : stepProgress,
        steps: backupSteps,
        activeStep: 2,
        logLines: [detail],
      }
    }
    default: {
      const detail = t('shell.runningManualBackupDetail')
      return {
        label: t('shell.runningManualBackup'),
        detail,
        progressLabel,
        progressValue: stepProgress,
        steps: backupSteps,
        activeStep: 0,
        logLines: [detail],
      }
    }
  }
}
