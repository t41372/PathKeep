/**
 * @file backend-preview-workflows.ts
 * @description Browser-preview workflow and security helpers that keep review-heavy backend commands out of the main dispatcher.
 * @module lib/backend-preview-workflows
 *
 * ## Responsibilities
 * - Build deterministic preview payloads for security, rekey, restore, retention, and takeout workflows.
 * - Mutate the in-memory preview state for import batch revert/restore flows in the same way the browser-preview backend expects.
 * - Reuse shared dashboard/run helpers so workflow previews stay aligned with the rest of the preview fixture surface.
 *
 * ## Not responsible for
 * - Dispatching commands or deciding when a workflow should run; `backend.ts` still owns the command switch.
 * - Owning the mutable preview state lifecycle; that belongs to `backend-preview-state.ts`.
 * - Pretending browser-preview can inspect real archive IO.
 *
 * ## Dependencies
 * - Depends on typed frontend contracts from `./types` and the shared preview state shape from `./backend-preview-state`.
 * - Reuses dashboard/run helpers from `./backend-preview-support` so storage and audit summaries stay consistent.
 *
 * ## Performance notes
 * - These helpers run on preview command paths, so they stay deterministic, bounded, and avoid copying more history state than each workflow response needs.
 */

import type { MockBackendState } from './backend-preview-state'
import {
  buildMockDashboardSnapshot,
  prependMockRun,
} from './backend-preview-support'
import type {
  ImportBatchDetail,
  ImportBatchOverview,
  RekeyPreview,
  RekeyRequest,
  RetentionPreview,
  SecurityStatus,
  SnapshotRestorePreview,
  SnapshotRestoreRequest,
  TakeoutInspection,
} from './types'

/**
 * Reconstructs the archive security summary the Security surface expects after preview mutations.
 *
 * The browser-preview state stores raw config and recent runs, but the page consumes a distilled security
 * report with warnings and last-rekey metadata, so this helper keeps that derivation centralized.
 */
export function buildMockSecurityStatus(
  state: MockBackendState,
): SecurityStatus {
  const warnings = state.snapshot.archiveStatus.warning
    ? [state.snapshot.archiveStatus.warning]
    : []
  const lastRekeyRun =
    state.snapshot.recentRuns.find((run) => run.runType === 'rekey') ?? null

  if (
    state.snapshot.config.archiveMode === 'Encrypted' &&
    state.snapshot.config.rememberDatabaseKeyInKeyring &&
    !state.snapshot.keyringStatus.storedSecret
  ) {
    warnings.push(
      'Archive is encrypted, but the database key is not currently stored in the system keyring.',
    )
  }

  const mode = !state.snapshot.archiveStatus.initialized
    ? 'uninitialized'
    : !state.snapshot.archiveStatus.encrypted
      ? 'plaintext'
      : state.snapshot.archiveStatus.unlocked
        ? 'encrypted'
        : 'locked'

  return {
    initialized: state.snapshot.archiveStatus.initialized,
    mode,
    encrypted: state.snapshot.archiveStatus.encrypted,
    unlocked: state.snapshot.archiveStatus.unlocked,
    databasePath: state.snapshot.archiveStatus.databasePath,
    strongholdPath: state.snapshot.directories.strongholdPath,
    rememberDatabaseKeyInKeyring:
      state.snapshot.config.rememberDatabaseKeyInKeyring,
    lastSuccessfulBackupAt: state.snapshot.archiveStatus.lastSuccessfulBackupAt,
    lastRekeyAt: lastRekeyRun?.finishedAt ?? null,
    lastRekeyRunId: lastRekeyRun?.id ?? null,
    lastRekeySnapshotPath: lastRekeyRun
      ? `${state.snapshot.directories.rawSnapshotsDir}/rekey/archive-before-rekey-${lastRekeyRun.id}.sqlite`
      : null,
    keyringStatus: structuredClone(state.snapshot.keyringStatus),
    warnings,
  }
}

/**
 * Shows the manual review steps and edge-case warnings before preview mode simulates a rekey.
 *
 * Rekey is a high-trust operation, so preview mode should explain why execute might fail or be unsafe
 * instead of pretending every requested mode change is immediately runnable.
 */
