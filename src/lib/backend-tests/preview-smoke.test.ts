/**
 * @file preview-smoke.test.ts
 * @description High-signal browser-preview smoke coverage for the backend facade.
 * @module lib/backend-tests/preview-smoke
 *
 * ## Responsibilities
 * - Protect the main browser-preview facade flow that exercises the most important commands end to end.
 * - Keep one canonical smoke test for snapshot, backup, history, import, schedule, AI, and integration preview behavior.
 * - Reuse the shared preview config fixture while keeping the transport mock local to this suite.
 *
 * ## Not responsible for
 * - Fine-grained app-lock edge cases; those live in the state-focused suite.
 * - Desktop/Tauri passthrough verification; that lives in the passthrough suite.
 * - Exhaustive fallback branch coverage; that lives in the regressions and workflow suites.
 *
 * ## Dependencies
 * - Depends on `../backend` and the shared preview fixtures in `./test-helpers`.
 * - Mocks `@tauri-apps/api/core` locally so this suite stays browser-preview only.
 *
 * ## Performance notes
 * - This suite intentionally groups one broad smoke path so we keep high-signal preview coverage without booting the desktop runtime.
 */

import { beforeEach, describe, expect, test, vi } from 'vitest'

const { invoke, isTauri } = vi.hoisted(() => ({
  invoke: vi.fn(),
  isTauri: vi.fn(() => false),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke,
  isTauri,
}))

import { backend, backendTestHarness } from '../backend'
import {
  previewConfigFixture as config,
  schedulePlanFixture as schedulePlan,
} from './test-helpers'

