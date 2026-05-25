/**
 * @file backend-preview-support.ts
 * @description Preview-mode backend support helpers for remote backup fixtures and browser-preview review snapshots.
 *
 * ## Responsibilities
 * - Derive deterministic remote backup paths, object keys, and upload URLs for preview-mode fixtures.
 * - Build preview-only remote backup verification and dashboard/audit snapshots without touching live backend state.
 * - Mutate the mock backend state for preview-only derived intelligence clearing and recent-run prepending.
 *
 * ## Not responsible for
 * - Owning the browser-preview command facade or transport selection.
 * - Changing desktop runtime behavior, persistence, or real remote-backup execution.
 * - Normalizing unrelated preview fixtures that do not rely on the extracted helper set.
 *
 * ## Dependencies
 * - Depends on browser-preview fixture types from `./types`.
 * - Depends on the mock build info fixture from `./backend-preview-fixtures`.
 * - Depends on the mock backend state shape from `./backend-preview-state`.
 *
 * ## Performance notes
 * - These helpers stay deterministic and cheap; they should not introduce unbounded cloning or extra traversal.
 * - The mock state mutations here are intentionally narrow so preview consumers can reuse them without extra recomputation.
 */

import type { MockBackendState } from './backend-preview-state'
import type {
  AuditRunDetail,
  BackupRunOverview,
  ClearDerivedIntelligenceReport,
  DashboardSnapshot,
} from './types'

/**
 * Returns the count of unique URLs represented in a history item list.
 *
 * The preview dashboard snapshot uses this to keep the total URL number aligned with
 * the deterministic browser-preview history fixture.
 */
function uniqueUrlCount(items: Array<{ url: string }>) {
  return new Set(items.map((item: { url: string }) => item.url)).size
}

/**
 * Clears the derived-intelligence preview fixture and marks the runtime as stale.
 *
 * The returned report mirrors the browser-preview backend's deterministic summary so
 * preview consumers can present a stable result without invoking the real pipeline.
 */
export function clearDerivedIntelligenceFixture(
  state: MockBackendState,
): ClearDerivedIntelligenceReport {
  state.derivedStateCleared = true
  state.snapshot.intelligenceStatus = {
    ready: false,
    lastRunAt: null,
    runs: 0,
    cards: 0,
    topics: 0,
    threads: 0,
    queryGroups: 0,
    referencePages: 0,
    contentCoverage: 0,
    warning: null,
  }
  state.intelligenceRuntime.modules = state.intelligenceRuntime.modules.map(
    (module) => ({
      ...module,
      status: module.enabled ? 'stale' : 'disabled',
      lastInvalidatedAt: new Date().toISOString(),
      staleReason: module.enabled
        ? 'Derived intelligence state was cleared manually.'
        : null,
      notes: module.enabled
        ? [
            'Manual rebuild required before this deterministic module is fresh again.',
          ]
        : ['Disabled in Settings.'],
    }),
  )

  return {
    clearedVisitDerivedFactRows: 8,
    clearedDailyRollupRows: 11,
    clearedStructuralRows: 27,
    clearedRuntimeRows: 12,
    notes: [
      'Only Core Intelligence derived rows, checkpoints, and runtime traces were cleared.',
      'Canonical archive visits, manifests, and import history were left untouched.',
    ],
  }
}

/**
 * Builds the mock dashboard snapshot used by browser-preview consumers.
 *
 * The snapshot keeps totals, storage numbers, and next-action text deterministic so
 * the preview UI can render a stable review surface.
 */
