/**
 * This module contains reusable front-end helper logic for Storage Analytics.ts.
 *
 * Why this file exists:
 * - Files in `src/lib/` are where UI policy becomes testable without inflating every route component.
 * - If you are trying to understand a front-end contract quickly, these helpers usually explain the reusable part of the story.
 *
 * Main declarations:
 * - `StorageAnalyticsSlice`
 * - `totalTrackedStorageBytes`
 * - `reclaimableStorageBytes`
 * - `storageAnalyticsSlices`
 * - `dominantStorageSlice`
 * - `storageGrowthEvidence`
 *
 * Source-of-truth notes:
 * - Keep helper behavior aligned with the shipping design, feature, and architecture docs rather than local route assumptions.
 * - Avoid burying user-visible copy or route-only workflow rules here unless the helper truly owns that cross-cutting contract.
 */

import type { DashboardSnapshot, StorageSummary } from './types'

/**
 * Defines the typed shape for storage analytics slice.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export interface StorageAnalyticsSlice {
  id: 'core' | 'audit' | 'exports' | 'rebuildable'
  bytes: number
}

/**
 * Explains how total tracked storage bytes works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function totalTrackedStorageBytes(storage: StorageSummary): number {
  return (
    storage.archiveDatabaseBytes +
    storage.sourceEvidenceDatabaseBytes +
    storage.searchDatabaseBytes +
    storage.intelligenceDatabaseBytes +
    storage.manifestBytes +
    storage.snapshotBytes +
    storage.exportBytes +
    storage.stagingBytes +
    storage.quarantineBytes +
    storage.semanticSidecarBytes +
    storage.intelligenceBlobBytes
  )
}

/**
 * Explains how reclaimable storage bytes works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
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
 * Explains how storage analytics slices works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function storageAnalyticsSlices(
  storage: StorageSummary,
): StorageAnalyticsSlice[] {
  return [
    {
      id: 'core',
      bytes:
        storage.archiveDatabaseBytes +
        storage.sourceEvidenceDatabaseBytes +
        storage.searchDatabaseBytes +
        storage.intelligenceDatabaseBytes,
    },
    {
      id: 'audit',
      bytes: storage.manifestBytes + storage.snapshotBytes,
    },
    {
      id: 'exports',
      bytes: storage.exportBytes,
    },
    {
      id: 'rebuildable',
      bytes:
        storage.stagingBytes +
        storage.quarantineBytes +
        storage.semanticSidecarBytes +
        storage.intelligenceBlobBytes,
    },
  ]
}

/**
 * Explains how dominant storage slice works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function dominantStorageSlice(storage: StorageSummary) {
  return (
    [...storageAnalyticsSlices(storage)].sort(
      (left, right) => right.bytes - left.bytes,
    )[0] ?? { id: 'core' as const, bytes: 0 }
  )
}

/**
 * Explains how storage growth evidence works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function storageGrowthEvidence(dashboard: DashboardSnapshot | null) {
  const latestRun = dashboard?.recentRuns.find(
    (run) => run.status === 'success',
  )

  return {
    latestRunId: latestRun?.id ?? null,
    latestVisitGrowth: latestRun?.newVisits ?? 0,
    latestUrlGrowth: latestRun?.newUrls ?? 0,
    latestDownloadGrowth: latestRun?.newDownloads ?? 0,
    totalTrackedBytes: dashboard
      ? totalTrackedStorageBytes(dashboard.storage)
      : 0,
    reclaimableBytes: dashboard
      ? reclaimableStorageBytes(dashboard.storage)
      : 0,
    dominantSlice: dashboard
      ? dominantStorageSlice(dashboard.storage)
      : { id: 'core' as const, bytes: 0 },
  }
}
