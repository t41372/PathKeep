import type { DashboardSnapshot, StorageSummary } from './types'

export interface StorageAnalyticsSlice {
  id: 'core' | 'audit' | 'exports' | 'rebuildable'
  bytes: number
}

export function totalTrackedStorageBytes(storage: StorageSummary): number {
  return (
    storage.archiveDatabaseBytes +
    storage.manifestBytes +
    storage.snapshotBytes +
    storage.exportBytes +
    storage.stagingBytes +
    storage.quarantineBytes
  )
}

export function reclaimableStorageBytes(storage: StorageSummary): number {
  return storage.exportBytes + storage.stagingBytes + storage.quarantineBytes
}

export function storageAnalyticsSlices(
  storage: StorageSummary,
): StorageAnalyticsSlice[] {
  return [
    {
      id: 'core',
      bytes: storage.archiveDatabaseBytes,
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
      bytes: storage.stagingBytes + storage.quarantineBytes,
    },
  ]
}

export function dominantStorageSlice(storage: StorageSummary) {
  return (
    [...storageAnalyticsSlices(storage)].sort(
      (left, right) => right.bytes - left.bytes,
    )[0] ?? { id: 'core' as const, bytes: 0 }
  )
}

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
