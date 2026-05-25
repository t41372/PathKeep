/**
 * This test file protects the front-end helper and contract logic in Backend Client.
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

import { beforeEach, describe, expect, test, vi } from 'vitest'

const {
  invokeCommandMock,
  hasDesktopCommandTransportMock,
  backendHarnessMock,
} = vi.hoisted(() => ({
  invokeCommandMock: vi.fn(),
  hasDesktopCommandTransportMock: vi.fn(() => false),
  backendHarnessMock: {
    call: vi.fn(),
  },
}))

vi.mock('./ipc/bridge', () => ({
  invokeCommand: invokeCommandMock,
}))

vi.mock('./runtime', () => ({
  hasDesktopCommandTransport: hasDesktopCommandTransportMock,
}))

vi.mock('./backend', () => ({
  backendTestHarness: backendHarnessMock,
}))

describe('backend client', () => {
  beforeEach(() => {
    invokeCommandMock.mockReset()
    backendHarnessMock.call.mockReset()
    hasDesktopCommandTransportMock.mockReturnValue(false)
    ;(
      window as Window & {
        __PATHKEEP_DESKTOP_COMMAND_METRICS__?: Array<{
          command: string
          durationMs: number
          requestBytes: number
          responseBytes: number
          recordedAt: string
        }>
      }
    ).__PATHKEEP_DESKTOP_COMMAND_METRICS__ = []
  })

  test('uses the live desktop command transport when available', async () => {
    hasDesktopCommandTransportMock.mockReturnValue(true)
    invokeCommandMock.mockResolvedValueOnce({ version: '0.1.0' })

    const { backend } = await import('./backend-client')
    const result = await backend.getAppBuildInfo()

    expect(invokeCommandMock).toHaveBeenCalledWith('app_build_info', undefined)
    expect(result).toEqual({ version: '0.1.0' })
    expect(backendHarnessMock.call).not.toHaveBeenCalled()
  })

  test('falls back to the browser preview harness when no desktop transport exists', async () => {
    backendHarnessMock.call.mockResolvedValueOnce({ version: 'preview' })

    const { backend } = await import('./backend-client')
    const result = await backend.getAppBuildInfo()

    expect(backendHarnessMock.call).toHaveBeenCalledWith(
      'app_build_info',
      undefined,
    )
    expect(result).toEqual({ version: 'preview' })
    expect(invokeCommandMock).not.toHaveBeenCalled()
  })

  test('records desktop metrics without traversing the entire response payload', async () => {
    hasDesktopCommandTransportMock.mockReturnValue(true)
    const rows = Array.from({ length: 8 }, (_, index) => ({ index }))
    rows.length = 20
    Object.defineProperty(rows, 15, {
      enumerable: true,
      get() {
        throw new Error('desktop metrics should not read deep array entries')
      },
    })
    invokeCommandMock.mockResolvedValueOnce({
      version: '0.1.0',
      rows,
    })

    const { backend } = await import('./backend-client')
    const result = await backend.getAppBuildInfo()

    expect(result).toEqual({
      version: '0.1.0',
      rows,
    })
    const metrics = (
      window as Window & {
        __PATHKEEP_DESKTOP_COMMAND_METRICS__?: Array<{
          responseBytes: number
        }>
      }
    ).__PATHKEEP_DESKTOP_COMMAND_METRICS__
    expect(metrics).toHaveLength(1)
    expect(metrics?.[0]?.responseBytes).toBeGreaterThan(0)
  })

  test('bounds desktop metrics across unusual payloads and long sessions', async () => {
    hasDesktopCommandTransportMock.mockReturnValue(true)
    const cyclic: Record<string, unknown> = {
      id: 1n,
      symbol: Symbol('opaque'),
      missing: undefined,
    }
    cyclic.self = cyclic
    Object.defineProperty(cyclic, 'volatile', {
      enumerable: true,
      get() {
        throw new Error('metric summarization should degrade')
      },
    })
    invokeCommandMock.mockResolvedValue(cyclic)

    const { call } = await import('./backend-client/shared')
    for (let index = 0; index < 205; index += 1) {
      await call('diagnostic_command', {
        index,
        exactBytesFallback: 1n,
      })
    }

    const metrics = (
      window as Window & {
        __PATHKEEP_DESKTOP_COMMAND_METRICS__?: Array<{
          command: string
          requestBytes: number
          responseBytes: number
        }>
      }
    ).__PATHKEEP_DESKTOP_COMMAND_METRICS__
    expect(metrics).toHaveLength(200)
    expect(metrics?.[0]).toMatchObject({
      command: 'diagnostic_command',
      requestBytes: 0,
      responseBytes: 0,
    })
  })

  test('summarizes bigint, cycles, and symbols for desktop metrics', async () => {
    hasDesktopCommandTransportMock.mockReturnValue(true)
    const cyclic: Record<string, unknown> = {
      id: 1n,
      opaque: Symbol('desktop'),
    }
    cyclic.self = cyclic
    invokeCommandMock.mockResolvedValueOnce(cyclic)

    const { call } = await import('./backend-client/shared')
    await expect(call('diagnostic_command')).resolves.toBe(cyclic)

    const metrics = (
      window as Window & {
        __PATHKEEP_DESKTOP_COMMAND_METRICS__?: Array<{
          responseBytes: number
        }>
      }
    ).__PATHKEEP_DESKTOP_COMMAND_METRICS__
    expect(metrics?.[0]?.responseBytes).toBeGreaterThan(0)
  })

  test('skips metric storage when no window object is available', async () => {
    backendHarnessMock.call.mockResolvedValueOnce({ ok: true })
    vi.stubGlobal('window', undefined)

    const { call } = await import('./backend-client/shared')
    await expect(call('preview_command')).resolves.toEqual({ ok: true })

    vi.unstubAllGlobals()
  })

  test('keeps the compatibility facade wired to every desktop command', async () => {
    backendHarnessMock.call.mockResolvedValue({ ok: true })

    const { backend } = await import('./backend-client')
    const config = { initialized: false } as never
    const schedulePlan = { label: 'com.yi-ting.pathkeep.backup' } as never
    const request = { id: 'request' } as never

    const calls: Array<{
      run: () => Promise<unknown>
      command: string
      args?: Record<string, unknown>
    }> = [
      { run: () => backend.getAppBuildInfo(), command: 'app_build_info' },
      { run: () => backend.loadAppLockStatus(), command: 'app_lock_status' },
      { run: () => backend.getAppSnapshot(), command: 'app_snapshot' },
      {
        run: () => backend.saveConfig(config),
        command: 'save_config',
        args: { config },
      },
      {
        run: () => backend.initializeArchive(config, 'secret'),
        command: 'initialize_archive',
        args: { config, databaseKey: 'secret' },
      },
      {
        run: () => backend.rekeyArchive(request),
        command: 'rekey_archive',
        args: { request },
      },
      {
        run: () => backend.previewRekeyArchive(request),
        command: 'preview_rekey_archive',
        args: { request },
      },
      {
        run: () => backend.previewSnapshotRestore(request),
        command: 'preview_snapshot_restore',
        args: { request },
      },
      {
        run: () => backend.runSnapshotRestore(request),
        command: 'run_snapshot_restore',
        args: { request },
      },
      {
        run: () => backend.previewRetentionPrune(),
        command: 'preview_retention_prune',
      },
      {
        run: () => backend.runRetentionPrune(request),
        command: 'run_retention_prune',
        args: { request },
      },
      {
        run: () => backend.setSessionDatabaseKey('secret'),
        command: 'set_session_database_key',
        args: { databaseKey: 'secret' },
      },
      {
        run: () => backend.clearSessionDatabaseKey(),
        command: 'clear_session_database_key',
      },
      {
        run: () => backend.setAppLockPasscode(request),
        command: 'set_app_lock_passcode',
        args: { request },
      },
      {
        run: () => backend.clearAppLockPasscode(),
        command: 'clear_app_lock_passcode',
      },
      {
        run: () => backend.lockAppSession('idle'),
        command: 'lock_app_session',
        args: { reason: 'idle' },
      },
      {
        run: () => backend.unlockAppSession(request),
        command: 'unlock_app_session',
        args: { request },
      },
      {
        run: () => backend.runBackupNow(true),
        command: 'run_backup_now',
        args: { dueOnly: true },
      },
      {
        run: () => backend.queryHistory(request),
        command: 'query_history',
        args: { query: request },
      },
      {
        run: () => backend.loadHistoryFavicons([request]),
        command: 'load_history_favicons',
        args: { entries: [request] },
      },
      {
        run: () =>
          backend.loadHistoryOgImages([{ url: 'https://example.test' }]),
        command: 'load_history_og_images',
        args: { entries: [{ url: 'https://example.test' }] },
      },
      {
        run: () => backend.markOgImagesShown(['https://example.test']),
        command: 'mark_og_images_shown',
        args: { urls: ['https://example.test'] },
      },
      {
        run: () => backend.triggerOgImageRefetch(['https://example.test']),
        command: 'trigger_og_image_refetch',
        args: { urls: ['https://example.test'] },
      },
      {
        run: () => backend.prefetchOgImages(500),
        command: 'prefetch_og_images',
        args: { budget: 500 },
      },
      {
        run: () => backend.getOgImageStorageStats(),
        command: 'get_og_image_storage_stats',
        args: {},
      },
      {
        run: () => backend.clearOgImageCache(),
        command: 'clear_og_image_cache',
        args: {},
      },
      {
        run: () => backend.runOgImageCleanup(),
        command: 'run_og_image_cleanup',
        args: {},
      },
      {
        run: () => backend.getUrlAnnotation('https://example.test'),
        command: 'get_url_annotation',
        args: { url: 'https://example.test' },
      },
      {
        run: () =>
          backend.setUrlNotes({ url: 'https://example.test', notes: 'note' }),
        command: 'set_url_notes',
        args: { request: { url: 'https://example.test', notes: 'note' } },
      },
      {
        run: () =>
          backend.replaceUrlTags({
            url: 'https://example.test',
            tags: ['t1', 't2'],
          }),
        command: 'replace_url_tags',
        args: {
          request: { url: 'https://example.test', tags: ['t1', 't2'] },
        },
      },
      {
        run: () => backend.listUrlAnnotations(),
        command: 'list_url_annotations',
        args: { limit: null },
      },
      {
        run: () => backend.listUrlAnnotations(20),
        command: 'list_url_annotations',
        args: { limit: 20 },
      },
      {
        run: () => backend.searchUrlAnnotations('keyword'),
        command: 'search_url_annotations',
        args: { query: 'keyword', limit: null },
      },
      {
        run: () => backend.searchUrlAnnotations('keyword', 10),
        command: 'search_url_annotations',
        args: { query: 'keyword', limit: 10 },
      },
      {
        run: () => backend.loadDashboardSnapshot(),
        command: 'load_dashboard_snapshot',
      },
      {
        run: () => backend.loadAuditRunDetail(42),
        command: 'load_audit_run_detail',
        args: { runId: 42 },
      },
      {
        run: () => backend.exportHistory(request),
        command: 'export_history',
        args: { request },
      },
      {
        run: () => backend.inspectTakeout(request),
        command: 'inspect_takeout',
        args: { request },
      },
      {
        run: () => backend.importTakeout(request),
        command: 'import_takeout',
        args: { request },
      },
      {
        run: () => backend.inspectBrowserHistory(request),
        command: 'inspect_browser_history',
        args: { request },
      },
      {
        run: () => backend.importBrowserHistory(request),
        command: 'import_browser_history',
        args: { request },
      },
      {
        run: () => backend.previewImportBatch(7),
        command: 'preview_import_batch',
        args: { batchId: 7 },
      },
      {
        run: () => backend.revertImportBatch(7),
        command: 'revert_import_batch',
        args: { batchId: 7 },
      },
      {
        run: () => backend.restoreImportBatch(7),
        command: 'restore_import_batch',
        args: { batchId: 7 },
      },
      {
        run: () => backend.previewSchedule(),
        command: 'preview_schedule',
        args: { platform: undefined },
      },
      {
        run: () => backend.scheduleStatus(),
        command: 'schedule_status',
        args: { platform: undefined },
      },
      {
        run: () => backend.applySchedule(schedulePlan),
        command: 'apply_schedule',
        args: { plan: schedulePlan },
      },
      {
        run: () => backend.removeSchedule(schedulePlan),
        command: 'remove_schedule',
        args: { plan: schedulePlan },
      },
      {
        run: () => backend.repairSchedule(schedulePlan),
        command: 'repair_schedule',
        args: { plan: schedulePlan },
      },
      { run: () => backend.doctor(), command: 'doctor_report' },
      { run: () => backend.repairHealth(), command: 'repair_health' },
      { run: () => backend.keyringStatus(), command: 'keyring_status' },
      { run: () => backend.securityStatus(), command: 'security_status' },
      {
        run: () => backend.keyringGetDatabaseKey(),
        command: 'keyring_get_database_key',
      },
      {
        run: () => backend.keyringStoreDatabaseKey('secret'),
        command: 'keyring_store_database_key',
        args: { value: 'secret' },
      },
      {
        run: () => backend.keyringClearDatabaseKey(),
        command: 'keyring_clear_database_key',
      },
      {
        run: () => backend.storeAiProviderApiKey(request),
        command: 'store_ai_provider_api_key',
        args: { input: request },
      },
      {
        run: () => backend.clearAiProviderApiKey('provider'),
        command: 'clear_ai_provider_api_key',
        args: { providerId: 'provider' },
      },
      {
        run: () => backend.testAiProviderConnection(request),
        command: 'test_ai_provider_connection',
        args: { request },
      },
      {
        run: () => backend.loadAiQueueStatus(),
        command: 'load_ai_queue_status',
      },
      {
        run: () => backend.runAiQueueJobs(3),
        command: 'run_ai_queue_jobs',
        args: { maxJobs: 3 },
      },
      {
        run: () => backend.replayAiJob(5),
        command: 'replay_ai_job',
        args: { jobId: 5 },
      },
      {
        run: () => backend.cancelAiJob(5),
        command: 'cancel_ai_job',
        args: { jobId: 5 },
      },
      {
        run: () => backend.buildAiIndex(request),
        command: 'build_ai_index',
        args: { request },
      },
      {
        run: () => backend.searchAiHistory(request),
        command: 'search_ai_history',
        args: { request },
      },
      {
        run: () => backend.askAiAssistant(request),
        command: 'ask_ai_assistant',
        args: { request },
      },
      {
        run: () => backend.loadAiAssistantJob(5),
        command: 'load_ai_assistant_job',
        args: { jobId: 5 },
      },
      {
        run: () => backend.listSearchEngineRules(),
        command: 'list_search_engine_rules',
      },
      {
        run: () => backend.upsertSearchEngineRule(request),
        command: 'upsert_search_engine_rule',
        args: { input: request },
      },
      {
        run: () => backend.deleteSearchEngineRule('rule'),
        command: 'delete_search_engine_rule',
        args: { ruleId: 'rule' },
      },
      {
        run: () => backend.clearDerivedIntelligence(),
        command: 'clear_derived_intelligence',
      },
      {
        run: () => backend.loadIntelligenceRuntime(),
        command: 'load_intelligence_runtime',
      },
      {
        run: () => backend.retryIntelligenceJob(8),
        command: 'retry_intelligence_job',
        args: { jobId: 8 },
      },
      {
        run: () => backend.cancelIntelligenceJob(8),
        command: 'cancel_intelligence_job',
        args: { jobId: 8 },
      },
      {
        run: () => backend.previewAiIntegrations(),
        command: 'preview_ai_integrations',
      },
      {
        run: () => backend.resetLocalSecretVault(),
        command: 'reset_local_secret_vault',
      },
      {
        run: () => backend.openPathInFileManager('/tmp/archive'),
        command: 'open_path_in_file_manager',
        args: { path: '/tmp/archive' },
      },
      {
        run: () => backend.openExternalUrl('https://example.test'),
        command: 'open_external_url',
        args: { url: 'https://example.test' },
      },
      {
        run: () => backend.checkForAppUpdate(),
        command: 'check_for_app_update',
      },
      {
        run: () => backend.downloadAndInstallAppUpdate('1.2.3'),
        command: 'download_and_install_app_update',
        args: { request: { expectedVersion: '1.2.3' } },
      },
      {
        run: () => backend.downloadAndInstallAppUpdate(),
        command: 'download_and_install_app_update',
        args: { request: { expectedVersion: null } },
      },
      {
        run: () => backend.relaunchAfterUpdate(),
        command: 'relaunch_after_update',
      },
    ]

    for (const expected of calls) {
      backendHarnessMock.call.mockClear()
      await expected.run()
      expect(backendHarnessMock.call).toHaveBeenCalledWith(
        expected.command,
        expected.args,
      )
    }
  })

  test('keeps focused Core Intelligence client methods wired to desktop commands', async () => {
    backendHarnessMock.call.mockResolvedValue({ ok: true })

    const { intelligenceClient } = await import('./backend-client/intelligence')
    const overviewRequest = {
      dateRange: { start: '2026-04-01', end: '2026-04-25' },
      profileId: 'chrome:Default',
    }
    const calls = [
      {
        run: () =>
          intelligenceClient.queueCoreIntelligenceRebuild({
            fullRebuild: true,
            limit: 25,
            profileId: 'chrome:Default',
          }),
        command: 'queue_core_intelligence_rebuild',
        args: {
          request: {
            fullRebuild: true,
            limit: 25,
            profileId: 'chrome:Default',
          },
        },
      },
      {
        run: () => intelligenceClient.getPrimaryOverview(overviewRequest),
        command: 'get_intelligence_primary_overview',
        args: { request: overviewRequest },
      },
      {
        run: () => intelligenceClient.getSecondaryOverview(overviewRequest),
        command: 'get_intelligence_secondary_overview',
        args: { request: overviewRequest },
      },
      {
        run: () =>
          intelligenceClient.getDayInsights({
            date: '2026-04-25',
            profileId: null,
          }),
        command: 'get_day_insights',
        args: { request: { date: '2026-04-25', profileId: null } },
      },
    ]

    for (const expected of calls) {
      backendHarnessMock.call.mockClear()
      await expected.run()
      expect(backendHarnessMock.call).toHaveBeenCalledWith(
        expected.command,
        expected.args,
      )
    }
  })
})
