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

  test('loads integration preview and persists provider draft changes', async () => {
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
    expect(result.current.ai.persistedProviderIds.has('llm-1')).toBe(true)
    expect(result.current.ai.noProviders).toBe(false)

    act(() => {
      result.current.ai.onToggleAi()
      result.current.ai.onAddProvider('llm')
      result.current.ai.onUpdateProvider('llm', 'llm-1', {
        defaultModel: 'patched-model',
      })
      result.current.ai.onSelectProvider('embedding', 'embed-1')
      result.current.ai.onRemoveProvider('embedding', 'embed-1')
    })

    expect(result.current.ai.configDirty).toBe(true)
    expect(result.current.ai.currentSettings?.enabled).toBe(true)
    // A bare onAddProvider('llm') defaults to the LM Studio preset (the headline
    // local path), so the seeded draft id carries the lm-studio format prefix.
    expect(result.current.ai.currentSettings?.llmProviders).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'llm-1',
          defaultModel: 'patched-model',
        }),
        expect.objectContaining({
          id: 'lm-studio-llm-12345',
          purpose: 'llm',
          requestFormat: 'lm-studio',
        }),
      ]),
    )
    expect(result.current.ai.currentSettings?.embeddingProviderId).toBeNull()

    await act(async () => {
      await result.current.ai.onSaveAiConfig()
    })
    expect(saveConfig).toHaveBeenCalledWith(
      expect.objectContaining({
        ai: expect.objectContaining({
          enabled: true,
          llmProviders: expect.arrayContaining([
            expect.objectContaining({ id: 'lm-studio-llm-12345' }),
          ]),
        }),
      }),
    )
    expect(result.current.ai.saving).toBe(false)

    act(() => {
      result.current.ai.onResetAiConfig()
    })
    expect(result.current.ai.currentSettings?.enabled).toBe(false)

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

  test('can create an AI draft from the saved snapshot before effect sync settles', () => {
    vi.spyOn(backend, 'previewAiIntegrations').mockResolvedValue(
      integrationPreviewFixture(),
    )
    const snapshot = snapshotFixture()

    const { result } = renderHook(
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

    act(() => {
      result.current.ai.onToggleAi()
    })

    expect(result.current.ai.currentSettings?.enabled).toBe(true)

    act(() => {
      result.current.ai.onAddProvider('embedding')
    })

    expect(result.current.ai.currentSettings?.embeddingProviders).toHaveLength(
      2,
    )
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

    const { result } = renderHook(
      () =>
        useSettingsAiState({
          refreshAppData,
          saveConfig,
          snapshot: null,
        }),
      { wrapper: Wrapper },
    )

    expect(result.current.ai.currentSettings).toBeNull()
    expect(result.current.ai.indexMeta).toBeNull()
    expect(result.current.ai.noProviders).toBe(true)
    expect(result.current.ai.persistedProviderIds.size).toBe(0)

    act(() => {
      result.current.ai.onToggleAi()
      result.current.ai.onResetAiConfig()
      result.current.ai.onSearchTuningChange('lexicalWeight', 2)
      result.current.ai.onResetSearchTuning()
    })
    await act(async () => {
      await result.current.ai.onSaveAiConfig()
    })

    expect(saveConfig).not.toHaveBeenCalled()
    expect(refreshAppData).not.toHaveBeenCalled()
    expect(preview).not.toHaveBeenCalled()
  })

  test('toggles the assistant + semantic sub-flags on the draft without cascading from the master', () => {
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

    // Both default OFF in the fixture; flipping the master does NOT cascade.
    expect(result.current.ai.currentSettings?.assistantEnabled).toBe(false)
    expect(result.current.ai.currentSettings?.semanticIndexEnabled).toBe(false)

    act(() => {
      result.current.ai.onToggleAi()
    })
    expect(result.current.ai.currentSettings?.enabled).toBe(true)
    expect(result.current.ai.currentSettings?.assistantEnabled).toBe(false)
    expect(result.current.ai.currentSettings?.semanticIndexEnabled).toBe(false)

    act(() => {
      result.current.ai.onToggleAssistant()
      result.current.ai.onToggleSemanticIndex()
    })
    expect(result.current.ai.currentSettings?.assistantEnabled).toBe(true)
    expect(result.current.ai.currentSettings?.semanticIndexEnabled).toBe(true)

    // Toggling back off works independently too.
    act(() => {
      result.current.ai.onToggleAssistant()
    })
    expect(result.current.ai.currentSettings?.assistantEnabled).toBe(false)
    expect(result.current.ai.currentSettings?.semanticIndexEnabled).toBe(true)
  })

  test('mutates, clamps, resets, and persists the search-tuning knobs through Save', async () => {
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

    // A weight edit lands clamped on the draft (the handler sanitizes the raw
    // slider/input value) without auto-saving.
    act(() => {
      result.current.ai.onSearchTuningChange('lexicalWeight', 2.5)
      result.current.ai.onSearchTuningChange('starredBoost', 9)
      result.current.ai.onSearchTuningChange('hybridRrfK', 80.7)
    })
    expect(result.current.ai.currentSettings?.lexicalWeight).toBe(2.5)
    // Clamped to the [0, 0.5] cap and floored to an integer respectively.
    expect(result.current.ai.currentSettings?.starredBoost).toBe(0.5)
    expect(result.current.ai.currentSettings?.hybridRrfK).toBe(80)
    expect(result.current.ai.configDirty).toBe(true)
    expect(saveConfig).not.toHaveBeenCalled()

    // An emptied number field arrives as NaN and resets that knob to its default.
    act(() => {
      result.current.ai.onSearchTuningChange('lexicalWeight', Number.NaN)
    })
    expect(result.current.ai.currentSettings?.lexicalWeight).toBe(1)

    // Save round-trips the knobs through the shared AI config Save.
    await act(async () => {
      await result.current.ai.onSaveAiConfig()
    })
    expect(saveConfig).toHaveBeenCalledWith(
      expect.objectContaining({
        ai: expect.objectContaining({
          starredBoost: 0.5,
          hybridRrfK: 80,
          lexicalWeight: 1,
        }),
      }),
    )

    // Reset returns all four knobs to their accepted defaults on the draft.
    act(() => {
      result.current.ai.onSearchTuningChange('semanticWeight', 0)
    })
    expect(result.current.ai.currentSettings?.semanticWeight).toBe(0)
    act(() => {
      result.current.ai.onResetSearchTuning()
    })
    expect(result.current.ai.currentSettings?.hybridRrfK).toBe(60)
    expect(result.current.ai.currentSettings?.lexicalWeight).toBe(1)
    expect(result.current.ai.currentSettings?.semanticWeight).toBe(1)
    expect(result.current.ai.currentSettings?.starredBoost).toBe(0.15)
  })

  test('seeds an added provider from the chosen preset format', () => {
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

    act(() => {
      result.current.ai.onAddProvider('embedding', 'openai')
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
