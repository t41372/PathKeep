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
  ai: {
    enabled: false,
    assistantEnabled: false,
    semanticIndexEnabled: false,
    mcpEnabled: false,
    skillEnabled: false,
    autoIndexAfterBackup: false,
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
      backend.exportHistory({ query: { q: 'sqlite' }, format: 'jsonl' }),
    ).resolves.toMatchObject({ format: 'jsonl', count: 1 })
    await expect(backend.previewRemoteBackup()).resolves.toMatchObject({
      bundlePath: expect.stringContaining('pathkeep-remote.zip'),
    })
    await expect(backend.runRemoteBackup()).resolves.toMatchObject({
      uploaded: false,
    })
    await expect(
      backend.inspectTakeout({ sourcePath: '/tmp/takeout', dryRun: true }),
    ).resolves.toMatchObject({
      dryRun: true,
      notes: ['Tauri is not available in browser preview mode.'],
    })
    await expect(
      backend.importTakeout({ sourcePath: '/tmp/takeout', dryRun: false }),
    ).resolves.toMatchObject({
      dryRun: true,
      notes: ['Tauri is not available in browser preview mode.'],
    })
    await expect(backend.previewImportBatch(7)).resolves.toMatchObject({
      batch: expect.objectContaining({ id: 1, status: 'imported' }),
    })
    await expect(backend.revertImportBatch(7)).resolves.toMatchObject({
      batch: expect.objectContaining({ id: 1, status: 'reverted' }),
    })
    await expect(backend.restoreImportBatch(7)).resolves.toMatchObject({
      batch: expect.objectContaining({ id: 1, status: 'imported' }),
    })
    await expect(backend.previewSchedule()).resolves.toMatchObject({
      platform: 'macos',
      applySupported: false,
    })
    await expect(backend.applySchedule(schedulePlan)).resolves.toMatchObject({
      applied: false,
    })
    await expect(backend.doctor()).resolves.toMatchObject({
      checks: [],
    })
    await expect(backend.repairHealth()).resolves.toMatchObject({
      runId: 1,
      repairedImportAudits: 0,
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
    ).resolves.toMatchObject({
      storedSecret: false,
    })
    await expect(backend.clearS3Credentials()).resolves.toMatchObject({
      storedSecret: false,
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
        limit: 20,
      }),
    ).resolves.toMatchObject({
      providerId: 'mock-embedding',
      indexedItems: 2,
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
      limit: 10,
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
    expect(invoke).toHaveBeenNthCalledWith(3, 'search_ai_history', {
      request: searchRequest,
    })
    expect(invoke).toHaveBeenNthCalledWith(4, 'ask_ai_assistant', {
      request: assistantRequest,
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
