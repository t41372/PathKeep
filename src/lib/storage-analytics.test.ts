/**
 * This test file protects the front-end helper and contract logic in Storage Analytics.
 *
 * Why this file exists:
 * - Pure helpers are where we keep UI policy testable without booting the whole shell.
 * - When these tests fail, they usually point at a contract drift that would otherwise show up as subtle route regressions.
 *
 * Main declarations:
 * - This file is mostly internal implementation detail.
 *
 * Source-of-truth notes:
 * - Helper behavior should stay aligned with the same design, feature, and architecture docs that guide the UI surfaces consuming it.
 * - Prefer focused behavioral assertions over snapshotting implementation detail.
 */

import { describe, expect, test } from 'vitest'
import {
  buildStorageAnalyticsSummary,
  dominantStorageGroup,
  reclaimableStorageBytes,
  storageGrowthEvidence,
  totalTrackedStorageBytes,
} from './storage-analytics'

const storage = {
  archiveDatabaseBytes: 150,
  sourceEvidenceDatabaseBytes: 35,
  searchDatabaseBytes: 15,
  intelligenceDatabaseBytes: 25,
  manifestBytes: 10,
  snapshotBytes: 20,
  exportBytes: 5,
  stagingBytes: 3,
  quarantineBytes: 2,
  semanticSidecarBytes: 7,
  intelligenceBlobBytes: 8,
}

describe('storage analytics helpers', () => {
  test('sums tracked and reclaimable storage bytes', () => {
    expect(totalTrackedStorageBytes(storage)).toBe(280)
    expect(reclaimableStorageBytes(storage)).toBe(45)
  })

  test('builds stable core vs other storage summaries and dominant group', () => {
    expect(buildStorageAnalyticsSummary(storage)).toEqual({
      trackedStorageBytes: 280,
      reclaimableBytes: 45,
      coreHistoryBytes: 185,
      otherDataBytes: 95,
      coreBreakdown: [
        { id: 'canonicalArchive', bytes: 150 },
        { id: 'sourceEvidence', bytes: 35 },
      ],
      otherBreakdown: [
        { id: 'searchProjection', bytes: 15 },
        { id: 'intelligenceProjection', bytes: 25 },
        { id: 'semanticIndex', bytes: 7 },
        { id: 'contentBlobs', bytes: 8 },
        { id: 'auditArtifacts', bytes: 30 },
        { id: 'exports', bytes: 5 },
        { id: 'temporaryFiles', bytes: 5 },
      ],
    })
    expect(dominantStorageGroup(storage)).toEqual({
      id: 'coreHistory',
      bytes: 185,
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
      trackedStorageBytes: 280,
      reclaimableBytes: 45,
      coreHistoryBytes: 185,
      otherDataBytes: 95,
      dominantGroup: { id: 'coreHistory', bytes: 185 },
      summary: {
        trackedStorageBytes: 280,
        reclaimableBytes: 45,
        coreHistoryBytes: 185,
        otherDataBytes: 95,
        coreBreakdown: [
          { id: 'canonicalArchive', bytes: 150 },
          { id: 'sourceEvidence', bytes: 35 },
        ],
        otherBreakdown: [
          { id: 'searchProjection', bytes: 15 },
          { id: 'intelligenceProjection', bytes: 25 },
          { id: 'semanticIndex', bytes: 7 },
          { id: 'contentBlobs', bytes: 8 },
          { id: 'auditArtifacts', bytes: 30 },
          { id: 'exports', bytes: 5 },
          { id: 'temporaryFiles', bytes: 5 },
        ],
      },
    })
    expect(storageGrowthEvidence(null)).toEqual({
      latestRunId: null,
      latestVisitGrowth: 0,
      latestUrlGrowth: 0,
      latestDownloadGrowth: 0,
      trackedStorageBytes: 0,
      reclaimableBytes: 0,
      coreHistoryBytes: 0,
      otherDataBytes: 0,
      dominantGroup: { id: 'coreHistory', bytes: 0 },
      summary: {
        trackedStorageBytes: 0,
        reclaimableBytes: 0,
        coreHistoryBytes: 0,
        otherDataBytes: 0,
        coreBreakdown: [],
        otherBreakdown: [],
      },
    })
  })
})
