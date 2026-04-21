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

import { mockBuildInfo } from './backend-preview-fixtures'
import type { MockBackendState } from './backend-preview-state'
import type {
  AppConfig,
  AuditRunDetail,
  BackupRunOverview,
  ClearDerivedIntelligenceReport,
  DashboardSnapshot,
  RemoteBackupPreview,
  RemoteBackupVerification,
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
 * Derives a deterministic remote bundle path for preview-mode remote backup flows.
 *
 * The returned path is used in both the preview command string and the verification
 * fixture so the two outputs stay in sync.
 */
export function remoteBundlePath() {
  const timestamp = new Date().toISOString().replaceAll(':', '-')
  return `/tmp/pathkeep-remote-${timestamp}.zip`
}

/**
 * Derives the S3 object key used by preview remote backup fixtures.
 *
 * The key respects the configured prefix while preserving the bundle file name
 * so the mocked upload target matches the desktop contract.
 */
export function remoteObjectKey(config: AppConfig, bundlePath: string) {
  const prefix = config.remoteBackup.prefix.trim().replace(/^\/+|\/+$/g, '')
  const fileName = bundlePath.split('/').pop()!
  return prefix ? `${prefix}/${fileName}` : fileName
}

/**
 * Derives the upload URL for the preview remote backup command.
 *
 * The behavior mirrors the browser-preview logic for custom endpoints, path-style
 * addressing, and default AWS S3 endpoint construction.
 */
export function remoteUploadUrl(config: AppConfig, objectKey: string) {
  const trimmedObjectKey = objectKey.replace(/^\/+/, '')
  const endpoint = config.remoteBackup.endpoint?.trim()

  if (endpoint) {
    const normalized =
      endpoint.startsWith('http://') || endpoint.startsWith('https://')
        ? endpoint.replace(/\/+$/g, '')
        : `https://${endpoint.replace(/\/+$/g, '')}`
    if (config.remoteBackup.pathStyle) {
      return `${normalized}/${config.remoteBackup.bucket}/${trimmedObjectKey}`
    }

    const url = new URL(normalized)
    url.hostname = `${config.remoteBackup.bucket}.${url.hostname}`
    return `${url.toString().replace(/\/+$/g, '')}/${trimmedObjectKey}`
  }

  if (config.remoteBackup.pathStyle) {
    return `https://s3.${config.remoteBackup.region}.amazonaws.com/${config.remoteBackup.bucket}/${trimmedObjectKey}`
  }

  return `https://${config.remoteBackup.bucket}.s3.${config.remoteBackup.region}.amazonaws.com/${trimmedObjectKey}`
}

/**
 * Builds the preview remote backup fixture payload.
 *
 * The helper also records the bundle path on mock state so a later verify call can
 * reuse the same deterministic path instead of inventing a different one.
 */
export function previewRemoteBackupFixture(
  state: MockBackendState,
): RemoteBackupPreview {
  const config = state.snapshot.config
  const bundlePath = remoteBundlePath()
  const objectKey = remoteObjectKey(config, bundlePath)
  const uploadUrl = remoteUploadUrl(config, objectKey)
  const warnings = []

  if (config.archiveMode === 'Plaintext') {
    warnings.push(
      'The remote bundle will contain a plaintext archive because local encryption is currently disabled.',
    )
  }
  if (!state.snapshot.config.remoteBackup.credentialsSaved) {
    warnings.push(
      'Remote credentials are not stored yet. Save the access key and secret before using Execute.',
    )
  }
  if (config.remoteBackup.endpoint) {
    warnings.push(
      'A custom S3-compatible endpoint is configured. Verify TLS, bucket policy, and path-style compatibility before trusting automatic upload.',
    )
  }

  state.lastRemoteBundlePath = bundlePath

  return {
    bundlePath,
    objectKey,
    uploadUrl,
    previewCommand: `curl --fail --show-error --aws-sigv4 "aws:amz:${config.remoteBackup.region}:s3" --user "$S3_ACCESS_KEY_ID:$S3_SECRET_ACCESS_KEY" -T '${bundlePath}' '${uploadUrl}'`,
    manualSteps: [
      'Review the bundle path, object key, and upload URL before you trust the destination.',
      'Store S3 credentials in Settings or copy the preview command into your own terminal session.',
      'After execute finishes, run Verify to confirm checksums and restore readiness on the generated bundle.',
    ],
    warnings,
  }
}

/**
 * Builds the preview verification fixture for a remote backup bundle.
 *
 * The helper prefers the last preview-generated bundle path when available so the
 * mock verification story stays coherent across preview and verify actions.
 */
export function verifyRemoteBackupFixture(
  state: MockBackendState,
  bundlePath?: string,
): RemoteBackupVerification {
  const resolvedBundlePath =
    bundlePath ?? state.lastRemoteBundlePath ?? remoteBundlePath()
  const objectKey = remoteObjectKey(state.snapshot.config, resolvedBundlePath)
  return {
    bundlePath: resolvedBundlePath,
    bundleVersion: 'pathkeep.remote-backup.v1',
    appVersion: mockBuildInfo.version,
    createdAt: new Date().toISOString(),
    archiveMode:
      state.snapshot.config.archiveMode === 'Encrypted'
        ? 'encrypted'
        : 'plaintext',
    objectKey,
    restoreReady: true,
    checks: [
      {
        name: 'bundle-manifest',
        status: 'ok',
        message:
          'Bundle manifest exists and declares a supported PathKeep remote bundle version.',
      },
      {
        name: 'checksums',
        status: 'ok',
        message:
          'Preview verification recalculated bundle checksums and found no drift.',
      },
      {
        name: 'restore-validation',
        status: 'ok',
        message:
          'Required archive/config entries are present, so the bundle is restorable in the desktop app.',
      },
    ],
    warnings:
      state.snapshot.config.archiveMode === 'Plaintext'
        ? [
            'Restore validation passed, but the archive inside this bundle stays plaintext at rest.',
          ]
        : [],
    restoreSteps: [
      'Download the bundle to a local disk before attempting restore.',
      'Verify the manifest and archive entries before replacing a live PathKeep archive.',
      'If the archive is encrypted, unlock PathKeep with the current database key before restore.',
    ],
    manifestFiles: [
      {
        relativePath: 'archive/history-vault.sqlite',
        sha256: 'preview-archive-sha256',
        sizeBytes: 146_800_640,
      },
      {
        relativePath: 'config/config.json',
        sha256: 'preview-config-sha256',
        sizeBytes: 4_096,
      },
      {
        relativePath: 'metadata/bundle-manifest.json',
        sha256: 'preview-manifest-sha256',
        sizeBytes: 1_024,
      },
    ],
  }
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

  return {
    generatedAt: new Date().toISOString(),
    totalProfiles: state.snapshot.config.selectedProfileIds.filter(
      (profileId) =>
        profileId.startsWith('chrome:') || profileId.startsWith('arc:'),
    ).length,
    totalUrls: uniqueUrlCount(state.history.items),
    totalVisits: state.history.items.length,
    totalDownloads: state.snapshot.recentRuns[0]?.newDownloads ?? 1,
    lastSuccessfulBackupAt: state.snapshot.archiveStatus.lastSuccessfulBackupAt,
    recentRuns: state.snapshot.recentRuns,
    storage: {
      archiveDatabaseBytes: 146_800_640,
      sourceEvidenceDatabaseBytes: 9_830_400,
      searchDatabaseBytes: 18_432_000,
      intelligenceDatabaseBytes: 24_576_000,
      manifestBytes: 384_000,
      snapshotBytes: 1_228_800,
      exportBytes: 96_000,
      stagingBytes: 0,
      quarantineBytes: 0,
      semanticSidecarBytes: 41_943_040,
      intelligenceBlobBytes: 12_582_912,
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
