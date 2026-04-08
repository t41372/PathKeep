import { beforeEach, describe, expect, test, vi } from 'vitest'

const { invoke, isTauri } = vi.hoisted(() => ({
  invoke: vi.fn(),
  isTauri: vi.fn(() => false),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke,
  isTauri,
}))

import { backend, backendTestHarness } from './backend'
import type { AppConfig, SchedulePlan } from './types'

const config: AppConfig = {
  initialized: false,
  archiveMode: 'Encrypted',
  preferredLanguage: 'system',
  dueAfterHours: 72,
  scheduleCheckIntervalHours: 6,
  checkpointDays: 90,
  captureFavicons: true,
  selectedProfileIds: ['chrome:Default'],
  gitEnabled: true,
  rememberDatabaseKeyInKeyring: false,
  appAutostart: false,
  remoteBackup: {
    enabled: false,
    bucket: '',
    region: 'us-east-1',
    endpoint: null,
    prefix: 'pathkeep',
    pathStyle: true,
    uploadAfterBackup: false,
    credentialsSaved: false,
    lastUploadedAt: null,
    lastUploadedObjectKey: null,
    lastError: null,
  },
  enrichment: {
    plugins: [
      {
        id: 'readable-content-refetch',
        enabled: true,
        version: 'm4-v1',
      },
    ],
  },
  ai: {
    enabled: false,
    assistantEnabled: false,
    semanticIndexEnabled: false,
    mcpEnabled: false,
    skillEnabled: false,
    autoIndexAfterBackup: false,
    jobQueuePaused: false,
    jobQueueConcurrency: 1,
    llmProviderId: null,
    embeddingProviderId: null,
    retrievalTopK: 8,
    assistantSystemPrompt:
      'You are an audit-first history research assistant. Use the available browser history evidence before answering. Be explicit about uncertainty and cite the history rows you relied on.',
    llmProviders: [],
    embeddingProviders: [],
  },
}

const schedulePlan: SchedulePlan = {
  platform: 'macos',
  label: 'dev.example.pathkeep.backup',
  executablePath: '/Applications/PathKeep.app',
  generatedFiles: [],
  manualSteps: [],
  applyCommands: [],
  rollbackCommands: [],
  applySupported: false,
}

