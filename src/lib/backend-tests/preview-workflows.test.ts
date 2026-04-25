/**
 * @file preview-workflows.test.ts
 * @description Focused browser-preview workflow, trust-review, and remote-backup regressions for the backend facade.
 * @module lib/backend-tests/preview-workflows
 *
 * ## Responsibilities
 * - Preserve the preview queue, remote backup, doctor repair, dashboard, history, schedule, export, security, rekey, import, and provider-helper coverage split out of `src/lib/backend.test.ts`.
 * - Keep the browser-preview mock harness assertions aligned with the existing backend facade contract without changing their meaning.
 * - Reuse the shared preview config and schedule fixtures so the split suites stay synchronized on baseline assumptions.
 *
 * ## Not responsible for
 * - Covering Tauri transport passthrough behavior; this suite owns only the preview/mock path.
 * - Deleting or rewriting the original monolithic suite; another change can remove duplication when the coordinated split lands.
 * - Owning shared literal fixtures; `./test-helpers.ts` remains the canonical source.
 *
 * ## Dependencies
 * - Depends on the hoisted Vitest mock for `@tauri-apps/api/core` before importing `../backend`.
 * - Depends on `../backend` and `backendTestHarness` for the preview-state test harness.
 * - Depends on `./test-helpers` for the canonical preview config and schedule plan fixtures.
 *
 * ## Performance notes
 * - Each test resets the preview harness so queue, audit, and schedule mutations cannot leak across cases.
 * - Shared fixtures avoid duplicating large nested literals while preserving the same mock-harness behavior as the source suite.
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
import type { AppConfig } from '../types'
import { previewConfigFixture, schedulePlanFixture } from './test-helpers'

const createPreviewConfig = (): AppConfig =>
  structuredClone(previewConfigFixture)

const createSchedulePlan = () => structuredClone(schedulePlanFixture)

describe('backend facade preview workflows', () => {
  beforeEach(() => {
    isTauri.mockReturnValue(false)
    invoke.mockReset()
    backendTestHarness.reset()
  })

  test('tracks preview queue, provider secrets, remote preview, and doctor repair state transitions', async () => {
    const baseConfig = createPreviewConfig()
    const aiEnabledConfig: AppConfig = {
      ...baseConfig,
      remoteBackup: {
        ...baseConfig.remoteBackup,
        bucket: 'example-bucket',
      },
      ai: {
        ...baseConfig.ai,
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

  test('builds remote backup preview URLs for custom endpoints and AWS host styles', async () => {
    const baseConfig = createPreviewConfig()
    await backend.initializeArchive(
      {
        ...baseConfig,
        archiveMode: 'Plaintext',
        remoteBackup: {
          ...baseConfig.remoteBackup,
          bucket: 'example-bucket',
          region: 'us-west-2',
          endpoint: 'minio.example.test/',
          pathStyle: false,
        },
      },
      null,
    )

    const customVirtualHost = await backend.previewRemoteBackup()
    expect(customVirtualHost.uploadUrl).toMatch(
      /^https:\/\/example-bucket\.minio\.example\.test\/pathkeep\/pathkeep-remote-.*\.zip$/,
    )
    expect(customVirtualHost.warnings).toContain(
      'A custom S3-compatible endpoint is configured. Verify TLS, bucket policy, and path-style compatibility before trusting automatic upload.',
    )

    await backend.saveConfig({
      ...(await backend.getAppSnapshot()).config,
      remoteBackup: {
        ...(await backend.getAppSnapshot()).config.remoteBackup,
        endpoint: 'https://storage.example.test/',
        pathStyle: true,
      },
    })

    const customPathStyle = await backend.previewRemoteBackup()
    expect(customPathStyle.uploadUrl).toMatch(
      /^https:\/\/storage\.example\.test\/example-bucket\/pathkeep\/pathkeep-remote-.*\.zip$/,
    )

    await backend.saveConfig({
      ...(await backend.getAppSnapshot()).config,
      remoteBackup: {
        ...(await backend.getAppSnapshot()).config.remoteBackup,
        endpoint: null,
        pathStyle: false,
      },
    })

    const awsVirtualHost = await backend.previewRemoteBackup()
    expect(awsVirtualHost.uploadUrl).toMatch(
      /^https:\/\/example-bucket\.s3\.us-west-2\.amazonaws\.com\/pathkeep\/pathkeep-remote-.*\.zip$/,
    )

    await backend.saveConfig({
      ...(await backend.getAppSnapshot()).config,
      remoteBackup: {
        ...(await backend.getAppSnapshot()).config.remoteBackup,
        prefix: '   ',
      },
    })

    const awsWithoutPrefix = await backend.previewRemoteBackup()
    expect(awsWithoutPrefix.objectKey).toMatch(/^pathkeep-remote-.*\.zip$/)
    expect(awsWithoutPrefix.uploadUrl).toMatch(
      /^https:\/\/example-bucket\.s3\.us-west-2\.amazonaws\.com\/pathkeep-remote-.*\.zip$/,
    )
  })

  test('reflects disabled readable-content refetch notes and falls back when verify bundle path is missing', async () => {
    const baseConfig = createPreviewConfig()
    await backend.initializeArchive(
      {
        ...baseConfig,
        archiveMode: 'Plaintext',
      },
      null,
    )

    const currentSnapshot = await backend.getAppSnapshot()
    await backend.saveConfig({
      ...currentSnapshot.config,
      enrichment: {
        plugins: [
          {
            id: 'readable-content-refetch',
            enabled: false,
            version: 'm4-v1',
          },
        ],
      },
    })
    backendTestHarness.mutateState((state) => {
      state.history.items[0].title = null
    })

    await expect(
      backend.queryHistory({
        q: '^missing-title-branch$',
        domain: null,
        profileId: null,
        browserKind: null,
        startTimeMs: null,
        endTimeMs: null,
        sort: 'newest',
        limit: 10,
        regexMode: true,
      }),
    ).resolves.toMatchObject({ total: 0 })

    await expect(
      backendTestHarness.call('verify_remote_backup', { bundlePath: 42 }),
    ).resolves.toMatchObject({
      bundlePath: expect.stringMatching(/^\/tmp\/pathkeep-remote-.*\.zip$/),
      restoreReady: true,
    })
  })

  test('covers preview dashboard, history, schedule, and export edge cases through the mock harness', async () => {
    const baseConfig = createPreviewConfig()
    const initializedConfig: AppConfig = {
      ...baseConfig,
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
          relativePath: 'schedule/com.yi-ting.pathkeep.service',
        }),
        expect.objectContaining({
          relativePath: 'schedule/com.yi-ting.pathkeep.timer',
        }),
      ],
    })

    backendTestHarness.seedSchedule({
      ...createSchedulePlan(),
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
        ...createSchedulePlan(),
        platform: 'windows',
        label: 'dev.example.override',
        executablePath: 'C:/PathKeep/pathkeep.exe',
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

    await expect(backend.retryIntelligenceJob(11)).resolves.toMatchObject({
      recentJobs: expect.any(Array),
    })
    await expect(backend.cancelIntelligenceJob(11)).resolves.toMatchObject({
      recentJobs: expect.any(Array),
    })
    await expect(backend.resetLocalSecretVault()).resolves.toBeUndefined()
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

    const baseConfig = createPreviewConfig()
    await backend.initializeArchive(
      {
        ...baseConfig,
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
      ...baseConfig,
      ai: {
        ...baseConfig.ai,
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
    ).resolves.toEqual(expect.stringContaining('com.yi-ting.pathkeep'))
  })

  test('covers Browser Direct and search-rule facade methods through preview commands', async () => {
    const snapshot = await backend.getAppSnapshot()
    const safariProfile = snapshot.browserProfiles.find(
      (profile) => profile.profileId === 'safari:default',
    )
    expect(safariProfile?.historyPath).toBeTruthy()

    await expect(
      backend.inspectBrowserHistory({
        sourcePath: safariProfile?.historyPath ?? '/tmp/History.db',
        dryRun: true,
        browserFamily: 'safari',
        profileId: 'safari:default',
        browserName: 'Safari',
        profileName: 'Safari',
      }),
    ).resolves.toMatchObject({
      dryRun: true,
      sourcePath: safariProfile?.historyPath,
    })

    await expect(
      backend.importBrowserHistory({
        sourcePath: safariProfile?.historyPath ?? '/tmp/History.db',
        dryRun: false,
        browserFamily: 'safari',
        profileId: 'safari:default',
        browserName: 'Safari',
        profileName: 'Safari',
      }),
    ).resolves.toMatchObject({
      dryRun: false,
      importBatch: expect.objectContaining({ status: 'imported' }),
    })

    await expect(
      backend.loadHistoryFavicons([
        {
          profileId: 'chrome:Default',
          url: 'https://developer.chrome.com/docs/devtools/storage/sqlite',
          visitTime: Date.now(),
        },
      ]),
    ).resolves.toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          profileId: 'chrome:Default',
        }),
      ]),
    )

    await expect(backend.listSearchEngineRules()).resolves.toEqual(
      expect.any(Array),
    )
    await expect(
      backend.upsertSearchEngineRule({
        ruleId: 'custom:test',
        engineId: 'test',
        displayName: 'Test Search',
        hostPattern: 'search.example.test',
        pathPrefix: '/search',
        queryParamKey: 'q',
        enabled: true,
        note: null,
        exampleUrl: 'https://search.example.test/search?q=pathkeep',
      }),
    ).resolves.toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          ruleId: 'custom:test',
          displayName: 'Test Search',
        }),
      ]),
    )
    await expect(
      backend.deleteSearchEngineRule('custom:test'),
    ).resolves.not.toEqual(
      expect.arrayContaining([
        expect.objectContaining({ ruleId: 'custom:test' }),
      ]),
    )
  })
})
