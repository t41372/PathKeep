/**
 * @file use-settings-ai-state.test.tsx
 * @description Hook-level coverage for Settings AI provider and integration-preview state.
 * @module pages/settings
 *
 * ## Responsibilities
 * - Verify AI provider draft mutations, save/reset, API-key storage, and local integration preview loading.
 * - Protect Settings from silently dropping provider selections or saved-secret state after handler refactors.
 * - Exercise the state owner directly instead of depending only on broad route rendering smoke tests.
 *
 * ## Not responsible for
 * - Re-testing individual provider form markup.
 * - Re-testing backend preview command internals.
 *
 * ## Dependencies
 * - Uses the shipped i18n provider, preview snapshot fixture, backend-client spies, and browser clipboard shim.
 *
 * ## Performance notes
 * - Hook-level tests keep Settings workflow coverage cheap and deterministic.
 */

import { act, renderHook, waitFor } from '@testing-library/react'
import type { ReactNode } from 'react'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import { mockSnapshot } from '../../lib/backend-preview-fixtures'
import { I18nProvider } from '../../lib/i18n'
import type {
  AiIntegrationPreview,
  AiProviderConfig,
  AppConfig,
  AppSnapshot,
} from '../../lib/types'
import { useSettingsAiState } from './use-settings-ai-state'

function Wrapper({ children }: { children: ReactNode }) {
  return <I18nProvider>{children}</I18nProvider>
}

