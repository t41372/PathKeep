/**
 * @file helpers.test.ts
 * @description Protects the pure dashboard helper contracts after the route split.
 * @module pages/dashboard
 *
 * ## Responsibilities
 * - Verify browser badge helpers stay stable.
 * - Verify source summaries and stat cards keep the shipped wording and tone behavior.
 *
 * ## Not responsible for
 * - Rendering dashboard panels or route-level loading/error states
 * - Verifying browsing-rhythm or On This Day UI behavior
 *
 * ## Dependencies
 * - Depends only on `helpers.ts`.
 *
 * ## Performance notes
 * - Pure helper tests keep route refactors verifiable without mounting the full shell.
 */

import { describe, expect, test } from 'vitest'
import {
  browserIconClass,
  browserIconLetter,
  buildDashboardStats,
  buildDashboardStorageSegments,
  summarizeRunSources,
} from './helpers'

describe('dashboard helpers', () => {
  const t = (key: string, vars?: Record<string, string | number>) => {
    if (key === 'audit.sourceChrome') return 'Chrome'
    if (key === 'audit.sourceFirefox') return 'Firefox'
    if (key === 'audit.sourceSafari') return 'Safari'
    if (key === 'audit.sourceTakeout') return 'Google Takeout'
    if (key === 'audit.archiveWide') return 'Archive-wide'
    if (key === 'dashboard.totalRecords') return 'Total records'
    if (key === 'dashboard.uniqueUrls') return `${vars?.count} unique URLs`
    if (key === 'dashboard.lastBackup') return 'Last backup'
    if (key === 'common.pending') return 'Pending'
    if (key === 'dashboard.noManifestYet') return 'No manifest yet'
    if (key === 'dashboard.profilesInScope') return 'Profiles in scope'
    if (key === 'dashboard.profilesReadableAttention')
      return `${vars?.readable} readable · ${vars?.attention} attention`
    if (key === 'dashboard.archiveMode') return 'Archive mode'
    if (key === 'dashboard.archiveUnlocked') return 'Unlocked'
    if (key === 'dashboard.archiveNeedsUnlock') return 'Needs unlock'
    if (key === 'common.coreHistory' || key === 'coreHistory')
      return 'Core history'
    if (key === 'common.canonicalArchive' || key === 'canonicalArchive')
      return 'Canonical archive'
    if (key === 'common.sourceEvidence' || key === 'sourceEvidence')
      return 'Source evidence'
    if (key === 'common.otherData' || key === 'otherData') return 'Other data'
    if (key === 'common.searchProjection' || key === 'searchProjection')
      return 'Search projection'
    if (
      key === 'common.intelligenceProjection' ||
      key === 'intelligenceProjection'
    )
      return 'Intelligence projection'
    if (key === 'common.semanticIndex' || key === 'semanticIndex')
      return 'Semantic index'
    if (key === 'common.contentBlobs' || key === 'contentBlobs')
      return 'Content blobs'
    if (key === 'common.auditArtifacts' || key === 'auditArtifacts')
      return 'Audit artifacts'
    if (key === 'common.exports' || key === 'exports') return 'Exports'
    if (key === 'common.temporaryFiles' || key === 'temporaryFiles')
      return 'Temporary files'
    if (key === 'common.modeEncrypted' || key === 'modeEncrypted')
      return 'Encrypted'
    return key
  }

  test('maps browser profile ids to shipped icon classes and letters', () => {
    expect(browserIconClass('chrome:Default')).toBe('chrome')
    expect(browserIconClass('safari:Personal')).toBe('safari')
    expect(browserIconClass('unknown:profile')).toBe('')
    expect(browserIconLetter('firefox:Default')).toBe('F')
    expect(browserIconLetter('arc:Work')).toBe('A')
    expect(browserIconLetter('unknown:profile')).toBe('?')
  })

  test('summarizeRunSources keeps the shipped source strip order', () => {
    expect(summarizeRunSources(['chrome:Default', 'takeout:Google'], t)).toBe(
      'Chrome · Google Takeout',
    )
  })

  test('buildDashboardStats preserves shipped details and unlock tone behavior', () => {
    const stats = buildDashboardStats({
      dashboard: {
        generatedAt: '2026-04-21T00:00:00Z',
        totalProfiles: 3,
        totalUrls: 456,
        totalVisits: 1234,
        totalDownloads: 0,
        lastSuccessfulBackupAt: null,
        recentRuns: [],
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
      },
      snapshot: {
        directories: {} as never,
        runtimeDiagnostics: {} as never,
        config: {
          archiveMode: 'Encrypted',
        } as never,
        archiveStatus: {
          unlocked: false,
        } as never,
        appLockStatus: {} as never,
        keyringStatus: {} as never,
        aiStatus: {} as never,
        intelligenceStatus: {} as never,
        browserProfiles: [],
        recentRuns: [],
        recentImportBatches: [],
      },
      selectedProfilesCount: 3,
      backupReadyProfilesCount: 2,
      previewOnlyProfilesCount: 1,
      language: 'en',
      latestManifestHash: null,
      t,
    })

    expect(stats).toEqual([
      {
        label: 'Total records',
        value: '1,234',
        detail: '456 unique URLs',
        tone: 'accent',
      },
      {
        label: 'Last backup',
        value: 'Pending',
        detail: 'No manifest yet',
        tone: 'neutral',
      },
      {
        label: 'Profiles in scope',
        value: '3',
        detail: '2 readable · 1 attention',
        tone: 'neutral',
      },
      {
        label: 'Archive mode',
        value: 'Encrypted',
        detail: 'Needs unlock',
        tone: 'neutral',
      },
    ])
  })

  test('buildDashboardStorageSegments keeps the shipped two-band storage breakdown', () => {
    expect(
      buildDashboardStorageSegments(t, {
        trackedStorageBytes: 660,
        reclaimableBytes: 350,
        coreHistoryBytes: 30,
        otherDataBytes: 630,
        coreBreakdown: [],
        otherBreakdown: [],
      }),
    ).toEqual([
      {
        label: 'Core history',
        detail: 'Canonical archive · Source evidence',
        tone: 'storage-fill',
        value: 30,
      },
      {
        label: 'Other data',
        detail:
          'Search projection · Intelligence projection · Semantic index · Content blobs · Audit artifacts · Exports · Temporary files',
        tone: 'storage-fill secondary',
        value: 630,
      },
    ])
  })
})
