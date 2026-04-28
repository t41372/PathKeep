/**
 * This module contains reusable front-end helper logic for storage analytics.
 *
 * Why this file exists:
 * - Storage analytics now shares one cross-route truth boundary: core browser
 *   history data vs. everything PathKeep derives or adds around it.
 * - Keeping the grouping logic here prevents Dashboard, Intelligence, and
 *   Settings from quietly drifting into incompatible totals or labels.
 *
 * Main declarations:
 * - `StorageAnalyticsSummary`
 * - `buildStorageAnalyticsSummary`
 * - `dominantStorageGroup`
 * - `storageGrowthEvidence`
 *
 * Source-of-truth notes:
 * - Raw byte counters still come from `StorageSummary`.
 * - This helper only groups those raw counters into the UI contract agreed in
 *   the accepted storage-plane and Intelligence docs.
 */

import type { DashboardSnapshot, StorageSummary } from './types'

export type StorageAnalyticsGroupId = 'coreHistory' | 'otherData'

export type StorageAnalyticsDetailId =
  | 'canonicalArchive'
  | 'sourceEvidence'
  | 'searchProjection'
  | 'intelligenceProjection'
  | 'semanticIndex'
  | 'contentBlobs'
  | 'auditArtifacts'
  | 'exports'
  | 'temporaryFiles'

/**
 * Defines the typed shape for one storage analytics detail row.
 *
 * The route owns the copy, but the helper owns which bytes roll into which row.
 */
export interface StorageAnalyticsDetail {
  id: StorageAnalyticsDetailId
  bytes: number
}

/**
 * Defines the shared summary contract used by Dashboard, Intelligence, and
 * Settings storage surfaces.
 */
export interface StorageAnalyticsSummary {
  trackedStorageBytes: number
  reclaimableBytes: number
  coreHistoryBytes: number
  otherDataBytes: number
  coreBreakdown: StorageAnalyticsDetail[]
  otherBreakdown: StorageAnalyticsDetail[]
}

/**
 * Defines the typed shape for the dominant top-level storage group.
 */
export interface StorageAnalyticsGroup {
  id: StorageAnalyticsGroupId
  bytes: number
}

/**
 * Explains how total tracked storage bytes works.
 */
export function totalTrackedStorageBytes(storage: StorageSummary): number {
  return buildStorageAnalyticsSummary(storage).trackedStorageBytes
}

/**
 * Explains how reclaimable storage bytes works.
 */
export function reclaimableStorageBytes(storage: StorageSummary): number {
  return (
    storage.snapshotBytes +
    storage.exportBytes +
    storage.stagingBytes +
    storage.quarantineBytes +
    storage.semanticSidecarBytes +
    storage.intelligenceBlobBytes
  )
}

/**
 * Builds the shared storage analytics summary from raw byte counters.
 */
export function buildStorageAnalyticsSummary(
  storage: StorageSummary,
): StorageAnalyticsSummary {
  const coreBreakdown: StorageAnalyticsDetail[] = [
    {
      id: 'canonicalArchive',
      bytes: storage.archiveDatabaseBytes,
    },
    {
      id: 'sourceEvidence',
      bytes: storage.sourceEvidenceDatabaseBytes,
    },
  ]
  const otherBreakdown: StorageAnalyticsDetail[] = [
    {
      id: 'searchProjection',
      bytes: storage.searchDatabaseBytes,
    },
    {
      id: 'intelligenceProjection',
      bytes: storage.intelligenceDatabaseBytes,
    },
    {
      id: 'semanticIndex',
      bytes: storage.semanticSidecarBytes,
    },
    {
      id: 'contentBlobs',
      bytes: storage.intelligenceBlobBytes,
    },
    {
      id: 'auditArtifacts',
      bytes: storage.manifestBytes + storage.snapshotBytes,
    },
    {
      id: 'exports',
      bytes: storage.exportBytes,
    },
    {
      id: 'temporaryFiles',
      bytes: storage.stagingBytes + storage.quarantineBytes,
    },
  ]
  const coreHistoryBytes = coreBreakdown.reduce(
    (total, item) => total + item.bytes,
    0,
  )
  const otherDataBytes = otherBreakdown.reduce(
    (total, item) => total + item.bytes,
    0,
  )

  return {
    trackedStorageBytes: coreHistoryBytes + otherDataBytes,
    reclaimableBytes: reclaimableStorageBytes(storage),
    coreHistoryBytes,
    otherDataBytes,
    coreBreakdown,
    otherBreakdown,
  }
}

function storageAnalyticsGroups(
  summary: StorageAnalyticsSummary,
): StorageAnalyticsGroup[] {
  return [
    {
      id: 'coreHistory',
      bytes: summary.coreHistoryBytes,
    },
    {
      id: 'otherData',
      bytes: summary.otherDataBytes,
    },
  ]
}

function isStorageAnalyticsSummary(
  value: StorageSummary | StorageAnalyticsSummary,
): value is StorageAnalyticsSummary {
  return 'coreHistoryBytes' in value && 'otherDataBytes' in value
}

/**
 * Returns the dominant top-level storage group.
 */
export function dominantStorageGroup(
  value: StorageSummary | StorageAnalyticsSummary,
): StorageAnalyticsGroup {
  const summary = isStorageAnalyticsSummary(value)
    ? value
    : buildStorageAnalyticsSummary(value)

  return [...storageAnalyticsGroups(summary)].sort(
    (left, right) => right.bytes - left.bytes,
  )[0]
}

/**
 * Builds growth evidence from the latest successful dashboard run together with
 * the shared storage summary.
 */
export function storageGrowthEvidence(dashboard: DashboardSnapshot | null) {
  const latestRun = dashboard?.recentRuns.find(
    (run) => run.status === 'success',
  )
  const summary = dashboard
    ? buildStorageAnalyticsSummary(dashboard.storage)
    : {
        trackedStorageBytes: 0,
        reclaimableBytes: 0,
        coreHistoryBytes: 0,
        otherDataBytes: 0,
        coreBreakdown: [],
        otherBreakdown: [],
      }

  return {
    latestRunId: latestRun?.id ?? null,
    latestVisitGrowth: latestRun?.newVisits ?? 0,
    latestUrlGrowth: latestRun?.newUrls ?? 0,
    latestDownloadGrowth: latestRun?.newDownloads ?? 0,
    trackedStorageBytes: summary.trackedStorageBytes,
    reclaimableBytes: summary.reclaimableBytes,
    coreHistoryBytes: summary.coreHistoryBytes,
    otherDataBytes: summary.otherDataBytes,
    dominantGroup: dominantStorageGroup(summary),
    summary,
  }
}
