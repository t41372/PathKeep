import { fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import type {
  AiIndexStatus,
  AiProviderConfig,
  AiSettings,
} from '../../lib/types'
import {
  AiProvidersSection,
  type AiProvidersSectionState,
} from './ai-providers-section'
import type { SettingsSectionNavItem } from './section-nav-items'

const navItem: SettingsSectionNavItem = {
  id: 'settings-ai',
  icon: 'smart_toy',
  key: 'ai',
  label: 'AI providers',
}

const providerTranslations = {
  providerName: 'Provider name',
  providerId: 'Provider ID',
  requestFormat: 'Request format',
  baseUrl: 'Base URL',
  baseUrlPlaceholder: 'http://localhost:11434/v1',
  defaultModel: 'Default model',
  modelCatalog: 'Model catalog',
  modelCatalogHint: 'model-a, model-b',
  enabled: 'Provider enabled',
  temperature: 'Temperature',
  maxTokens: 'Max tokens',
  dimensions: 'Dimensions',
  notes: 'Notes',
  apiKey: 'API key',
  apiKeyPlaceholder: 'Paste API key',
  keySaved: 'saved',
  keyNotSaved: 'not saved',
  saveKey: 'Save key',
  clearKey: 'Clear key',
  remove: 'Remove',
  requestFormatLabels: {
    openai: 'OpenAI compatible',
    anthropic: 'Anthropic',
    google: 'Google',
    ollama: 'Ollama',
    'lm-studio': 'LM Studio',
  },
}

describe('AiProvidersSection', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  test('does not render until the route state has a full AI view model', () => {
    const { container } = renderSection({
      currentSettings: null,
    })

    expect(container.firstChild).toBeNull()
  })

  test('renders deferred controls without invoking route-owned AI handlers', () => {
    const handlers = handlerFixture()
    renderSection({
      ...handlers,
      aiApiKeys: { 'llm-1': '  secret  ', 'embed-1': '  embed-secret  ' },
      configDirty: true,
      currentSettings: settingsFixture({
        llmProviderId: 'missing-llm',
        embeddingProviderId: 'missing-embedding',
      }),
      persistedProviderIds: new Set(['llm-1', 'embed-1']),
    })

    fireEvent.click(
      screen.getByRole('button', { name: 'AI features coming in v0.3' }),
    )
    fireEvent.click(screen.getByRole('button', { name: 'CHAT PROVIDERS' }))

    expect(screen.getByText('Optional AI is coming in v0.3')).toBeVisible()
    expect(screen.getByText('Coming in v0.3')).toBeVisible()
    expect(
      screen.getAllByTitle('This feature is coming in a future update.'),
    ).toHaveLength(4)
    expect(
      screen.getByRole('button', { name: 'AI features coming in v0.3' }),
    ).toBeDisabled()
    expect(
      screen.getByRole('button', { name: 'CHAT PROVIDERS' }),
    ).toBeDisabled()
    expect(handlers.onSaveAiConfig).not.toHaveBeenCalled()
    expect(handlers.onResetAiConfig).not.toHaveBeenCalled()
    expect(handlers.onToggleAi).not.toHaveBeenCalled()
    expect(handlers.onAddProvider).not.toHaveBeenCalled()
    expect(handlers.onSelectProvider).not.toHaveBeenCalled()
    expect(handlers.onUpdateProvider).not.toHaveBeenCalled()
    expect(handlers.onRemoveProvider).not.toHaveBeenCalled()
    expect(handlers.onSaveAiApiKey).not.toHaveBeenCalled()
    expect(handlers.onClearAiApiKey).not.toHaveBeenCalled()
  })

  test('keeps the deferred state when no providers exist', () => {
    renderSection({
      currentSettings: settingsFixture({
        embeddingProviders: [],
        embeddingProviderId: null,
        llmProviderId: null,
        llmProviders: [],
      }),
      noProviders: true,
    })

    expect(screen.getByText('Optional AI is coming in v0.3')).toBeVisible()
    expect(
      screen.getByRole('button', { name: 'CHAT PROVIDERS' }),
    ).toBeDisabled()
    expect(
      screen.getByRole('button', { name: 'EMBEDDING PROVIDERS' }),
    ).toBeDisabled()
    expect(screen.queryByText('No AI providers configured yet')).toBeNull()
  })

  test('does not expose stale index health or provider names while deferred', () => {
    const first = renderSection({
      aiStatus: aiStatusFixture({
        warning:
          'Select an embedding provider in Settings before enabling semantic retrieval.',
      }),
      indexMeta: {
        label: 'Warning',
        tone: 'warning',
        description: 'Embedding provider required.',
      },
    })

    expect(screen.queryByText('Local LLM')).toBeNull()
    expect(screen.queryByText('Local Embeddings')).toBeNull()
    expect(
      first.container.querySelector('.status-callout--info'),
    ).toBeInTheDocument()
    expect(
      screen.queryByText(
        'Select an embedding provider in Settings before enabling semantic retrieval.',
      ),
    ).toBeNull()
    first.unmount()

    const blocked = renderSection({
      aiStatus: aiStatusFixture({ warning: 'Indexer is paused by policy.' }),
      indexMeta: {
        label: 'Blocked',
        tone: 'blocked',
        description: 'Manual intervention required.',
      },
    })

    expect(
      blocked.container.querySelector('.status-callout--info'),
    ).toBeInTheDocument()
    expect(screen.queryByText('Indexer is paused by policy.')).toBeNull()
    blocked.unmount()

    const info = renderSection({
      indexMeta: {
        label: 'Checking',
        tone: 'info',
        description: 'Runtime status is still loading.',
      },
    })

    expect(
      info.container.querySelector('.status-callout--info'),
    ).toBeInTheDocument()
  })
})