export function buildMockRekeyPreview(
  state: MockBackendState,
  request: RekeyRequest,
): RekeyPreview {
  if (!state.snapshot.archiveStatus.initialized) {
    throw new Error(
      'Initialize the archive before previewing a rekey operation.',
    )
  }

  const warnings: string[] = []
  if (
    state.snapshot.archiveStatus.encrypted &&
    !state.snapshot.archiveStatus.unlocked
  ) {
    warnings.push(
      'The archive is currently locked. Unlock it before executing the rekey.',
    )
  }
  if (request.newMode === 'Encrypted' && !request.newKey?.trim()) {
    warnings.push(
      'Encrypted rekey requires a new database key before execute can run.',
    )
  }
  if (state.snapshot.config.archiveMode === request.newMode) {
    warnings.push(
      'The target mode matches the current mode, so PathKeep will treat this as a key rotation / validation pass.',
    )
  }

  return {
    currentMode: state.snapshot.config.archiveMode,
    nextMode: request.newMode,
    requiresNewKey: request.newMode === 'Encrypted',
    snapshotPath: `${state.snapshot.directories.rawSnapshotsDir}/rekey/archive-before-rekey-<timestamp>.sqlite`,
    tempDatabasePath: `${state.snapshot.directories.archiveDatabasePath}.rekey.sqlite`,
    steps: [
      'Create a safety snapshot before rewriting the archive.',
      'Export the archive into a temporary database using the requested target mode.',
      'Swap the rewritten database into place only after the export succeeds.',
    ],
    warnings,
  }
}

/**
 * Distinguishes between automated source-checkpoint restore and archive-safety snapshot review in preview mode.
 *
 * The UI needs different copy and affordances for these two snapshot kinds, so preview mode derives the same
 * shape that desktop does instead of forcing tests to handcraft it.
 */
export function buildMockSnapshotRestorePreview(
  state: MockBackendState,
  request: SnapshotRestoreRequest,
): SnapshotRestorePreview {
  const snapshotPath =
    request.snapshotPath ||
    `${state.snapshot.directories.rawSnapshotsDir}/run-1`
  const archiveSnapshot = snapshotPath.endsWith('.sqlite')
  return {
    snapshotPath,
    snapshotKind: archiveSnapshot
      ? 'archive-safety-snapshot'
      : 'raw-source-checkpoint',
    sourceRunId: state.snapshot.recentRuns[0]?.id ?? null,
    sourceProfileId: archiveSnapshot ? null : 'chrome:Default',
    sourceBrowserName: archiveSnapshot ? null : 'Google Chrome',
    createdAt: new Date().toISOString(),
    reason: archiveSnapshot ? 'before-rekey' : 'periodic-checkpoint',
    executeSupported: !archiveSnapshot,
    estimatedVisits: archiveSnapshot ? 0 : 2,
    estimatedUrls: archiveSnapshot ? 0 : 1,
    estimatedDownloads: archiveSnapshot ? 0 : 0,
    warnings: archiveSnapshot
      ? [
          'This snapshot is a full archive safety copy. PathKeep currently automates restore only for saved browser source checkpoints; keep this file for manual recovery review.',
        ]
      : [
          'Snapshot restore replays the saved browser checkpoint into the current archive. Existing visible archive facts stay in place and duplicate rows are skipped.',
        ],
  }
}

/**
 * Summarizes which local retention buckets can be pruned without touching the canonical archive itself.
 *
 * Retention review reuses dashboard storage totals, so this helper keeps the bucket accounting in one place
 * and makes the prune warning copy consistent with Audit and Settings.
 */
export function buildMockRetentionPreview(
  state: MockBackendState,
): RetentionPreview {
  const dashboard = buildMockDashboardSnapshot(state)
  return {
    buckets: [
      {
        id: 'snapshots',
        bytes: dashboard.storage.snapshotBytes,
        itemCount: 3,
        paths: [state.snapshot.directories.rawSnapshotsDir],
      },
      {
        id: 'exports',
        bytes: dashboard.storage.exportBytes,
        itemCount: 2,
        paths: [state.snapshot.directories.exportsDir],
      },
      {
        id: 'staging',
        bytes: dashboard.storage.stagingBytes,
        itemCount: Math.sign(dashboard.storage.stagingBytes),
        paths: [state.snapshot.directories.stagingDir],
      },
      {
        id: 'quarantine',
        bytes: dashboard.storage.quarantineBytes,
        itemCount: Math.sign(dashboard.storage.quarantineBytes),
        paths: [state.snapshot.directories.quarantineDir],
      },
    ],
    warnings: [
      'Pruning snapshots removes saved restore checkpoints from future Audit review. Manifest and run summaries stay in place.',
      'Export pruning only removes local files under the PathKeep data directory. Remote objects are unchanged.',
    ],
  }
}

/**
 * Produces a deterministic takeout import inspection so import/review flows can stay honest in browser preview mode.
 *
 * Preview mode cannot parse a real archive, so this helper returns representative recognized/quarantined files
 * and, when not in dry-run mode, also mutates the in-memory import batch state the rest of the UI reads.
 */