export function buildMockDashboardSnapshot(
  state: MockBackendState,
): DashboardSnapshot {
  if (!state.snapshot.config.initialized) {
    return {
      generatedAt: new Date().toISOString(),
      totalProfiles: 0,
      totalUrls: 0,
      totalVisits: 0,
      totalDownloads: 0,
      lastSuccessfulBackupAt: null,
      recentRuns: state.snapshot.recentRuns,
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
      nextAction:
        'Initialize the archive, confirm at least one Chromium profile, and run the first manual backup.',
    }
  }

  const showcaseTotals = state.showcaseTotals

  return {
    generatedAt: new Date().toISOString(),
    totalProfiles:
      showcaseTotals?.modeledProfiles ??
      state.snapshot.config.selectedProfileIds.filter(
        (profileId) =>
          profileId.startsWith('chrome:') || profileId.startsWith('arc:'),
      ).length,
    totalUrls:
      showcaseTotals?.modeledTotalUrls ?? uniqueUrlCount(state.history.items),
    totalVisits:
      showcaseTotals?.modeledTotalVisits ?? state.history.items.length,
    totalDownloads: state.snapshot.recentRuns[0]?.newDownloads ?? 1,
    lastSuccessfulBackupAt: state.snapshot.archiveStatus.lastSuccessfulBackupAt,
    earliestVisitAt: state.history.items.reduce<string | null>(
      (earliest, item) => {
        const iso = item.visitedAt
        if (!earliest || iso < earliest) return iso
        return earliest
      },
      null,
    ),
    latestVisitAt: state.history.items.reduce<string | null>((latest, item) => {
      const iso = item.visitedAt
      if (!latest || iso > latest) return iso
      return latest
    }, null),
    recentRuns: state.snapshot.recentRuns,
    storage: {
      archiveDatabaseBytes: showcaseTotals ? 777_990_144 : 146_800_640,
      sourceEvidenceDatabaseBytes: showcaseTotals ? 572_641_280 : 9_830_400,
      searchDatabaseBytes: showcaseTotals ? 72_432_000 : 18_432_000,
      intelligenceDatabaseBytes: showcaseTotals ? 94_576_000 : 24_576_000,
      manifestBytes: 384_000,
      snapshotBytes: 1_228_800,
      exportBytes: 96_000,
      stagingBytes: 0,
      quarantineBytes: 0,
      semanticSidecarBytes: 0,
      intelligenceBlobBytes: showcaseTotals ? 18_582_912 : 12_582_912,
    },
    nextAction:
      state.snapshot.recentRuns.length === 0
        ? 'Run the first manual backup to write the manifest chain and explorer index.'
        : null,
  }
}

/**
 * Builds a mock audit run detail for the preview review surface.
 *
 * The helper throws when asked for an unknown run id so preview consumers get the
 * same failure shape they would expect from the deterministic backend fixture.
 */
export function buildMockAuditRunDetail(
  state: MockBackendState,
  runId: number,
): AuditRunDetail {
  const run = state.snapshot.recentRuns.find((item) => item.id === runId)
  if (!run) {
    throw new Error(`Mock backend does not know audit run ${runId}`)
  }

  const artifactPath =
    run.runType === 'rekey'
      ? `${state.snapshot.directories.rawSnapshotsDir}/rekey/archive-before-rekey-${run.id}.sqlite`
      : run.runType === 'snapshot_restore'
        ? `${state.snapshot.directories.rawSnapshotsDir}/chrome:Default/2026-04-09T10-00-00.000Z`
        : run.runType === 'retention_prune'
          ? state.snapshot.directories.appRoot
          : `${state.snapshot.directories.rawSnapshotsDir}/run-${run.id}`
  const artifactReason =
    run.runType === 'rekey'
      ? 'before-rekey'
      : run.runType === 'snapshot_restore'
        ? 'restored-source-checkpoint'
        : run.runType === 'retention_prune'
          ? 'pruned-retention-buckets'
          : 'periodic-checkpoint'
  const artifactKind =
    run.runType === 'retention_prune' ? 'retention' : 'snapshot'

  return {
    run,
    trigger: run.trigger ?? 'manual',
    timezone: 'America/Phoenix',
    dueOnly: false,
    profileScope: run.profileScope ?? state.snapshot.config.selectedProfileIds,
    warnings: [],
    errorMessage: null,
    stats: {
      profilesProcessed: run.profilesProcessed,
      newVisits: run.newVisits,
      newUrls: run.newUrls,
      newDownloads: run.newDownloads,
    },
    manifestPath: `${state.snapshot.directories.manifestsDir}/2026-04-06/run-${run.id}-preview.json`,
    manifestHash: run.manifestHash ?? `preview-manifest-${run.id}`,
    artifacts: [
      {
        kind: artifactKind,
        path: artifactPath,
        checksum: `snapshot-${run.id}`,
        sizeBytes: 4096,
        createdAt: run.finishedAt ?? run.startedAt,
        reason: artifactReason,
      },
    ],
  }
}

/**
 * Prepends a mock run to the preview recent-runs list and returns it.
 *
 * The helper mutates the mock state in place because the preview backend keeps the
 * recent-runs array as its authoritative deterministic view.
 */
export function prependMockRun(
  state: MockBackendState,
  run: BackupRunOverview,
): BackupRunOverview {
  state.snapshot.recentRuns = [run, ...state.snapshot.recentRuns]
  return run
}