describe('backend facade preview smoke', () => {
  beforeEach(() => {
    isTauri.mockReturnValue(false)
    invoke.mockReset()
    backendTestHarness.reset()
  })

  test('covers browser preview commands with deterministic mock data', async () => {
    await expect(backend.getAppSnapshot()).resolves.toMatchObject({
      config: expect.objectContaining({
        archiveMode: 'Encrypted',
        initialized: false,
        enrichment: {
          plugins: expect.arrayContaining([
            expect.objectContaining({ id: 'readable-content-refetch' }),
            expect.objectContaining({ id: 'title-normalization' }),
          ]),
        },
      }),
    })
    await expect(backend.getAppBuildInfo()).resolves.toMatchObject({
      productName: 'PathKeep',
      gitCommitShort: 'preview',
    })
    await expect(backend.saveConfig(config)).resolves.toMatchObject({
      config: expect.objectContaining({ archiveMode: 'Encrypted' }),
    })
    await expect(
      backend.initializeArchive(config, 'key'),
    ).resolves.toMatchObject({
      config: expect.objectContaining({
        archiveMode: 'Encrypted',
        initialized: true,
      }),
      archiveStatus: expect.objectContaining({
        initialized: true,
        unlocked: true,
      }),
    })
    await expect(
      backend.rekeyArchive({ newMode: 'Plaintext', newKey: null }),
    ).resolves.toMatchObject({
      config: expect.objectContaining({ archiveMode: 'Plaintext' }),
    })
    await expect(
      backend.setSessionDatabaseKey('session-key'),
    ).resolves.toBeUndefined()
    await expect(backend.clearSessionDatabaseKey()).resolves.toBeUndefined()
    await expect(backend.runBackupNow()).resolves.toMatchObject({
      dueSkipped: false,
      run: expect.objectContaining({ status: 'success' }),
    })
    await expect(backend.loadDashboardSnapshot()).resolves.toMatchObject({
      totalVisits: 2,
      recentRuns: expect.arrayContaining([
        expect.objectContaining({ status: 'success' }),
      ]),
    })
    await expect(backend.loadAuditRunDetail(1848)).resolves.toMatchObject({
      run: expect.objectContaining({ id: 1848 }),
      artifacts: [expect.objectContaining({ kind: 'snapshot' })],
    })
    await expect(
      backend.previewSnapshotRestore({
        snapshotPath: '/tmp/run-1848.snapshot',
      }),
    ).resolves.toMatchObject({
      snapshotKind: 'raw-source-checkpoint',
      executeSupported: true,
    })
    await expect(
      backend.previewSnapshotRestore({
        snapshotPath: '/tmp/archive-before-rekey.snapshot.sqlite',
      }),
    ).resolves.toMatchObject({
      snapshotKind: 'archive-safety-snapshot',
      executeSupported: false,
    })
    const snapshotRestore = await backend.runSnapshotRestore({
      snapshotPath: '/tmp/run-1848.snapshot',
    })
    expect(snapshotRestore).toMatchObject({
      run: expect.objectContaining({ runType: 'snapshot_restore' }),
    })
    await expect(
      backend.loadAuditRunDetail(snapshotRestore.run!.id),
    ).resolves.toMatchObject({
      artifacts: [
        expect.objectContaining({
          kind: 'snapshot',
          reason: 'restored-source-checkpoint',
        }),
      ],
    })
    await expect(
      backend.runSnapshotRestore({ snapshotPath: '/tmp/run-1848.snapshot' }),
    ).resolves.toMatchObject({
      run: expect.objectContaining({ runType: 'snapshot_restore' }),
    })
    await expect(
      backend.runSnapshotRestore({
        snapshotPath: '/tmp/archive-before-rekey.snapshot.sqlite',
      }),
    ).rejects.toThrow(
      'Automatic restore is only supported for saved browser source checkpoints right now.',
    )
    await expect(backend.previewRetentionPrune()).resolves.toMatchObject({
      buckets: expect.arrayContaining([
        expect.objectContaining({ id: 'snapshots' }),
        expect.objectContaining({ id: 'exports' }),
      ]),
    })
    await expect(
      backend.runRetentionPrune({ bucketIds: ['snapshots', 'exports'] }),
    ).resolves.toMatchObject({
      runId: expect.any(Number),
      buckets: expect.arrayContaining([
        expect.objectContaining({ id: 'snapshots' }),
        expect.objectContaining({ id: 'exports' }),
      ]),
    })
    await expect(
      backend.queryHistory({
        q: 'sqlite',
        domain: null,
        profileId: null,
        browserKind: null,
        startTimeMs: null,
        endTimeMs: null,
        sort: 'newest',
        limit: 10,
      }),
    ).resolves.toMatchObject({ total: 1 })
    await expect(
      backend.queryHistory({
        q: '^https://developer\\.chrome\\.com/.+sqlite$',
        domain: null,
        profileId: null,
        browserKind: null,
        startTimeMs: null,
        endTimeMs: null,
        sort: 'newest',
        limit: 10,
        regexMode: true,
      }),
    ).resolves.toMatchObject({ total: 1 })
    await expect(
      backend.queryHistory({
        q: '^SQLite inspection in browser developer tools$',
        domain: null,
        profileId: null,
        browserKind: null,
        startTimeMs: null,
        endTimeMs: null,
        sort: 'newest',
        limit: 10,
        regexMode: true,
      }),
    ).resolves.toMatchObject({ total: 1 })
    const firstHistoryPage = await backend.queryHistory({
      q: null,
      domain: null,
      profileId: null,
      browserKind: null,
      startTimeMs: null,
      endTimeMs: null,
      sort: 'newest',
      limit: 1,
      cursor: null,
    })
    expect(firstHistoryPage.total).toBe(2)
    expect(firstHistoryPage.items).toHaveLength(1)
    expect(firstHistoryPage.nextCursor).toBeTruthy()
    const secondHistoryPage = await backend.queryHistory({
      q: null,
      domain: null,
      profileId: null,
      browserKind: null,
      startTimeMs: null,
      endTimeMs: null,
      sort: 'newest',
      limit: 1,
      cursor: firstHistoryPage.nextCursor,
    })
    expect(secondHistoryPage).toMatchObject({
      total: 2,
      items: [expect.objectContaining({ id: 2 })],
      nextCursor: null,
    })
    await expect(
      backend.queryHistory({
        q: null,
        domain: null,
        profileId: null,
        browserKind: null,
        startTimeMs: null,
        endTimeMs: null,
        sort: 'newest',
        limit: 1,
        cursor: `${secondHistoryPage.items[0].visitTime}|${secondHistoryPage.items[0].id}`,
      }),
    ).resolves.toMatchObject({
      total: 2,
      items: [],
      page: 3,
      pageSize: 1,
      pageCount: 2,
      hasPrevious: true,
      hasNext: false,
      nextCursor: null,
    })
    await expect(
      backend.exportHistory({
        query: {
          q: 'chromium.+history',
          regexMode: true,
        },
        format: 'jsonl',
      }),
    ).resolves.toMatchObject({ format: 'jsonl', count: 1 })
    await expect(
      backend.exportHistory({ query: { q: 'sqlite' }, format: 'jsonl' }),
    ).resolves.toMatchObject({ format: 'jsonl', count: 1 })
    await expect(
      backend.openExternalUrl('https://example.com/pathkeep'),
    ).resolves.toBe('https://example.com/pathkeep')
    const remotePreview = await backend.previewRemoteBackup()
    expect(remotePreview).toMatchObject({
      bundlePath: expect.stringMatching(/pathkeep-remote-.*\.zip$/),
    })
    await expect(
      backend.verifyRemoteBackup(remotePreview.bundlePath),
    ).resolves.toMatchObject({
      bundlePath: remotePreview.bundlePath,
      objectKey: remotePreview.objectKey,
      bundleVersion: 'pathkeep.remote-backup.v1',
      restoreReady: true,
    })
    await expect(backend.runRemoteBackup()).resolves.toMatchObject({
      uploaded: false,
    })
    await expect(
      backend.inspectTakeout({ sourcePath: '/tmp/takeout', dryRun: true }),
    ).resolves.toMatchObject({
      dryRun: true,
      notes: [
        'Preview includes recognized BrowserHistory rows and quarantined unsupported files.',
      ],
    })
    const imported = await backend.importTakeout({
      sourcePath: '/tmp/takeout',
      dryRun: false,
    })
    expect(imported).toMatchObject({
      dryRun: false,
      importBatch: expect.objectContaining({ id: 1, status: 'imported' }),
    })
    const snapshotAfterImport = await backend.getAppSnapshot()
    expect(snapshotAfterImport.recentRuns[0]).toMatchObject({
      runType: 'import',
      profileScope: ['takeout::browser-history'],
    })
    await expect(
      backend.previewImportBatch(imported.importBatch!.id),
    ).resolves.toMatchObject({
      batch: expect.objectContaining({ id: 1, status: 'imported' }),
    })
    await expect(
      backend.revertImportBatch(imported.importBatch!.id),
    ).resolves.toMatchObject({
      batch: expect.objectContaining({ id: 1, status: 'reverted' }),
    })
    await expect(
      backend.restoreImportBatch(imported.importBatch!.id),
    ).resolves.toMatchObject({
      batch: expect.objectContaining({ id: 1, status: 'imported' }),
    })
    const snapshotAfterRestore = await backend.getAppSnapshot()
    const latestRestoreRun = snapshotAfterRestore.recentRuns[0]
    await expect(
      backend.loadAuditRunDetail(latestRestoreRun.id),
    ).resolves.toMatchObject({
      run: expect.objectContaining({
        id: latestRestoreRun.id,
        runType: 'restore',
      }),
    })
    await expect(backend.previewSchedule()).resolves.toMatchObject({
      platform: 'macos',
      applySupported: false,
    })
    await expect(backend.previewSchedule('windows')).resolves.toMatchObject({
      platform: 'windows',
      generatedFiles: [
        expect.objectContaining({
          relativePath: 'schedule/com.yi-ting.pathkeep.task.xml',
        }),
      ],
    })
    await expect(backend.scheduleStatus('linux')).resolves.toMatchObject({
      platform: 'linux',
      installState: 'manual-review',
      manualSteps: [
        expect.stringContaining('systemd'),
        expect.stringContaining('systemctl --user'),
      ],
    })
    await expect(backend.applySchedule(schedulePlan)).resolves.toMatchObject({
      applied: false,
    })
    await expect(backend.removeSchedule(schedulePlan)).resolves.toMatchObject({
      applied: false,
    })
    await expect(backend.doctor()).resolves.toMatchObject({
      checks: expect.arrayContaining([
        expect.objectContaining({ name: 'import-artifacts' }),
      ]),
    })
    const repair = await backend.repairHealth()
    expect(repair).toMatchObject({
      runId: expect.any(Number),
      repairedImportAudits: 1,
    })
    await expect(
      backend.loadAuditRunDetail(repair.runId!),
    ).resolves.toMatchObject({
      run: expect.objectContaining({
        id: repair.runId,
        runType: 'doctor',
      }),
    })
    await expect(backend.keyringStatus()).resolves.toMatchObject({
      available: true,
      backend: 'Mock keyring',
    })
    await expect(backend.keyringGetDatabaseKey()).resolves.toBeNull()
    await expect(
      backend.keyringStoreDatabaseKey('secret'),
    ).resolves.toMatchObject({
      storedSecret: true,
    })
    await expect(backend.keyringClearDatabaseKey()).resolves.toMatchObject({
      storedSecret: false,
    })
    await expect(
      backend.storeS3Credentials({
        accessKeyId: 'key',
        secretAccessKey: 'secret',
      }),
    ).resolves.toBeUndefined()
    expect((await backend.getAppSnapshot()).config.remoteBackup).toMatchObject({
      credentialsSaved: true,
    })
    await expect(backend.runRemoteBackup()).resolves.toMatchObject({
      uploaded: true,
    })
    await expect(backend.clearS3Credentials()).resolves.toBeUndefined()
    expect((await backend.getAppSnapshot()).config.remoteBackup).toMatchObject({
      credentialsSaved: false,
    })
    await expect(
      backend.storeAiProviderApiKey({
        providerId: 'llm-preview',
        apiKey: 'secret',
      }),
    ).resolves.toMatchObject({
      browserProfiles: expect.arrayContaining([
        expect.objectContaining({
          profileId: 'chrome:Default',
          browserName: 'Google Chrome',
        }),
      ]),
    })
    await expect(
      backend.clearAiProviderApiKey('llm-preview'),
    ).resolves.toMatchObject({
      browserProfiles: expect.arrayContaining([
        expect.objectContaining({
          profileId: 'chrome:Profile 2',
          browserFamily: 'chromium',
        }),
      ]),
    })
    await expect(
      backend.buildAiIndex({
        providerId: 'mock-embedding',
        fullRebuild: false,
        clearOnly: false,
        limit: 20,
      }),
    ).resolves.toMatchObject({
      providerId: 'mock-embedding',
      indexedItems: 2,
    })
    await expect(
      backend.testAiProviderConnection({
        providerId: 'mock-embedding',
        purpose: 'embedding',
      }),
    ).resolves.toMatchObject({
      providerId: 'mock-embedding',
      ok: true,
    })
    await expect(backend.loadAiQueueStatus()).resolves.toMatchObject({
      recentJobs: expect.any(Array),
    })
    await expect(backend.replayAiJob(2)).resolves.toMatchObject({
      id: 2,
      state: expect.stringMatching(/queued|paused/),
    })
    await expect(backend.cancelAiJob(1)).resolves.toMatchObject({
      id: 1,
      state: 'cancelled',
    })
    await expect(
      backend.searchAiHistory({
        query: 'history',
        profileId: null,
        domain: null,
        limit: 3,
      }),
    ).resolves.toMatchObject({
      providerId: 'lexical-fallback',
      total: 2,
      items: [
        expect.objectContaining({
          historyId: 1,
          matchReason: 'Browser preview lexical fixture',
        }),
        expect.objectContaining({
          historyId: 2,
          score: expect.closeTo(0.7, 5),
        }),
      ],
    })
    const firstSemanticPage = await backend.searchAiHistory({
      query: 'history',
      profileId: null,
      domain: null,
      limit: 1,
      cursor: null,
    })
    expect(firstSemanticPage.items).toHaveLength(1)
    expect(firstSemanticPage.nextCursor).toBe('1')
    await expect(
      backend.searchAiHistory({
        query: 'history',
        profileId: null,
        domain: null,
        limit: 1,
        cursor: firstSemanticPage.nextCursor,
      }),
    ).resolves.toMatchObject({
      total: 2,
      items: [expect.objectContaining({ historyId: 2 })],
      nextCursor: null,
    })
    await expect(
      backend.askAiAssistant({
        question: 'What did I read?',
        profileId: null,
        domain: null,
      }),
    ).resolves.toMatchObject({
      providerId: 'preview-llm',
      embeddingProviderId: 'lexical-fallback',
      citations: [
        expect.objectContaining({ historyId: 1 }),
        expect.objectContaining({ historyId: 2 }),
      ],
    })
    await expect(backend.clearDerivedIntelligence()).resolves.toMatchObject({
      clearedVisitDerivedFactRows: 8,
      clearedDailyRollupRows: 11,
      clearedStructuralRows: 27,
      clearedRuntimeRows: 12,
    })
    await expect(backend.loadIntelligenceRuntime()).resolves.toMatchObject({
      queue: expect.objectContaining({ failed: 1 }),
      plugins: expect.arrayContaining([
        expect.objectContaining({ pluginId: 'title-normalization' }),
        expect.objectContaining({ pluginId: 'readable-content-refetch' }),
      ]),
    })
    await expect(backend.previewAiIntegrations()).resolves.toMatchObject({
      mcpCommand: expect.stringContaining('--worker mcp-server'),
      consentSummary: expect.any(String),
      capabilityNotes: expect.arrayContaining([expect.any(String)]),
      scopeBoundary: expect.arrayContaining([expect.any(String)]),
      auditTrace: expect.arrayContaining([expect.any(String)]),
      generatedFiles: expect.arrayContaining([
        expect.objectContaining({
          relativePath: 'integrations/pathkeep-mcp.json',
        }),
      ]),
    })
    await expect(backend.resetLocalSecretVault()).resolves.toBeUndefined()
    await expect(backend.openPathInFileManager('/tmp/pathkeep')).resolves.toBe(
      '/tmp/pathkeep',
    )
    await expect(
      backendTestHarness.call('open_path_in_file_manager'),
    ).resolves.toEqual(expect.stringContaining('com.yi-ting.pathkeep'))
    await expect(backendTestHarness.call('open_external_url')).resolves.toBe(
      'https://example.com',
    )
  })
})