function renderSection(overrides: Partial<AiProvidersSectionState> = {}) {
  const state: AiProvidersSectionState = {
    aiApiKeys: {},
    aiStatus: aiStatusFixture(),
    configDirty: false,
    copyFeedback: null,
    currentSettings: settingsFixture(),
    indexMeta: {
      label: 'Ready',
      tone: 'success',
      description: '1,200 rows indexed.',
    },
    integrationError: null,
    integrationPreview: null,
    noProviders: false,
    persistedProviderIds: new Set(['llm-1']),
    providerTranslations,
    saving: false,
    ...handlerFixture(),
    ...overrides,
  }

  return render(
    <MemoryRouter>
      <I18nProvider>
        <AiProvidersSection navItem={navItem} state={state} />
      </I18nProvider>
    </MemoryRouter>,
  )
}

function handlerFixture() {
  return {
    onAddProvider: vi.fn(),
    onApiKeyChange: vi.fn(),
    onClearAiApiKey: vi.fn().mockResolvedValue(undefined),
    onCopyIntegrationValue: vi.fn().mockResolvedValue(undefined),
    onOpenPath: vi.fn(),
    onRemoveProvider: vi.fn(),
    onResetAiConfig: vi.fn(),
    onSaveAiApiKey: vi.fn().mockResolvedValue(undefined),
    onSaveAiConfig: vi.fn().mockResolvedValue(undefined),
    onSelectProvider: vi.fn(),
    onToggleAi: vi.fn(),
    onUpdateProvider: vi.fn(),
  }
}

function settingsFixture(overrides: Partial<AiSettings> = {}): AiSettings {
  return {
    enabled: false,
    assistantEnabled: false,
    semanticIndexEnabled: true,
    mcpEnabled: false,
    skillEnabled: false,
    autoIndexAfterBackup: true,
    jobQueuePaused: false,
    jobQueueConcurrency: 1,
    enrichmentEnabled: false,
    enrichmentPlugins: [],
    llmProviderId: 'llm-1',
    embeddingProviderId: 'embed-1',
    retrievalTopK: 8,
    assistantSystemPrompt: '',
    llmProviders: [providerFixture('llm-1', 'llm')],
    embeddingProviders: [providerFixture('embed-1', 'embedding')],
    ...overrides,
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
    apiKeySaved: id === 'llm-1',
    defaultModel: purpose === 'llm' ? 'gpt-test' : 'text-embedding-test',
    modelCatalog: [],
    temperature: purpose === 'llm' ? 0.7 : null,
    maxTokens: purpose === 'llm' ? 1200 : null,
    dimensions: purpose === 'embedding' ? 1536 : null,
    notes: null,
  }
}

function aiStatusFixture(
  overrides: Partial<AiIndexStatus> = {},
): AiIndexStatus {
  return {
    enabled: true,
    assistantEnabled: false,
    mcpEnabled: false,
    skillEnabled: false,
    state: 'ready',
    ready: true,
    indexedItems: 1200,
    lastIndexedAt: '2026-04-25T12:00:00Z',
    llmProviderId: 'llm-1',
    embeddingProviderId: 'embed-1',
    queuePaused: false,
    queueConcurrency: 1,
    queuedJobs: 0,
    runningJobs: 0,
    failedJobs: 0,
    recentJobs: [],
    semanticSidecarBytes: 2048,
    semanticMetadataBytes: 1024,
    estimatedEmbeddingTokens: 4096,
    warning: null,
    ...overrides,
  }
}
