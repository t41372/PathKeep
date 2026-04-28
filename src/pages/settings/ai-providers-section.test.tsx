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

  test('wires provider editor controls back to route-owned handlers', () => {
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

    fireEvent.click(screen.getByRole('button', { name: 'Save' }))
    fireEvent.click(screen.getByRole('button', { name: 'Discard changes' }))
    fireEvent.click(screen.getByLabelText('Enable AI features'))
    fireEvent.click(screen.getByRole('button', { name: 'Add chat provider' }))
    fireEvent.click(
      screen.getByRole('button', { name: 'Add embedding provider' }),
    )
    fireEvent.click(screen.getByLabelText('Local LLM'))
    fireEvent.change(screen.getByDisplayValue('Local LLM'), {
      target: { value: 'Renamed LLM' },
    })
    fireEvent.click(screen.getByLabelText('Local Embeddings'))
    fireEvent.change(screen.getByDisplayValue('Local Embeddings'), {
      target: { value: 'Renamed Embeddings' },
    })
    fireEvent.click(screen.getAllByRole('button', { name: 'Remove' })[0])
    fireEvent.click(screen.getAllByRole('button', { name: 'Remove' })[1])
    fireEvent.click(screen.getAllByRole('button', { name: 'Save key' })[0])
    fireEvent.click(screen.getAllByRole('button', { name: 'Save key' })[1])
    fireEvent.click(screen.getAllByRole('button', { name: 'Clear key' })[0])
    fireEvent.click(screen.getAllByRole('button', { name: 'Clear key' })[1])

    expect(handlers.onSaveAiConfig).toHaveBeenCalledTimes(1)
    expect(handlers.onResetAiConfig).toHaveBeenCalledTimes(1)
    expect(handlers.onToggleAi).toHaveBeenCalledTimes(1)
    expect(handlers.onAddProvider).toHaveBeenNthCalledWith(1, 'llm')
    expect(handlers.onAddProvider).toHaveBeenNthCalledWith(2, 'embedding')
    expect(handlers.onSelectProvider).toHaveBeenCalledWith('llm', 'llm-1')
    expect(handlers.onUpdateProvider).toHaveBeenCalledWith('llm', 'llm-1', {
      name: 'Renamed LLM',
    })
    expect(handlers.onUpdateProvider).toHaveBeenCalledWith(
      'embedding',
      'embed-1',
      {
        name: 'Renamed Embeddings',
      },
    )
    expect(handlers.onRemoveProvider).toHaveBeenCalledWith('llm', 'llm-1')
    expect(handlers.onRemoveProvider).toHaveBeenCalledWith(
      'embedding',
      'embed-1',
    )
    expect(handlers.onSaveAiApiKey).toHaveBeenCalledWith('llm-1')
    expect(handlers.onSaveAiApiKey).toHaveBeenCalledWith('embed-1')
    expect(handlers.onClearAiApiKey).toHaveBeenCalledWith('llm-1')
    expect(handlers.onClearAiApiKey).toHaveBeenCalledWith('embed-1')
    expect(handlers.onSelectProvider).toHaveBeenCalledWith(
      'embedding',
      'embed-1',
    )
    expect(screen.getAllByText('None')).toHaveLength(2)
  })

  test('shows the getting-started callout when no providers exist', () => {
    renderSection({
      currentSettings: settingsFixture({
        embeddingProviders: [],
        embeddingProviderId: null,
        llmProviderId: null,
        llmProviders: [],
      }),
      noProviders: true,
    })

    expect(
      screen.getByText('No AI providers configured yet'),
    ).toBeInTheDocument()
    expect(screen.getAllByText('CHAT PROVIDERS')).toHaveLength(2)
    expect(screen.getAllByText('EMBEDDING PROVIDERS')).toHaveLength(2)
    expect(screen.getAllByText('None')).toHaveLength(2)
  })

  test('renders active provider names, index health tones, and warning variants', () => {
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

    expect(screen.getAllByText('Local LLM').length).toBeGreaterThan(0)
    expect(screen.getAllByText('Local Embeddings').length).toBeGreaterThan(0)
    expect(
      first.container.querySelector('.status-callout--warning'),
    ).toBeInTheDocument()
    expect(
      screen.getByText(
        'Select an embedding provider in Settings before enabling semantic retrieval.',
      ),
    ).toBeVisible()
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
      blocked.container.querySelector('.status-callout--blocked'),
    ).toBeInTheDocument()
    expect(screen.getByText('Indexer is paused by policy.')).toBeVisible()
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