export function buildMockTakeoutInspection(
  state: MockBackendState,
  sourcePath: string,
  dryRun: boolean,
): TakeoutInspection {
  const previewEntries = [
    {
      sourcePath: `${sourcePath}/Takeout/Chrome/BrowserHistory.json`,
      url: 'https://example.org/archive/trust-ui',
      title: 'PathKeep trust UX notes',
      visitedAt: new Date(Date.now() - 86_400_000).toISOString(),
      sourceVisitId: 41,
      status: dryRun ? 'preview' : 'imported',
    },
    {
      sourcePath: `${sourcePath}/Takeout/Chrome/BrowserHistory.json`,
      url: 'https://example.org/archive/linux-timer',
      title: 'systemd timer notes',
      visitedAt: new Date(Date.now() - 43_200_000).toISOString(),
      sourceVisitId: 42,
      status: dryRun ? 'preview' : 'imported',
    },
  ]
  const recognizedFiles = [
    {
      path: `${sourcePath}/Takeout/Chrome/BrowserHistory.json`,
      kind: 'browser-history',
      status: dryRun ? 'preview' : 'imported',
      records: previewEntries.length,
    },
  ]
  const quarantinedFiles = [
    {
      path: `${sourcePath}/Takeout/Chrome/unsupported.csv`,
      kind: 'unknown',
      status: 'quarantined',
      records: 1,
    },
  ]
  const notes = dryRun
    ? [
        'Preview includes recognized BrowserHistory rows and quarantined unsupported files.',
      ]
    : [
        'Import wrote a local batch and kept unsupported files quarantined for audit review.',
      ]

  if (dryRun) {
    return {
      dryRun: true,
      sourcePath,
      recognizedFiles,
      quarantinedFiles,
      previewEntries,
      candidateItems: 2,
      importedItems: 0,
      duplicateItems: 1,
      notes,
      importBatch: null,
    }
  }

  const batchId = state.nextImportBatchId
  state.nextImportBatchId += 1

  const importedAt = new Date().toISOString()
  const importBatch: ImportBatchOverview = {
    id: batchId,
    sourceKind: 'takeout',
    sourcePath,
    profileId: 'takeout::browser-history',
    createdAt: new Date().toISOString(),
    importedAt,
    revertedAt: null,
    status: 'imported',
    candidateItems: 2,
    importedItems: 2,
    duplicateItems: 1,
    visibleItems: 2,
    auditPath: `${state.snapshot.directories.quarantineDir}/import-batch-${batchId}.json`,
    gitCommit: null,
  }

  state.snapshot.recentImportBatches = [
    importBatch,
    ...state.snapshot.recentImportBatches,
  ]
  prependMockRun(state, {
    id: (state.snapshot.recentRuns[0]?.id ?? batchId) + 1,
    startedAt: importedAt,
    finishedAt: importedAt,
    status: 'success',
    runType: 'import',
    trigger: 'manual',
    profileScope: [importBatch.profileId],
    manifestHash: null,
    profilesProcessed: 1,
    newVisits: importBatch.importedItems,
    newUrls: 0,
    newDownloads: 0,
  })
  state.importBatchDetails[batchId] = {
    batch: importBatch,
    previewEntries,
    recognizedFiles,
    quarantinedFiles,
    notes,
  }

  return {
    dryRun: false,
    sourcePath,
    recognizedFiles,
    quarantinedFiles,
    previewEntries,
    candidateItems: 2,
    importedItems: 2,
    duplicateItems: 1,
    notes,
    importBatch,
  }
}

/**
 * Applies revert/restore transitions to one preview import batch and records the synthetic audit run that follows.
 *
 * Import review screens need a mutable batch owner during tests, so this helper updates batch visibility and
 * history in one place instead of duplicating that state choreography across harnesses.
 */
export function mutateImportBatch(
  state: MockBackendState,
  batchId: number,
  action: 'revert' | 'restore',
): ImportBatchDetail {
  const detail = state.importBatchDetails[batchId]
  if (!detail) {
    throw new Error(`Mock backend does not know import batch ${batchId}`)
  }

  const updatedBatch: ImportBatchOverview = {
    ...detail.batch,
    status: action === 'revert' ? 'reverted' : 'imported',
    revertedAt: action === 'revert' ? new Date().toISOString() : null,
    visibleItems: action === 'revert' ? 0 : detail.batch.importedItems,
  }

  const updatedDetail: ImportBatchDetail = {
    ...detail,
    batch: updatedBatch,
    previewEntries: detail.previewEntries.map((entry) => ({
      ...entry,
      status: action === 'revert' ? 'reverted' : 'imported',
    })),
    notes: [
      action === 'revert'
        ? 'Import batch was reverted from the live archive view.'
        : 'Import batch was restored into the live archive view.',
    ],
  }

  state.importBatchDetails[batchId] = updatedDetail
  state.snapshot.recentImportBatches = state.snapshot.recentImportBatches.map(
    (batch) => (batch.id === batchId ? updatedBatch : batch),
  )
  prependMockRun(state, {
    id: (state.snapshot.recentRuns[0]?.id ?? batchId) + 1,
    startedAt: new Date().toISOString(),
    finishedAt: new Date().toISOString(),
    status: 'success',
    runType: action === 'revert' ? 'rollback' : 'restore',
    trigger: 'manual',
    profileScope: [updatedBatch.profileId],
    manifestHash: null,
    profilesProcessed: 1,
    newVisits: updatedBatch.importedItems,
    newUrls: 0,
    newDownloads: 0,
  })
  return updatedDetail
}