describe('useSettingsAiState', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  test('loads integration preview and auto-saves every structural change', async () => {
    vi.spyOn(Date, 'now').mockReturnValue(12345)
    const snapshot = snapshotFixture()
    const saveConfig = vi.fn((config: AppConfig) =>
      Promise.resolve({
        ...snapshot,
        config,
      }),
    )
    const refreshAppData = vi.fn().mockResolvedValue(undefined)
    vi.spyOn(backend, 'previewAiIntegrations').mockResolvedValue(
      integrationPreviewFixture(),
    )
    const openPath = vi
      .spyOn(backend, 'openPathInFileManager')
      .mockResolvedValue('/tmp/pathkeep')

    const { result } = renderHook(
      () =>
        useSettingsAiState({
          refreshAppData,
          saveConfig,
          snapshot,
        }),
      { wrapper: Wrapper },
    )

    await waitFor(() =>
      expect(result.current.ai.integrationPreview?.generatedFiles).toHaveLength(
        1,
      ),
    )
    expect(result.current.ai.noProviders).toBe(false)

    // The master toggle auto-saves immediately and resolves true on a real write.
    await act(async () => {
      expect(await result.current.ai.onToggleAi()).toBe(true)
    })
    expect(result.current.ai.currentSettings?.enabled).toBe(true)
    expect(saveConfig).toHaveBeenLastCalledWith(
      expect.objectContaining({
        ai: expect.objectContaining({ enabled: true }),
      }),
    )

    // Adding a provider auto-persists immediately (so test/save-key work at once).
    // A bare onAddProvider('llm') defaults to the LM Studio preset (headline path).
    await act(async () => {
      expect(await result.current.ai.onAddProvider('llm')).toBe(true)
    })
    expect(saveConfig).toHaveBeenLastCalledWith(
      expect.objectContaining({
        ai: expect.objectContaining({
          llmProviders: expect.arrayContaining([
            expect.objectContaining({
              id: 'lm-studio-llm-12345',
              purpose: 'llm',
              requestFormat: 'lm-studio',
            }),
          ]),
        }),
      }),
    )

    // A provider FIELD edit stays in the local buffer while typing (no save)…
    act(() => {
      result.current.ai.onUpdateProvider('llm', 'llm-1', {
        defaultModel: 'patched-model',
      })
    })
    const savesBeforeCommit = saveConfig.mock.calls.length
    expect(
      result.current.ai.currentSettings?.llmProviders.find(
        (provider) => provider.id === 'llm-1',
      )?.defaultModel,
    ).toBe('patched-model')
    expect(saveConfig).toHaveBeenCalledTimes(savesBeforeCommit)
    // …and is persisted on commit (blur).
    await act(async () => {
      expect(await result.current.ai.onCommitProviders()).toBe(true)
    })
    expect(saveConfig.mock.calls.length).toBe(savesBeforeCommit + 1)
    // Re-committing with no further edit is a no-op (no redundant write).
    await act(async () => {
      expect(await result.current.ai.onCommitProviders()).toBe(false)
    })

    // Removing the active embedding provider auto-saves and clears the selection.
    await act(async () => {
      expect(
        await result.current.ai.onRemoveProvider('embedding', 'embed-1'),
      ).toBe(true)
    })
    expect(result.current.ai.currentSettings?.embeddingProviderId).toBeNull()
    expect(saveConfig).toHaveBeenLastCalledWith(
      expect.objectContaining({
        ai: expect.objectContaining({ embeddingProviders: [] }),
      }),
    )
    expect(result.current.ai.saving).toBe(false)

    act(() => {
      result.current.ai.onOpenPath('/tmp/pathkeep/integrations/mcp.json')
    })
    expect(openPath).toHaveBeenCalledWith('/tmp/pathkeep/integrations/mcp.json')
  })

  test('stores, clears, copies, and reports AI provider secret state', async () => {
    const snapshot = snapshotFixture()
    const refreshAppData = vi.fn().mockResolvedValue(undefined)
    vi.spyOn(backend, 'previewAiIntegrations').mockResolvedValue(
      integrationPreviewFixture(),
    )
    const storeKey = vi
      .spyOn(backend, 'storeAiProviderApiKey')
      .mockResolvedValue(snapshot)
    const clearKey = vi
      .spyOn(backend, 'clearAiProviderApiKey')
      .mockResolvedValue(snapshot)
    const originalClipboard = navigator.clipboard
    const writeText = vi.fn().mockResolvedValue(undefined)
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: {
        writeText,
      },
    })

    try {
      const { result } = renderHook(
        () =>
          useSettingsAiState({
            refreshAppData,
            saveConfig: vi.fn((config: AppConfig) =>
              Promise.resolve({
                ...snapshot,
                config,
              }),
            ),
            snapshot,
          }),
        { wrapper: Wrapper },
      )

      await waitFor(() =>
        expect(result.current.ai.integrationPreview).not.toBeNull(),
      )

      await act(async () => {
        await result.current.ai.onSaveAiApiKey('llm-1')
      })
      expect(storeKey).not.toHaveBeenCalled()

      act(() => {
        result.current.ai.onApiKeyChange('llm-1', '  secret-key  ')
      })
      await act(async () => {
        await result.current.ai.onSaveAiApiKey('llm-1')
      })
      expect(storeKey).toHaveBeenCalledWith({
        providerId: 'llm-1',
        apiKey: 'secret-key',
      })
      expect(result.current.ai.aiApiKeys['llm-1']).toBe('')
      expect(
        result.current.ai.currentSettings?.llmProviders.find(
          (provider) => provider.id === 'llm-1',
        )?.apiKeySaved,
      ).toBe(true)
      expect(refreshAppData).toHaveBeenCalledTimes(1)

      await act(async () => {
        await result.current.ai.onClearAiApiKey('llm-1')
      })
      expect(clearKey).toHaveBeenCalledWith('llm-1')
      expect(
        result.current.ai.currentSettings?.llmProviders.find(
          (provider) => provider.id === 'llm-1',
        )?.apiKeySaved,
      ).toBe(false)

      await act(async () => {
        await result.current.ai.onCopyIntegrationValue(
          'mcp',
          '{"mcpServers":{}}',
        )
      })
      expect(writeText).toHaveBeenCalledWith('{"mcpServers":{}}')
      expect(result.current.ai.copyFeedback).toEqual({
        key: 'mcp',
        tone: 'success',
      })
    } finally {
      Object.defineProperty(navigator, 'clipboard', {
        configurable: true,
        value: originalClipboard,
      })
    }
  })

  test('resets integration preview when disabled and reports preview load failures', async () => {
    const snapshot = snapshotFixture()
    const preview = vi
      .spyOn(backend, 'previewAiIntegrations')
      .mockRejectedValue(new Error('mcp unavailable'))

    const { result, rerender } = renderHook(
      ({ enableIntegrationPreview }: { enableIntegrationPreview: boolean }) =>
        useSettingsAiState({
          enableIntegrationPreview,
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          saveConfig: vi.fn((config: AppConfig) =>
            Promise.resolve({
              ...snapshot,
              config,
            }),
          ),
          snapshot,
        }),
      {
        initialProps: { enableIntegrationPreview: true },
        wrapper: Wrapper,
      },
    )

    await waitFor(() =>
      expect(result.current.ai.integrationError).toBe('mcp unavailable'),
    )
    expect(result.current.ai.integrationPreview).toBeNull()

    preview.mockClear()
    rerender({ enableIntegrationPreview: false })
    await waitFor(() => expect(result.current.ai.integrationError).toBeNull())
    expect(result.current.ai.integrationPreview).toBeNull()
    expect(preview).not.toHaveBeenCalled()
  })

  test('keeps preview completion quiet after cleanup and falls back for non-error failures', async () => {
    let resolvePreview: (preview: AiIntegrationPreview) => void = () => {}
    let rejectPreview: (error: unknown) => void = () => {}
    vi.spyOn(backend, 'previewAiIntegrations').mockReturnValueOnce(
      new Promise<AiIntegrationPreview>((resolve) => {
        resolvePreview = resolve
      }),
    )

    const snapshot = snapshotFixture()
    const first = renderHook(
      () =>
        useSettingsAiState({
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          saveConfig: vi.fn((config: AppConfig) =>
            Promise.resolve({
              ...snapshot,
              config,
            }),
          ),
          snapshot,
        }),
      { wrapper: Wrapper },
    )
    first.unmount()
    await act(async () => {
      resolvePreview(integrationPreviewFixture())
      await Promise.resolve()
    })

    vi.spyOn(backend, 'previewAiIntegrations').mockReturnValueOnce(
      new Promise<AiIntegrationPreview>((_, reject) => {
        rejectPreview = reject
      }),
    )
    const second = renderHook(
      () =>
        useSettingsAiState({
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          saveConfig: vi.fn((config: AppConfig) =>
            Promise.resolve({
              ...snapshot,
              config,
            }),
          ),
          snapshot,
        }),
      { wrapper: Wrapper },
    )
    second.unmount()
    await act(async () => {
      rejectPreview('late preview failed')
      await Promise.resolve()
    })

    vi.spyOn(backend, 'previewAiIntegrations').mockRejectedValueOnce(
      'plain preview failure',
    )
    const third = renderHook(
      () =>
        useSettingsAiState({
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          saveConfig: vi.fn((config: AppConfig) =>
            Promise.resolve({
              ...snapshot,
              config,
            }),
          ),
          snapshot,
        }),
      { wrapper: Wrapper },
    )
    await waitFor(() =>
      expect(third.result.current.ai.integrationError).toBe(
        'plain preview failure',
      ),
    )
  })

  test('auto-saves a toggle and an added provider from the saved snapshot', async () => {
    vi.spyOn(backend, 'previewAiIntegrations').mockResolvedValue(
      integrationPreviewFixture(),
    )
    const snapshot = snapshotFixture()
    const saveConfig = vi.fn((config: AppConfig) =>
      Promise.resolve({
        ...snapshot,
        config,
      }),
    )

    const { result } = renderHook(
      () =>
        useSettingsAiState({
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          saveConfig,
          snapshot,
        }),
      { wrapper: Wrapper },
    )

    await act(async () => {
      await result.current.ai.onToggleAi()
    })
    expect(result.current.ai.currentSettings?.enabled).toBe(true)

    await act(async () => {
      await result.current.ai.onAddProvider('embedding')
    })
    expect(result.current.ai.currentSettings?.embeddingProviders).toHaveLength(
      2,
    )
    expect(saveConfig).toHaveBeenCalledTimes(2)
  })

  test('keeps AI handlers no-op safe before a snapshot is available', async () => {
    const refreshAppData = vi.fn().mockResolvedValue(undefined)
    const saveConfig = vi.fn((config: AppConfig) =>
      Promise.resolve({
        ...snapshotFixture(),
        config,
      }),
    )
    const preview = vi
      .spyOn(backend, 'previewAiIntegrations')
      .mockResolvedValue(integrationPreviewFixture())

    const { result, rerender } = renderHook(
      ({ snap }: { snap: AppSnapshot | null }) =>
        useSettingsAiState({
          refreshAppData,
          saveConfig,
          snapshot: snap,
        }),
      { initialProps: { snap: null as AppSnapshot | null }, wrapper: Wrapper },
    )

    expect(result.current.ai.currentSettings).toBeNull()
    expect(result.current.ai.indexMeta).toBeNull()
    expect(result.current.ai.noProviders).toBe(true)

    // Every auto-save handler no-ops (returns false, never calls saveConfig)
    // without a snapshot, including the blur-commit and the tuning knobs.
    // onUpdateProvider also no-ops (no draft buffer to mutate yet).
    await act(async () => {
      result.current.ai.onUpdateProvider('llm', 'llm-1', { defaultModel: 'x' })
      expect(await result.current.ai.onToggleAi()).toBe(false)
      expect(await result.current.ai.onCommitProviders()).toBe(false)
      expect(
        await result.current.ai.onSearchTuningChange('lexicalWeight', 2),
      ).toBe(false)
      expect(await result.current.ai.onResetSearchTuning()).toBe(false)
      expect(await result.current.ai.onAddProvider('llm')).toBe(false)
    })

    expect(saveConfig).not.toHaveBeenCalled()
    expect(refreshAppData).not.toHaveBeenCalled()
    expect(preview).not.toHaveBeenCalled()

    // When a snapshot first arrives the hook seeds the draft from it (so the
    // section can render and edits can begin auto-saving).
    rerender({ snap: snapshotFixture() })
    await waitFor(() =>
      expect(result.current.ai.currentSettings).not.toBeNull(),
    )
    expect(result.current.ai.currentSettings?.enabled).toBe(false)
  })

  test('auto-saves each sub-flag without cascading from the master', async () => {
    vi.spyOn(backend, 'previewAiIntegrations').mockResolvedValue(
      integrationPreviewFixture(),
    )
    const snapshot = snapshotFixture()
    const { result } = renderHook(
      () =>
        useSettingsAiState({
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          saveConfig: vi.fn((config: AppConfig) =>
            Promise.resolve({ ...snapshot, config }),
          ),
          snapshot,
        }),
      { wrapper: Wrapper },
    )

    // All four default OFF in the fixture; flipping the master does NOT cascade.
    expect(result.current.ai.currentSettings?.assistantEnabled).toBe(false)
    expect(result.current.ai.currentSettings?.semanticIndexEnabled).toBe(false)
    expect(result.current.ai.currentSettings?.mcpEnabled).toBe(false)
    expect(result.current.ai.currentSettings?.skillEnabled).toBe(false)

    await act(async () => {
      await result.current.ai.onToggleAi()
    })
    expect(result.current.ai.currentSettings?.enabled).toBe(true)
    expect(result.current.ai.currentSettings?.assistantEnabled).toBe(false)
    expect(result.current.ai.currentSettings?.semanticIndexEnabled).toBe(false)
    // The outward MCP surface stays OFF until explicitly opted into — turning
    // AI on never exposes the archive to external tools.
    expect(result.current.ai.currentSettings?.mcpEnabled).toBe(false)
    // The usage guide is its own consent and stays OFF too.
    expect(result.current.ai.currentSettings?.skillEnabled).toBe(false)
    // The GPU heavy-tier opt-in (W-AI-9-D) is its own consent and stays OFF too.
    expect(result.current.ai.currentSettings?.gpuEnabled ?? false).toBe(false)

    await act(async () => {
      await result.current.ai.onToggleAssistant()
      await result.current.ai.onToggleSemanticIndex()
      await result.current.ai.onToggleMcp()
      await result.current.ai.onToggleSkill()
      await result.current.ai.onToggleGpu()
    })
    expect(result.current.ai.currentSettings?.assistantEnabled).toBe(true)
    expect(result.current.ai.currentSettings?.semanticIndexEnabled).toBe(true)
    expect(result.current.ai.currentSettings?.mcpEnabled).toBe(true)
    expect(result.current.ai.currentSettings?.skillEnabled).toBe(true)
    expect(result.current.ai.currentSettings?.gpuEnabled).toBe(true)

    // Toggling the GPU opt-in back off is independent of the other consents.
    await act(async () => {
      await result.current.ai.onToggleGpu()
    })
    expect(result.current.ai.currentSettings?.gpuEnabled).toBe(false)

    // Toggling back off works independently too — each consent is its own; the
    // usage guide can be turned off without disturbing the MCP server toggle.
    await act(async () => {
      await result.current.ai.onToggleAssistant()
      await result.current.ai.onToggleMcp()
      await result.current.ai.onToggleSkill()
    })
    expect(result.current.ai.currentSettings?.assistantEnabled).toBe(false)
    expect(result.current.ai.currentSettings?.semanticIndexEnabled).toBe(true)
    expect(result.current.ai.currentSettings?.mcpEnabled).toBe(false)
    expect(result.current.ai.currentSettings?.skillEnabled).toBe(false)
  })

  test('clamps, resets, and auto-saves the search-tuning knobs on each change', async () => {
    vi.spyOn(backend, 'previewAiIntegrations').mockResolvedValue(
      integrationPreviewFixture(),
    )
    const snapshot = snapshotFixture()
    const saveConfig = vi.fn((config: AppConfig) =>
      Promise.resolve({ ...snapshot, config }),
    )
    const { result } = renderHook(
      () =>
        useSettingsAiState({
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          saveConfig,
          snapshot,
        }),
      { wrapper: Wrapper },
    )

    // Each knob edit lands clamped (the handler sanitizes the raw slider/input
    // value) AND auto-saves immediately.
    await act(async () => {
      expect(
        await result.current.ai.onSearchTuningChange('lexicalWeight', 2.5),
      ).toBe(true)
    })
    expect(result.current.ai.currentSettings?.lexicalWeight).toBe(2.5)
    await act(async () => {
      await result.current.ai.onSearchTuningChange('starredBoost', 9)
      await result.current.ai.onSearchTuningChange('hybridRrfK', 80.7)
    })
    // Clamped to the [0, 0.5] cap and floored to an integer respectively.
    expect(result.current.ai.currentSettings?.starredBoost).toBe(0.5)
    expect(result.current.ai.currentSettings?.hybridRrfK).toBe(80)
    expect(saveConfig).toHaveBeenLastCalledWith(
      expect.objectContaining({
        ai: expect.objectContaining({ starredBoost: 0.5, hybridRrfK: 80 }),
      }),
    )

    // An emptied number field arrives as NaN and resets that knob to its default.
    await act(async () => {
      await result.current.ai.onSearchTuningChange('lexicalWeight', Number.NaN)
    })
    expect(result.current.ai.currentSettings?.lexicalWeight).toBe(1)

    // Reset auto-saves all four knobs back to their accepted defaults.
    await act(async () => {
      await result.current.ai.onSearchTuningChange('semanticWeight', 0)
    })
    expect(result.current.ai.currentSettings?.semanticWeight).toBe(0)
    await act(async () => {
      expect(await result.current.ai.onResetSearchTuning()).toBe(true)
    })
    expect(result.current.ai.currentSettings?.hybridRrfK).toBe(60)
    expect(result.current.ai.currentSettings?.lexicalWeight).toBe(1)
    expect(result.current.ai.currentSettings?.semanticWeight).toBe(1)
    expect(result.current.ai.currentSettings?.starredBoost).toBe(0.15)
  })

  test('seeds an added provider from the chosen preset format', async () => {
    vi.spyOn(Date, 'now').mockReturnValue(99)
    vi.spyOn(backend, 'previewAiIntegrations').mockResolvedValue(
      integrationPreviewFixture(),
    )
    const snapshot = snapshotFixture()
    const { result } = renderHook(
      () =>
        useSettingsAiState({
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          saveConfig: vi.fn((config: AppConfig) =>
            Promise.resolve({ ...snapshot, config }),
          ),
          snapshot,
        }),
      { wrapper: Wrapper },
    )

    await act(async () => {
      await result.current.ai.onAddProvider('embedding', 'openai')
    })
    expect(result.current.ai.currentSettings?.embeddingProviders).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'openai-embedding-99',
          requestFormat: 'openai',
          purpose: 'embedding',
        }),
      ]),
    )
  })

  test('probes a provider and records the reachable report by id', async () => {
    vi.spyOn(backend, 'previewAiIntegrations').mockResolvedValue(
      integrationPreviewFixture(),
    )
    const probe = vi
      .spyOn(backend, 'testAiProviderConnection')
      .mockResolvedValue({
        providerId: 'llm-1',
        purpose: 'llm',
        model: 'local-model',
        ok: true,
        latencyMs: 21,
        capabilities: {
          supportsChat: true,
          supportsEmbeddings: false,
          supportsStreaming: true,
          supportsToolUse: true,
          supportsStructuredOutput: true,
        },
        llmCapabilities: null,
        errorCode: null,
        actionHint: null,
        retryHint: null,
        warnings: [],
        message: 'Reachable',
      })
    const snapshot = snapshotFixture()
    const { result } = renderHook(
      () =>
        useSettingsAiState({
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          saveConfig: vi.fn((config: AppConfig) =>
            Promise.resolve({ ...snapshot, config }),
          ),
          snapshot,
        }),
      { wrapper: Wrapper },
    )

    await act(async () => {
      await result.current.ai.onProviderProbe('llm', 'llm-1')
    })
    expect(probe).toHaveBeenCalledWith({ providerId: 'llm-1', purpose: 'llm' })
    expect(result.current.ai.providerProbes['llm-1'].ok).toBe(true)
    expect(result.current.ai.testingProviderId).toBeNull()
  })

  test('degrades a failed probe into a not-ok report instead of throwing', async () => {
    vi.spyOn(backend, 'previewAiIntegrations').mockResolvedValue(
      integrationPreviewFixture(),
    )
    vi.spyOn(backend, 'testAiProviderConnection').mockRejectedValue(
      new Error('Connection refused'),
    )
    const snapshot = snapshotFixture()
    const { result } = renderHook(
      () =>
        useSettingsAiState({
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          saveConfig: vi.fn((config: AppConfig) =>
            Promise.resolve({ ...snapshot, config }),
          ),
          snapshot,
        }),
      { wrapper: Wrapper },
    )

    await act(async () => {
      await result.current.ai.onProviderProbe('embedding', 'embed-1')
    })
    expect(result.current.ai.providerProbes['embed-1'].ok).toBe(false)
    expect(result.current.ai.providerProbes['embed-1'].message).toBe(
      'Connection refused',
    )
    expect(result.current.ai.testingProviderId).toBeNull()
  })

  test('selecting the already-active provider is a no-op auto-save (returns false)', async () => {
    vi.spyOn(backend, 'previewAiIntegrations').mockResolvedValue(
      integrationPreviewFixture(),
    )
    const snapshot = snapshotFixture()
    const saveConfig = vi.fn((config: AppConfig) =>
      Promise.resolve({ ...snapshot, config }),
    )
    const { result } = renderHook(
      () =>
        useSettingsAiState({
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          saveConfig,
          snapshot,
        }),
      { wrapper: Wrapper },
    )

    // 'llm-1' is already the active LLM provider in the fixture, so re-selecting
    // it changes nothing — no write, no chip.
    await act(async () => {
      expect(await result.current.ai.onSelectProvider('llm', 'llm-1')).toBe(
        false,
      )
    })
    expect(saveConfig).not.toHaveBeenCalled()

    // Selecting a genuinely different provider does auto-save.
    await act(async () => {
      await result.current.ai.onAddProvider('llm', 'openai')
    })
    saveConfig.mockClear()
    const added = result.current.ai.currentSettings?.llmProviders.find(
      (provider) => provider.id !== 'llm-1',
    )
    await act(async () => {
      expect(
        await result.current.ai.onSelectProvider('llm', added?.id ?? ''),
      ).toBe(true)
    })
    expect(saveConfig).toHaveBeenCalledTimes(1)
  })

  test('adopts a genuine external snapshot change but keeps local uncommitted edits', async () => {
    vi.spyOn(backend, 'previewAiIntegrations').mockResolvedValue(
      integrationPreviewFixture(),
    )
    const snapshot = snapshotFixture()
    const { result, rerender } = renderHook(
      ({ snap }: { snap: AppSnapshot }) =>
        useSettingsAiState({
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          saveConfig: vi.fn((config: AppConfig) =>
            Promise.resolve({ ...snap, config }),
          ),
          snapshot: snap,
        }),
      { initialProps: { snap: snapshot }, wrapper: Wrapper },
    )

    expect(result.current.ai.currentSettings?.enabled).toBe(false)

    // An external snapshot update (e.g. another surface enabled AI) is adopted
    // into the draft because there are no local uncommitted edits.
    const externallyEnabled: AppSnapshot = {
      ...snapshot,
      config: {
        ...snapshot.config,
        ai: { ...snapshot.config.ai, enabled: true },
      },
    }
    rerender({ snap: externallyEnabled })
    await waitFor(() =>
      expect(result.current.ai.currentSettings?.enabled).toBe(true),
    )

    // With a LOCAL uncommitted provider edit pending, a further external change
    // must not clobber the in-progress edit.
    act(() => {
      result.current.ai.onUpdateProvider('llm', 'llm-1', {
        defaultModel: 'in-progress',
      })
    })
    const reselected: AppSnapshot = {
      ...externallyEnabled,
      config: {
        ...externallyEnabled.config,
        ai: { ...externallyEnabled.config.ai, assistantEnabled: true },
      },
    }
    rerender({ snap: reselected })
    expect(
      result.current.ai.currentSettings?.llmProviders.find(
        (provider) => provider.id === 'llm-1',
      )?.defaultModel,
    ).toBe('in-progress')
  })

  test('keeps update + commit no-op-safe after the snapshot drops to null', async () => {
    vi.spyOn(backend, 'previewAiIntegrations').mockResolvedValue(
      integrationPreviewFixture(),
    )
    const snapshot = snapshotFixture()
    const saveConfig = vi.fn((config: AppConfig) =>
      Promise.resolve({ ...snapshot, config }),
    )
    const { result, rerender } = renderHook(
      ({ snap }: { snap: AppSnapshot | null }) =>
        useSettingsAiState({
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          saveConfig,
          snapshot: snap,
        }),
      {
        initialProps: { snap: snapshot as AppSnapshot | null },
        wrapper: Wrapper,
      },
    )

    // Drop the snapshot while a draft already exists in the ref.
    rerender({ snap: null })
    saveConfig.mockClear()

    // updateAiDraft still mutates the stale buffer, but persistAi refuses without
    // a snapshot (it needs config to write), so commit/toggle no-op to false.
    act(() => {
      result.current.ai.onUpdateProvider('llm', 'llm-1', {
        defaultModel: 'orphaned',
      })
    })
    await act(async () => {
      expect(await result.current.ai.onCommitProviders()).toBe(false)
      expect(await result.current.ai.onToggleAi()).toBe(false)
    })
    expect(saveConfig).not.toHaveBeenCalled()
  })
})

