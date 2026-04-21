/**
 * @file tauri-passthrough.test.ts
 * @description Desktop/Tauri passthrough coverage for the backend facade.
 * @module lib/backend-tests/tauri-passthrough
 *
 * ## Responsibilities
 * - Verify that backend facade methods forward the right command names and payloads when Tauri transport is active.
 * - Keep desktop-shell passthrough assertions separate from browser-preview fixture behavior.
 * - Reuse shared fixtures so command payload expectations stay consistent across split suites.
 *
 * ## Not responsible for
 * - Browser-preview fixture semantics.
 * - App-lock and workflow branch coverage.
 * - Lower-level IPC bridge tests outside the backend facade surface.
 *
 * ## Dependencies
 * - Depends on `../backend` and `./test-helpers`.
 * - Mocks `@tauri-apps/api/core` locally so invoke calls can be asserted directly.
 *
 * ## Performance notes
 * - These tests stay cheap because they never leave the mocked Tauri transport path.
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
import { schedulePlanFixture as schedulePlan } from './test-helpers'

describe('backend facade Tauri passthrough', () => {
  beforeEach(() => {
    isTauri.mockReturnValue(false)
    invoke.mockReset()
    backendTestHarness.reset()
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

  test('passes schedule, security, remote, and import payloads through to Tauri invoke', async () => {
    isTauri.mockReturnValue(true)
    invoke.mockResolvedValue({ ok: true })

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
    await expect(backend.previewAiIntegrations()).resolves.toEqual({ ok: true })
    await expect(
      backend.openPathInFileManager('/tmp/pathkeep'),
    ).resolves.toEqual({
      ok: true,
    })
    await expect(
      backend.openExternalUrl('https://example.com/pathkeep'),
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
    expect(invoke).toHaveBeenNthCalledWith(
      12,
      'preview_ai_integrations',
      undefined,
    )
    expect(invoke).toHaveBeenNthCalledWith(13, 'open_path_in_file_manager', {
      path: '/tmp/pathkeep',
    })
    expect(invoke).toHaveBeenNthCalledWith(14, 'open_external_url', {
      url: 'https://example.com/pathkeep',
    })
  })
})