describe('backend facade', () => {
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
          plugins: [
            expect.objectContaining({ id: 'readable-content-refetch' }),
          ],
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
      recentRuns: [expect.objectContaining({ status: 'success' })],
    })
    await expect(backend.loadAuditRunDetail(1848)).resolves.toMatchObject({
      run: expect.objectContaining({ id: 1848 }),
      artifacts: [expect.objectContaining({ kind: 'snapshot' })],
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
        cursor: firstHistoryPage.nextCursor,
      }),
    ).resolves.toMatchObject({
      total: 2,
      items: [expect.objectContaining({ id: 2 })],
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
    await expect(backend.loadAuditRunDetail(1851)).resolves.toMatchObject({
      run: expect.objectContaining({
        id: 1851,
        runType: 'rollback',
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
          relativePath: 'schedule/pathkeep-backup.xml',
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
    await expect(backend.repairHealth()).resolves.toMatchObject({
      runId: 1852,
      repairedImportAudits: 1,
    })
    await expect(backend.loadAuditRunDetail(1852)).resolves.toMatchObject({
      run: expect.objectContaining({
        id: 1852,
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
    await expect(
      backend.runInsightsNow({
        profileId: 'chrome:Default',
        windowDays: 30,
        fullRebuild: false,
        limit: null,
      }),
    ).resolves.toMatchObject({
      processedVisits: 24,
      cardCount: expect.any(Number),
    })
    await expect(
      backend.loadInsights({
        profileId: 'chrome:Default',
        windowDays: 30,
        fullRebuild: false,
        limit: null,
      }),
    ).resolves.toMatchObject({
      cards: expect.arrayContaining([
        expect.objectContaining({ title: 'Rising topic: archive tooling' }),
      ]),
      workflowMap: expect.objectContaining({ chromiumEnhanced: true }),
    })
    await expect(backend.clearDerivedIntelligence()).resolves.toMatchObject({
      clearedEnrichmentRows: 8,
      clearedFeatureRows: 8,
      clearedCardRows: 2,
    })
    await expect(
      backend.loadInsights({
        profileId: 'chrome:Default',
        windowDays: 30,
        fullRebuild: false,
        limit: null,
      }),
    ).resolves.toMatchObject({
      cards: [],
      topics: [],
      threads: [],
      status: expect.objectContaining({ ready: false }),
    })
    await expect(
      backend.runInsightsNow({
        profileId: 'chrome:Default',
        windowDays: 30,
        fullRebuild: true,
        limit: null,
      }),
    ).resolves.toMatchObject({
      processedVisits: 24,
      cardCount: expect.any(Number),
    })
    await expect(backend.loadThreadDetail('thread-001')).resolves.toMatchObject(
      {
        summary: expect.objectContaining({ threadId: 'thread-001' }),
        visits: expect.arrayContaining([
          expect.objectContaining({ historyId: 1 }),
        ]),
      },
    )
    await expect(
      backend.explainInsight({
        insightId: 'card-rising-topic-1',
        insightKind: 'card',
        profileId: 'chrome:Default',
        windowDays: 30,
      }),
    ).resolves.toSatisfy((value) => {
      expect(value.explanation).toContain('repeated revisits')
      expect(value.usedLlm).toBe(false)
      return true
    })
    await expect(backend.previewAiIntegrations()).resolves.toMatchObject({
      mcpCommand: expect.stringContaining('--worker mcp-server'),
      generatedFiles: [
        expect.objectContaining({
          relativePath: 'integrations/pathkeep-mcp.json',
        }),
      ],
    })
    await expect(backend.resetLocalSecretVault()).resolves.toBeUndefined()
    await expect(backend.openPathInFileManager('/tmp/pathkeep')).resolves.toBe(
      '/tmp/pathkeep',
    )
    await expect(
      backendTestHarness.call('open_path_in_file_manager'),
    ).resolves.toEqual(expect.stringContaining('PathKeep'))
  })

  test('enforces preview archive prerequisites and keeps lock state aligned with archive mode', async () => {
    await expect(backend.runBackupNow()).rejects.toThrow(
      'Initialize the archive before running a backup.',
    )
    await expect(backend.initializeArchive(config)).rejects.toThrow(
      'Mock encrypted archive initialization requires a database key.',
    )

    const plaintextSnapshot = await backend.initializeArchive(
      {
        ...config,
        archiveMode: 'Plaintext',
      },
      null,
    )
    expect(plaintextSnapshot.archiveStatus).toMatchObject({
      encrypted: false,
      initialized: true,
      unlocked: true,
    })

    await backend.clearSessionDatabaseKey()
    expect((await backend.getAppSnapshot()).archiveStatus.unlocked).toBe(true)

    const encryptedSnapshot = await backend.rekeyArchive({
      newMode: 'Encrypted',
      newKey: 'vault-passphrase',
    })
    expect(encryptedSnapshot.archiveStatus).toMatchObject({
      encrypted: true,
      unlocked: true,
    })

    await backend.clearSessionDatabaseKey()
    expect((await backend.getAppSnapshot()).archiveStatus.unlocked).toBe(false)

    await backend.setSessionDatabaseKey('vault-passphrase')
    expect((await backend.getAppSnapshot()).archiveStatus.unlocked).toBe(true)

    await backend.saveConfig({
      ...encryptedSnapshot.config,
      selectedProfileIds: [],
    })
    await expect(backend.runBackupNow()).rejects.toThrow(
      'Select at least one profile before running a backup.',
    )
  })

  test('tracks preview queue, provider secrets, remote preview, and doctor repair state transitions', async () => {
    const aiEnabledConfig: AppConfig = {
      ...config,
      remoteBackup: {
        ...config.remoteBackup,
        bucket: 'example-bucket',
      },
      ai: {
        ...config.ai,
        enabled: true,
        assistantEnabled: true,
        semanticIndexEnabled: true,
        llmProviderId: 'llm-primary',
        embeddingProviderId: 'embed-primary',
        llmProviders: [
          {
            id: 'llm-primary',
            name: 'Primary LLM',
            purpose: 'llm',
            requestFormat: 'openai',
            enabled: true,
            baseUrl: 'http://localhost:11434',
            apiKeySaved: false,
            defaultModel: 'qwen3:8b',
            modelCatalog: [],
            temperature: 0.2,
            maxTokens: 1200,
            dimensions: null,
            notes: null,
          },
        ],
        embeddingProviders: [
          {
            id: 'embed-primary',
            name: 'Primary Embedding',
            purpose: 'embedding',
            requestFormat: 'openai',
            enabled: true,
            baseUrl: 'http://localhost:11434',
            apiKeySaved: false,
            defaultModel: 'nomic-embed-text',
            modelCatalog: [],
            temperature: null,
            maxTokens: null,
            dimensions: 768,
            notes: null,
          },
        ],
      },
    }

    await backend.initializeArchive(aiEnabledConfig, 'vault-passphrase')
    const storedProviders = await backend.storeAiProviderApiKey({
      providerId: 'llm-primary',
      apiKey: 'preview-secret',
    })
    expect(storedProviders.config.ai.llmProviders[0].apiKeySaved).toBe(true)
    const clearedProviders = await backend.clearAiProviderApiKey('llm-primary')
    expect(clearedProviders.config.ai.llmProviders[0].apiKeySaved).toBe(false)

    const preview = await backend.previewRemoteBackup()
    expect(preview).toMatchObject({
      bundlePath: expect.stringMatching(/^\/tmp\/pathkeep-remote-.*\.zip$/),
      objectKey: expect.stringMatching(/^pathkeep\/pathkeep-remote-.*\.zip$/),
    })
    expect(preview.uploadUrl).toContain('example-bucket')
    expect(preview.manualSteps).toEqual([
      'Review the bundle path, object key, and upload URL before you trust the destination.',
      'Store S3 credentials in Settings or copy the preview command into your own terminal session.',
      'After execute finishes, run Verify to confirm checksums and restore readiness on the generated bundle.',
    ])

    const imported = await backend.importTakeout({
      sourcePath: '/tmp/takeout',
      dryRun: false,
    })
    await backend.revertImportBatch(imported.importBatch!.id)
    const repair = await backend.repairHealth()
    expect(repair.repairedImportAudits).toBe(1)
    expect(repair.repairedVisibilityRows).toBe(1)
    expect(repair.clearedDerivedRows).toBe(2)
    expect(repair.runId).toBeTruthy()
    expect((await backend.loadAuditRunDetail(repair.runId!)).run.runType).toBe(
      'doctor',
    )

    const build = await backend.buildAiIndex({
      providerId: 'mock-embedding',
      fullRebuild: false,
      clearOnly: false,
      limit: 20,
    })
    expect(build.jobId).toBeGreaterThan(0)

    const assistant = await backend.askAiAssistant({
      question: 'What did I read?',
      profileId: null,
      domain: null,
    })
    expect(assistant.jobId).toBeGreaterThan(0)
    expect((await backend.loadAiAssistantJob(assistant.jobId!)).jobId).toBe(
      assistant.jobId,
    )

    const beforeDrain = await backend.loadAiQueueStatus()
    expect(
      beforeDrain.recentJobs.some(
        (job) => job.jobType === 'assistant' || job.jobType === 'index-build',
      ),
    ).toBe(true)

    const drained = await backend.runAiQueueJobs()
    expect(
      drained.recentJobs.some(
        (job) => job.summary === 'Preview queue drained this job.',
      ),
    ).toBe(true)

    const replayed = await backend.replayAiJob(assistant.jobId!)
    expect(['queued', 'paused']).toContain(replayed.state)
    const cancelled = await backend.cancelAiJob(replayed.id)
    expect(cancelled.state).toBe('cancelled')
  })

  test('covers preview dashboard, history, schedule, and export edge cases through the mock harness', async () => {
    const initializedConfig: AppConfig = {
      ...config,
      archiveMode: 'Plaintext',
      initialized: true,
      selectedProfileIds: ['chrome:Default', 'arc:Personal', 'firefox:Default'],
    }

    await backend.initializeArchive(initializedConfig, null)

    await expect(backend.loadDashboardSnapshot()).resolves.toMatchObject({
      totalProfiles: 2,
      totalDownloads: 1,
      nextAction: expect.stringContaining('Run the first manual backup'),
    })

    const oldestPage = await backend.queryHistory({
      q: null,
      domain: null,
      profileId: null,
      browserKind: 'chrome',
      startTimeMs: null,
      endTimeMs: null,
      sort: 'oldest',
      limit: 1,
      cursor: null,
    })
    expect(oldestPage.items).toHaveLength(1)
    expect(oldestPage.nextCursor).toBeTruthy()
    await expect(
      backend.queryHistory({
        q: null,
        domain: null,
        profileId: null,
        browserKind: 'chrome',
        startTimeMs: null,
        endTimeMs: null,
        sort: 'oldest',
        limit: 1,
        cursor: oldestPage.nextCursor,
      }),
    ).resolves.toMatchObject({
      total: 2,
      items: [expect.objectContaining({ profileId: 'chrome:Default' })],
      nextCursor: null,
    })

    await expect(
      backend.queryHistory({
        q: null,
        domain: null,
        profileId: null,
        browserKind: 'chrome',
        startTimeMs: null,
        endTimeMs: null,
        sort: 'newest',
        limit: 10,
        cursor: 'invalid-cursor',
      }),
    ).resolves.toSatisfy((page) => {
      expect(page.total).toBe(2)
      expect(page.items).toEqual(
        expect.arrayContaining([
          expect.objectContaining({ profileId: 'chrome:Default' }),
        ]),
      )
      return true
    })

    await expect(backend.previewSchedule('linux')).resolves.toMatchObject({
      platform: 'linux',
      generatedFiles: [
        expect.objectContaining({
          relativePath: 'schedule/pathkeep-backup.service',
        }),
        expect.objectContaining({
          relativePath: 'schedule/pathkeep-backup.timer',
        }),
      ],
    })

    backendTestHarness.seedSchedule({
      platform: 'linux',
      label: 'dev.example.preview',
      executablePath: '/usr/bin/pathkeep',
      generatedFiles: [
        {
          relativePath: 'schedule/pathkeep-backup.timer',
          absolutePath: undefined,
          purpose: 'Timer',
          contents: '[Timer]',
        },
      ],
      manualSteps: [],
      applyCommands: [],
      rollbackCommands: [],
      applySupported: true,
    })
    await expect(backend.scheduleStatus('linux')).resolves.toMatchObject({
      platform: 'linux',
      label: 'dev.example.preview',
      installState: 'installed',
      detectedFiles: ['schedule/pathkeep-backup.timer'],
    })

    backendTestHarness.seedSchedule(
      {
        platform: 'windows',
        label: 'dev.example.override',
        executablePath: 'C:/PathKeep/pathkeep.exe',
        generatedFiles: [],
        manualSteps: [],
        applyCommands: [],
        rollbackCommands: [],
        applySupported: false,
      },
      {
        platform: 'windows',
        label: 'dev.example.override',
        dueAfterHours: 6,
        checkIntervalHours: 1,
        applySupported: false,
        installState: 'permission-warning',
        detectedFiles: ['C:/PathKeep/pathkeep.exe'],
        manualSteps: ['Inspect Task Scheduler manually.'],
        auditPath: 'C:/PathKeep/audit.json',
        lastSuccessfulBackupAt: null,
        warnings: ['Needs administrator review.'],
      },
    )
    await expect(backend.scheduleStatus('windows')).resolves.toMatchObject({
      platform: 'windows',
      installState: 'permission-warning',
      warnings: ['Needs administrator review.'],
    })

    await expect(
      backendTestHarness.call('export_history', {
        request: { query: { q: 'sqlite' } },
      }),
    ).resolves.toMatchObject({
      format: 'jsonl',
      count: 1,
    })
  })

  test('covers preview security, rekey, import, and provider helper edge cases through the mock harness', async () => {
    await expect(
      backend.previewRekeyArchive({ newMode: 'Encrypted', newKey: null }),
    ).rejects.toThrow(
      'Initialize the archive before previewing a rekey operation.',
    )

    await backend.initializeArchive(
      {
        ...config,
        initialized: true,
        rememberDatabaseKeyInKeyring: true,
      },
      'vault-passphrase',
    )
    await backend.clearSessionDatabaseKey()

    await expect(backend.securityStatus()).resolves.toMatchObject({
      mode: 'locked',
      warnings: [
        'Archive is encrypted, but the database key is not currently stored in the system keyring.',
      ],
    })

    await expect(
      backend.previewRekeyArchive({
        newMode: 'Encrypted',
        newKey: '   ',
      }),
    ).resolves.toMatchObject({
      currentMode: 'Encrypted',
      nextMode: 'Encrypted',
      warnings: [
        expect.stringContaining('currently locked'),
        expect.stringContaining('requires a new database key'),
        expect.stringContaining('target mode matches the current mode'),
      ],
    })

    await backend.setSessionDatabaseKey('vault-passphrase')
    await expect(backend.securityStatus()).resolves.toMatchObject({
      mode: 'encrypted',
      warnings: [
        'Archive is encrypted, but the database key is not currently stored in the system keyring.',
      ],
    })

    await expect(
      backendTestHarness.call('keyring_store_database_key'),
    ).resolves.toMatchObject({
      storedSecret: false,
    })
    await expect(
      backend.keyringStoreDatabaseKey('secret'),
    ).resolves.toMatchObject({
      storedSecret: true,
    })
    await expect(
      backendTestHarness.call('keyring_store_database_key'),
    ).resolves.toMatchObject({
      storedSecret: true,
    })

    const aiEnabledConfig: AppConfig = {
      ...config,
      ai: {
        ...config.ai,
        enabled: true,
        assistantEnabled: true,
        semanticIndexEnabled: true,
        llmProviderId: 'llm-primary',
        embeddingProviderId: 'embed-primary',
        llmProviders: [
          {
            id: 'llm-primary',
            name: 'Primary LLM',
            purpose: 'llm',
            requestFormat: 'openai',
            enabled: true,
            baseUrl: null,
            apiKeySaved: false,
            defaultModel: 'gpt-4.1-mini',
            modelCatalog: [],
            temperature: 0.2,
            maxTokens: 800,
            dimensions: null,
            notes: null,
          },
        ],
        embeddingProviders: [
          {
            id: 'embed-primary',
            name: 'Primary Embedding',
            purpose: 'embedding',
            requestFormat: 'openai',
            enabled: true,
            baseUrl: null,
            apiKeySaved: false,
            defaultModel: 'text-embedding-3-large',
            modelCatalog: [],
            temperature: null,
            maxTokens: null,
            dimensions: 1536,
            notes: null,
          },
        ],
      },
    }

    await backend.saveConfig(aiEnabledConfig)
    expect(
      (
        await backend.storeAiProviderApiKey({
          providerId: 'embed-primary',
          apiKey: 'embedding-secret',
        })
      ).config.ai.embeddingProviders[0].apiKeySaved,
    ).toBe(true)
    expect(
      (await backend.clearAiProviderApiKey('embed-primary')).config.ai
        .embeddingProviders[0].apiKeySaved,
    ).toBe(false)

    await expect(backend.previewImportBatch(42)).resolves.toMatchObject({
      batch: expect.objectContaining({ id: 1, status: 'imported' }),
    })
    await expect(backend.revertImportBatch(42)).rejects.toThrow(
      'Mock backend does not know import batch 42',
    )
    await expect(backend.loadAuditRunDetail(9999)).rejects.toThrow(
      'Mock backend does not know audit run 9999',
    )

    await expect(
      backendTestHarness.call('test_ai_provider_connection'),
    ).resolves.toMatchObject({
      providerId: 'preview-provider',
      purpose: 'embedding',
      ok: true,
    })
    await expect(
      backendTestHarness.call('open_path_in_file_manager'),
    ).resolves.toEqual(expect.stringContaining('PathKeep'))
  })

  test('covers remaining preview fallback branches through seeded mock state', async () => {
    await expect(backend.securityStatus()).resolves.toMatchObject({
      mode: 'uninitialized',
    })
    await expect(backend.doctor()).resolves.toMatchObject({
      checks: [
        expect.objectContaining({
          name: 'import-artifacts',
          status: 'info',
        }),
        expect.objectContaining({
          name: 'visibility-state',
          status: 'ok',
        }),
      ],
    })
    await expect(backend.repairHealth()).resolves.toSatisfy((report) => {
      expect(report.runId).toBeGreaterThan(0)
      expect(report.repairedImportAudits).toBe(0)
      expect(report.repairedVisibilityRows).toBe(0)
      expect(report.clearedDerivedRows).toBe(0)
      return true
    })

    await backend.initializeArchive(
      {
        ...config,
        archiveMode: 'Plaintext',
        initialized: true,
      },
      null,
    )
    await expect(backend.securityStatus()).resolves.toMatchObject({
      mode: 'plaintext',
    })

    await expect(backend.scheduleStatus('windows')).resolves.toMatchObject({
      platform: 'windows',
      manualSteps: [
        expect.stringContaining('Task Scheduler'),
        expect.stringContaining('XML'),
      ],
      warnings: [expect.stringContaining('Task Scheduler')],
    })

    backendTestHarness.seedSchedule({
      platform: 'windows',
      label: 'dev.example.windows',
      executablePath: 'C:/PathKeep/pathkeep.exe',
      generatedFiles: [],
      manualSteps: ['Use the documented Task Scheduler import flow.'],
      applyCommands: [],
      rollbackCommands: [],
      applySupported: false,
    })
    await expect(backend.scheduleStatus('windows')).resolves.toMatchObject({
      installState: 'manual-review',
      manualSteps: ['Use the documented Task Scheduler import flow.'],
    })

    backendTestHarness.mutateState((state) => {
      state.snapshot.archiveStatus.warning = 'Preview archive warning.'
      state.snapshot.recentRuns = [
        {
          id: 900,
          startedAt: '2026-04-07T00:00:00Z',
          finishedAt: null,
          status: 'success',
          runType: 'backup',
          trigger: undefined,
          profileScope: undefined,
          manifestHash: null,
          profilesProcessed: 0,
          newVisits: 0,
          newUrls: 0,
          newDownloads: 0,
        },
      ]
      state.history.items = [
        ...state.history.items,
        {
          ...state.history.items[0],
          id: 99,
          profileId: 'standalone',
          url: 'https://example.test/unrelated-url',
          title: 'SQLite title match only',
          domain: 'example.test',
          visitTime: state.history.items[0].visitTime - 5000,
          visitedAt: new Date(
            state.history.items[0].visitTime - 5000,
          ).toISOString(),
        },
        {
          ...state.history.items[0],
          id: 100,
          profileId: 'chrome:NoTitle',
          url: 'https://example.test/no-title-row',
          title: null,
          domain: 'example.test',
          visitTime: state.history.items[0].visitTime - 10_000,
          visitedAt: new Date(
            state.history.items[0].visitTime - 10_000,
          ).toISOString(),
        },
      ]
      state.snapshot.config.ai.jobQueuePaused = true
    })

    await expect(backend.securityStatus()).resolves.toMatchObject({
      warnings: ['Preview archive warning.'],
    })

    await expect(backend.loadAuditRunDetail(900)).resolves.toMatchObject({
      trigger: 'manual',
      profileScope: ['chrome:Default'],
      artifacts: [
        expect.objectContaining({
          createdAt: '2026-04-07T00:00:00Z',
        }),
      ],
    })
    await expect(
      backendTestHarness.call('load_audit_run_detail'),
    ).resolves.toMatchObject({
      run: expect.objectContaining({ id: 900 }),
    })

    await expect(
      backend.queryHistory({
        q: 'title match only',
        domain: 'example.test',
        profileId: 'standalone',
        browserKind: 'standalone',
        startTimeMs: null,
        endTimeMs: null,
        sort: 'newest',
        limit: 10,
        cursor: null,
      }),
    ).resolves.toMatchObject({
      total: 1,
      items: [expect.objectContaining({ id: 99 })],
    })
    await expect(
      backend.queryHistory({
        q: 'title-does-not-exist',
        domain: 'example.test',
        profileId: 'chrome:NoTitle',
        browserKind: 'chrome',
        startTimeMs: null,
        endTimeMs: null,
        sort: 'newest',
        limit: 10,
        cursor: null,
      }),
    ).resolves.toMatchObject({
      total: 0,
      items: [],
    })

    await expect(
      backend.searchAiHistory({
        query: 'history',
        profileId: null,
        domain: null,
      }),
    ).resolves.toMatchObject({
      total: expect.any(Number),
      nextCursor: null,
    })

    await expect(
      backendTestHarness.call('import_takeout'),
    ).resolves.toMatchObject({
      sourcePath: '/tmp/takeout.zip',
      dryRun: false,
    })
    await expect(
      backendTestHarness.call('preview_import_batch'),
    ).resolves.toMatchObject({
      batch: expect.objectContaining({ id: 1 }),
    })
    backendTestHarness.mutateState((state) => {
      state.snapshot.recentRuns = []
    })
    await expect(
      backendTestHarness.call('revert_import_batch'),
    ).resolves.toMatchObject({
      batch: expect.objectContaining({ id: 1, status: 'reverted' }),
    })
    await expect(
      backendTestHarness.call('restore_import_batch'),
    ).resolves.toMatchObject({
      batch: expect.objectContaining({ id: 1, status: 'imported' }),
    })

    await expect(backend.replayAiJob(2)).resolves.toMatchObject({
      id: 2,
      state: 'paused',
    })

    backendTestHarness.mutateState((state) => {
      state.snapshot.recentRuns = []
    })
    await expect(
      backendTestHarness.call('load_audit_run_detail'),
    ).rejects.toThrow('1848')
  })

  test('delegates to Tauri invoke when running inside the desktop shell', async () => {
    isTauri.mockReturnValue(true)
    invoke.mockResolvedValue({ ok: true })

    await expect(backend.getAppSnapshot()).resolves.toEqual({ ok: true })
    expect(invoke).toHaveBeenCalledWith('app_snapshot', undefined)
  })

  test('passes explicit AI command payloads through to Tauri invoke', async () => {
    isTauri.mockReturnValue(true)
    invoke.mockResolvedValue({ ok: true })

    const buildRequest = {
      providerId: 'embed-primary',
      fullRebuild: true,
      clearOnly: false,
      limit: 10,
    }
    const providerRequest = {
      providerId: 'embed-primary',
      purpose: 'embedding' as const,
    }
    const searchRequest = {
      query: 'browser history backup',
      profileId: 'chrome:Default',
      domain: 'example.com',
      limit: 5,
    }
    const assistantRequest = {
      question: 'What did I read?',
      profileId: 'chrome:Default',
      domain: 'example.com',
    }

    await expect(backend.clearAiProviderApiKey('llm-primary')).resolves.toEqual(
      {
        ok: true,
      },
    )
    await expect(backend.buildAiIndex(buildRequest)).resolves.toEqual({
      ok: true,
    })
    await expect(
      backend.testAiProviderConnection(providerRequest),
    ).resolves.toEqual({
      ok: true,
    })
    await expect(backend.loadAiQueueStatus()).resolves.toEqual({
      ok: true,
    })
    await expect(backend.runAiQueueJobs(2)).resolves.toEqual({
      ok: true,
    })
    await expect(backend.replayAiJob(12)).resolves.toEqual({
      ok: true,
    })
    await expect(backend.cancelAiJob(12)).resolves.toEqual({
      ok: true,
    })
    await expect(backend.searchAiHistory(searchRequest)).resolves.toEqual({
      ok: true,
    })
    await expect(backend.askAiAssistant(assistantRequest)).resolves.toEqual({
      ok: true,
    })

    expect(invoke).toHaveBeenNthCalledWith(1, 'clear_ai_provider_api_key', {
      providerId: 'llm-primary',
    })
    expect(invoke).toHaveBeenNthCalledWith(2, 'build_ai_index', {
      request: buildRequest,
    })
    expect(invoke).toHaveBeenNthCalledWith(3, 'test_ai_provider_connection', {
      request: providerRequest,
    })
    expect(invoke).toHaveBeenNthCalledWith(4, 'load_ai_queue_status', undefined)
    expect(invoke).toHaveBeenNthCalledWith(5, 'run_ai_queue_jobs', {
      maxJobs: 2,
    })
    expect(invoke).toHaveBeenNthCalledWith(6, 'replay_ai_job', {
      jobId: 12,
    })
    expect(invoke).toHaveBeenNthCalledWith(7, 'cancel_ai_job', {
      jobId: 12,
    })
    expect(invoke).toHaveBeenNthCalledWith(8, 'search_ai_history', {
      request: searchRequest,
    })
    expect(invoke).toHaveBeenNthCalledWith(9, 'ask_ai_assistant', {
      request: assistantRequest,
    })
  })

  test('passes schedule, security, remote, and insights payloads through to Tauri invoke', async () => {
    isTauri.mockReturnValue(true)
    invoke.mockResolvedValue({ ok: true })

    const insightRequest = {
      profileId: 'chrome:Default',
      windowDays: 30,
      fullRebuild: false,
      limit: null,
    }
    const explainRequest = {
      insightId: 'card-rising-topic-1',
      insightKind: 'card' as const,
      profileId: 'chrome:Default',
      windowDays: 30,
    }

    await expect(backend.previewSchedule('linux')).resolves.toEqual({
      ok: true,
    })
    await expect(backend.scheduleStatus('macos')).resolves.toEqual({ ok: true })
    await expect(
      backend.applySchedule({ ...schedulePlan, applySupported: true }),
    ).resolves.toEqual({ ok: true })
    await expect(
      backend.removeSchedule({ ...schedulePlan, applySupported: true }),
    ).resolves.toEqual({ ok: true })
    await expect(backend.keyringStatus()).resolves.toEqual({ ok: true })
    await expect(backend.securityStatus()).resolves.toEqual({ ok: true })
    await expect(backend.previewRemoteBackup()).resolves.toEqual({ ok: true })
    await expect(backend.runRemoteBackup()).resolves.toEqual({ ok: true })
    await expect(backend.previewImportBatch(7)).resolves.toEqual({ ok: true })
    await expect(backend.revertImportBatch(7)).resolves.toEqual({ ok: true })
    await expect(backend.restoreImportBatch(7)).resolves.toEqual({ ok: true })
    await expect(backend.runInsightsNow(insightRequest)).resolves.toEqual({
      ok: true,
    })
    await expect(backend.loadInsights(insightRequest)).resolves.toEqual({
      ok: true,
    })
    await expect(backend.loadThreadDetail('thread-001')).resolves.toEqual({
      ok: true,
    })
    await expect(backend.explainInsight(explainRequest)).resolves.toEqual({
      ok: true,
    })
    await expect(backend.previewAiIntegrations()).resolves.toEqual({ ok: true })
    await expect(
      backend.openPathInFileManager('/tmp/pathkeep'),
    ).resolves.toEqual({
      ok: true,
    })

    expect(invoke).toHaveBeenNthCalledWith(1, 'preview_schedule', {
      platform: 'linux',
    })
    expect(invoke).toHaveBeenNthCalledWith(2, 'schedule_status', {
      platform: 'macos',
    })
    expect(invoke).toHaveBeenNthCalledWith(3, 'apply_schedule', {
      plan: { ...schedulePlan, applySupported: true },
    })
    expect(invoke).toHaveBeenNthCalledWith(4, 'remove_schedule', {
      plan: { ...schedulePlan, applySupported: true },
    })
    expect(invoke).toHaveBeenNthCalledWith(5, 'keyring_status', undefined)
    expect(invoke).toHaveBeenNthCalledWith(6, 'security_status', undefined)
    expect(invoke).toHaveBeenNthCalledWith(
      7,
      'preview_remote_backup',
      undefined,
    )
    expect(invoke).toHaveBeenNthCalledWith(8, 'run_remote_backup', undefined)
    expect(invoke).toHaveBeenNthCalledWith(9, 'preview_import_batch', {
      batchId: 7,
    })
    expect(invoke).toHaveBeenNthCalledWith(10, 'revert_import_batch', {
      batchId: 7,
    })
    expect(invoke).toHaveBeenNthCalledWith(11, 'restore_import_batch', {
      batchId: 7,
    })
    expect(invoke).toHaveBeenNthCalledWith(12, 'run_insights_now', {
      request: insightRequest,
    })
    expect(invoke).toHaveBeenNthCalledWith(13, 'load_insights', {
      request: insightRequest,
    })
    expect(invoke).toHaveBeenNthCalledWith(14, 'load_thread_detail', {
      threadId: 'thread-001',
    })
    expect(invoke).toHaveBeenNthCalledWith(15, 'explain_insight', {
      request: explainRequest,
    })
    expect(invoke).toHaveBeenNthCalledWith(
      16,
      'preview_ai_integrations',
      undefined,
    )
    expect(invoke).toHaveBeenNthCalledWith(17, 'open_path_in_file_manager', {
      path: '/tmp/pathkeep',
    })
  })

  test('throws when a mock command is not implemented in browser preview mode', async () => {
    await expect(
      backendTestHarness.call('inspect_takeout'),
    ).resolves.toMatchObject({
      sourcePath: '/tmp/takeout.zip',
      dryRun: true,
    })
    await expect(
      backendTestHarness.call('totally_unknown_command'),
    ).rejects.toThrow('Mock backend does not implement totally_unknown_command')
  })
})
