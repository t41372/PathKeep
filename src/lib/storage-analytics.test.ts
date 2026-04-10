import { describe, expect, test } from 'vitest'
import {
  dominantStorageSlice,
  reclaimableStorageBytes,
  storageAnalyticsSlices,
  storageGrowthEvidence,
  totalTrackedStorageBytes,
} from './storage-analytics'

const storage = {
  archiveDatabaseBytes: 150,
  manifestBytes: 10,
  snapshotBytes: 20,
  exportBytes: 5,
  stagingBytes: 3,
  quarantineBytes: 2,
}

describe('storage analytics helpers', () => {
  test('sums tracked and reclaimable storage bytes', () => {
    expect(totalTrackedStorageBytes(storage)).toBe(190)
    expect(reclaimableStorageBytes(storage)).toBe(30)
  })

  test('returns stable storage slices and dominant category', () => {
    expect(storageAnalyticsSlices(storage)).toEqual([
      { id: 'core', bytes: 150 },
      { id: 'audit', bytes: 30 },
      { id: 'exports', bytes: 5 },
      { id: 'rebuildable', bytes: 5 },
    ])
    expect(dominantStorageSlice(storage)).toEqual({
      id: 'core',
      bytes: 150,
    })
  })

  test('builds growth evidence from the latest successful run', () => {
    expect(
      storageGrowthEvidence({
        generatedAt: '2026-04-08T00:00:00.000Z',
        totalProfiles: 1,
        totalUrls: 2,
        totalVisits: 3,
        totalDownloads: 4,
        lastSuccessfulBackupAt: '2026-04-08T00:00:00.000Z',
        recentRuns: [
          {
            id: 11,
            startedAt: '2026-04-08T00:00:00.000Z',
            finishedAt: '2026-04-08T00:10:00.000Z',
            status: 'success',
            profilesProcessed: 1,
            newVisits: 8,
            newUrls: 3,
            newDownloads: 1,
          },
        ],
        storage,
        nextAction: null,
      }),
    ).toEqual({
      latestRunId: 11,
      latestVisitGrowth: 8,
      latestUrlGrowth: 3,
      latestDownloadGrowth: 1,
      totalTrackedBytes: 190,
      reclaimableBytes: 30,
      dominantSlice: { id: 'core', bytes: 150 },
    })
    expect(storageGrowthEvidence(null)).toEqual({
      latestRunId: null,
      latestVisitGrowth: 0,
      latestUrlGrowth: 0,
      latestDownloadGrowth: 0,
      totalTrackedBytes: 0,
      reclaimableBytes: 0,
      dominantSlice: { id: 'core', bytes: 0 },
    })
  })
})