function snapshotFixture(): AppSnapshot {
  const snapshot = structuredClone(mockSnapshot)
  const llmProvider = providerFixture('llm-1', 'llm')
  const embeddingProvider = providerFixture('embed-1', 'embedding')
  return {
    ...snapshot,
    config: {
      ...snapshot.config,
      initialized: true,
      ai: {
        ...snapshot.config.ai,
        enabled: false,
        llmProviderId: 'llm-1',
        embeddingProviderId: 'embed-1',
        llmProviders: [llmProvider],
        embeddingProviders: [embeddingProvider],
      },
    },
  }
}

function providerFixture(
  id: string,
  purpose: AiProviderConfig['purpose'],
): AiProviderConfig {
  return {
    id,
    name: purpose === 'llm' ? 'Local LLM' : 'Local Embeddings',
    purpose,
    requestFormat: 'openai',
    enabled: true,
    baseUrl: 'http://localhost:11434/v1',
    apiKeySaved: false,
    defaultModel: purpose === 'llm' ? 'gpt-test' : 'text-embedding-test',
    modelCatalog: [],
    temperature: purpose === 'llm' ? 0.7 : null,
    maxTokens: purpose === 'llm' ? 1200 : null,
    dimensions: purpose === 'embedding' ? 1536 : null,
    notes: null,
  }
}

function integrationPreviewFixture(): AiIntegrationPreview {
  return {
    mcpCommand: '/Applications/PathKeep.app --worker mcp-server',
    consentSummary:
      'External AI integrations stay local-first and explicit. PathKeep only exposes localhost MCP tools after you turn on AI + MCP in Settings, and the current app session must stay unlocked.',
    manualSteps: ['Enable MCP or Skill integration in Settings first.'],
    capabilityNotes: [
      'Semantic retrieval can use the configured embedding provider when the semantic index is built.',
    ],
    scopeBoundary: [
      'Only visible archive facts are returned to external tools.',
    ],
    auditTrace: ['Each MCP search writes a dedicated run-ledger entry.'],
    generatedFiles: [
      {
        relativePath: 'integrations/pathkeep-mcp.json',
        absolutePath: '/tmp/pathkeep/integrations/pathkeep-mcp.json',
        purpose: 'PathKeep MCP client snippet',
        contents: '{"mcpServers":{}}',
      },
    ],
    warnings: [],
  }
}
