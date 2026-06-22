import { fireEvent, render, screen, within } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import type {
  AiIndexStatus,
  AiProviderConfig,
  AiProviderConnectionTestReport,
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
  testConnection: 'Test connection',
  testingConnection: 'Testing…',
  probeReachable: 'Connected',
  probeUnreachable: 'Connection issue',
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

  test('does not render until the route state has a current AI draft', () => {
    const { container } = renderSection({
      currentSettings: null,
    })

    expect(container.firstChild).toBeNull()
  })

  test('renders the live provider editor and wires the route-owned handlers', () => {
    const handlers = handlerFixture()
    renderSection({
      ...handlers,
      currentSettings: settingsFixture({ enabled: true }),
    })

    // Master AI consent toggle reflects the persisted draft and is editable.
    const master = screen.getByRole('checkbox', { name: 'Enable AI features' })
    expect(master).toBeChecked()
    fireEvent.click(master)
    expect(handlers.onToggleAi).toHaveBeenCalledTimes(1)

    // The seeded providers appear as editable fields (real AiProviderEditorList).
    expect(screen.getByDisplayValue('Local LLM')).toBeVisible()
    expect(screen.getByDisplayValue('Local Embeddings')).toBeVisible()

    // The Add-provider preset choosers route to the per-purpose handler with the
    // chosen format. LM Studio is the headline preset for the chat provider.
    const presetPickers = screen.getAllByRole('combobox', {
      name: 'Start from a preset',
    })
    fireEvent.change(presetPickers[0], { target: { value: 'lm-studio' } })
    expect(handlers.onAddProvider).toHaveBeenLastCalledWith('llm', 'lm-studio')
    fireEvent.change(presetPickers[1], { target: { value: 'ollama' } })
    expect(handlers.onAddProvider).toHaveBeenLastCalledWith(
      'embedding',
      'ollama',
    )
  })

  test('exposes every Add-provider preset and ignores the placeholder option', () => {
    const handlers = handlerFixture()
    renderSection({
      ...handlers,
      currentSettings: settingsFixture({ enabled: true }),
    })

    const presetPicker = screen.getAllByRole('combobox', {
      name: 'Start from a preset',
    })[0]
    // All five presets are offered, with LM Studio first (headline local path).
    const optionLabels = Array.from(
      presetPicker.querySelectorAll('option'),
    ).map((option) => option.textContent)
    expect(optionLabels).toEqual([
      'Add chat provider',
      'LM Studio',
      'Ollama',
      'OpenAI',
      'Anthropic',
      'Google',
    ])
    // Re-selecting the disabled placeholder must not fire onAddProvider.
    fireEvent.change(presetPicker, { target: { value: '' } })
    expect(handlers.onAddProvider).not.toHaveBeenCalled()
  })

  test('routes every per-provider editor callback to the right purpose', () => {
    const handlers = handlerFixture()
    renderSection({
      ...handlers,
      // Both providers persisted + a typed key so save/clear key are enabled.
      // No provider is pre-selected so the radios are unchecked and clicking
      // them fires onSelect.
      aiApiKeys: { 'llm-1': '  secret  ', 'embed-1': '  embed-secret  ' },
      persistedProviderIds: new Set(['llm-1', 'embed-1']),
      currentSettings: settingsFixture({
        enabled: true,
        llmProviderId: null,
        embeddingProviderId: null,
      }),
    })

    // Select (radio), update (name input), and remove for the LLM provider.
    const llmName = screen.getByDisplayValue('Local LLM')
    fireEvent.change(llmName, { target: { value: 'Edited LLM' } })
    expect(handlers.onUpdateProvider).toHaveBeenLastCalledWith('llm', 'llm-1', {
      name: 'Edited LLM',
    })
    const radios = screen.getAllByRole('radio')
    fireEvent.click(radios[0])
    expect(handlers.onSelectProvider).toHaveBeenLastCalledWith('llm', 'llm-1')
    fireEvent.click(radios[1])
    expect(handlers.onSelectProvider).toHaveBeenLastCalledWith(
      'embedding',
      'embed-1',
    )

    // Update for the embedding provider routes with the embedding purpose.
    fireEvent.change(screen.getByDisplayValue('Local Embeddings'), {
      target: { value: 'Edited Embeddings' },
    })
    expect(handlers.onUpdateProvider).toHaveBeenLastCalledWith(
      'embedding',
      'embed-1',
      { name: 'Edited Embeddings' },
    )

    // Save key + Clear key for both purposes.
    const saveButtons = screen.getAllByRole('button', { name: 'Save key' })
    const clearButtons = screen.getAllByRole('button', { name: 'Clear key' })
    saveButtons.forEach((button) => fireEvent.click(button))
    clearButtons.forEach((button) => fireEvent.click(button))
    expect(handlers.onSaveAiApiKey).toHaveBeenCalledWith('llm-1')
    expect(handlers.onSaveAiApiKey).toHaveBeenCalledWith('embed-1')
    expect(handlers.onClearAiApiKey).toHaveBeenCalledWith('llm-1')
    expect(handlers.onClearAiApiKey).toHaveBeenCalledWith('embed-1')

    // Remove for both purposes.
    const removeButtons = screen.getAllByRole('button', { name: 'Remove' })
    removeButtons.forEach((button) => fireEvent.click(button))
    expect(handlers.onRemoveProvider).toHaveBeenCalledWith('llm', 'llm-1')
    expect(handlers.onRemoveProvider).toHaveBeenCalledWith(
      'embedding',
      'embed-1',
    )

    // API key typing routes to onApiKeyChange.
    fireEvent.change(screen.getAllByPlaceholderText('Paste API key')[0], {
      target: { value: 'new-key' },
    })
    expect(handlers.onApiKeyChange).toHaveBeenCalledWith('llm-1', 'new-key')
  })

  test('always shows the AI consent disclosure', () => {
    renderSection({ currentSettings: settingsFixture({ enabled: false }) })

    const disclosure = screen.getByTestId('ai-consent-disclosure')
    expect(within(disclosure).getByText('How AI uses your data')).toBeVisible()
    // The disclosure lists no-provider, egress, and local-only bullets.
    expect(
      within(disclosure).getByText(/PathKeep ships no AI provider/),
    ).toBeVisible()
    expect(
      within(disclosure).getByText(/chat transcripts are excluded from export/),
    ).toBeVisible()
  })

  test('keeps the provider editors visible but inert while AI is off', () => {
    renderSection({ currentSettings: settingsFixture({ enabled: false }) })

    // Editors are still shown (the user sees what they can configure)…
    expect(screen.getByDisplayValue('Local LLM')).toBeVisible()
    // …but the provider name input is disabled until AI is enabled.
    expect(screen.getByDisplayValue('Local LLM')).toBeDisabled()
    expect(
      screen.getAllByRole('combobox', { name: 'Start from a preset' })[0],
    ).toBeDisabled()
    // The master toggle itself stays usable so the user can opt in.
    expect(
      screen.getByRole('checkbox', { name: 'Enable AI features' }),
    ).toBeEnabled()
  })

  test('gates the assistant + semantic sub-toggles on the master switch and routes them', () => {
    const handlers = handlerFixture()
    // Master OFF: the two sub-toggles are visible but disabled, and the
    // "enable AI above" hint appears so the user sees what is available.
    const { rerender } = renderSection({
      ...handlers,
      currentSettings: settingsFixture({
        enabled: false,
        assistantEnabled: false,
        semanticIndexEnabled: false,
      }),
    })

    const assistantOff = screen.getByRole('checkbox', {
      name: 'AI assistant (chat)',
    })
    const semanticOff = screen.getByRole('checkbox', { name: 'Smart search' })
    expect(assistantOff).not.toBeChecked()
    expect(assistantOff).toBeDisabled()
    expect(semanticOff).not.toBeChecked()
    expect(semanticOff).toBeDisabled()
    expect(
      screen.getByText('Enable AI features above to turn these on.'),
    ).toBeVisible()
    // The semantic helper narrates the one-time index build requirement.
    expect(screen.getByText(/one-time index build/i)).toBeVisible()

    // Master ON: both become interactive and route to their own handlers,
    // independent of the master toggle (no cascade).
    rerender(
      <MemoryRouter>
        <I18nProvider>
          <AiProvidersSection
            navItem={navItem}
            state={{
              ...baseState(),
              ...handlers,
              currentSettings: settingsFixture({
                enabled: true,
                assistantEnabled: false,
                semanticIndexEnabled: false,
              }),
            }}
          />
        </I18nProvider>
      </MemoryRouter>,
    )
    const assistantOn = screen.getByRole('checkbox', {
      name: 'AI assistant (chat)',
    })
    const semanticOn = screen.getByRole('checkbox', { name: 'Smart search' })
    expect(assistantOn).toBeEnabled()
    expect(semanticOn).toBeEnabled()
    fireEvent.click(assistantOn)
    expect(handlers.onToggleAssistant).toHaveBeenCalledTimes(1)
    expect(handlers.onToggleAi).not.toHaveBeenCalled()
    fireEvent.click(semanticOn)
    expect(handlers.onToggleSemanticIndex).toHaveBeenCalledTimes(1)
    expect(handlers.onToggleAi).not.toHaveBeenCalled()
  })

  test('ties the master AI toggle to the consent disclosure for screen readers', () => {
    renderSection({ currentSettings: settingsFixture({ enabled: false }) })

    const master = screen.getByRole('checkbox', { name: 'Enable AI features' })
    expect(master).toHaveAttribute('aria-describedby', 'ai-consent-disclosure')
    expect(document.getElementById('ai-consent-disclosure')).not.toBeNull()
  })

  test('probes a persisted provider and shows reachable latency + error inline', () => {
    const handlers = handlerFixture()
    const { rerender } = renderSection({
      ...handlers,
      persistedProviderIds: new Set(['llm-1', 'embed-1']),
      currentSettings: settingsFixture({ enabled: true }),
    })

    // The Test-connection button is enabled for the persisted provider and
    // routes to the per-purpose probe handler. The first button belongs to the
    // LLM editor list, the second to the embedding editor list.
    const testButtons = screen.getAllByRole('button', {
      name: 'Test connection',
    })
    fireEvent.click(testButtons[0])
    expect(handlers.onProviderProbe).toHaveBeenLastCalledWith('llm', 'llm-1')
    // The embedding list's probe arm must route with the embedding purpose, not
    // the LLM purpose (distinct AiProviderEditorList instance).
    fireEvent.click(testButtons[1])
    expect(handlers.onProviderProbe).toHaveBeenLastCalledWith(
      'embedding',
      'embed-1',
    )

    // A reachable probe shows the latency line; an unreachable one shows the
    // error message + action hint.
    rerender(
      <MemoryRouter>
        <I18nProvider>
          <AiProvidersSection
            navItem={navItem}
            state={{
              ...baseState(),
              ...handlers,
              persistedProviderIds: new Set(['llm-1', 'embed-1']),
              currentSettings: settingsFixture({ enabled: true }),
              providerProbes: {
                'llm-1': probeReportFixture({ ok: true }),
                'embed-1': probeReportFixture({
                  ok: false,
                  message: 'Connection refused',
                  actionHint: 'Start LM Studio and load a model.',
                }),
              },
            }}
          />
        </I18nProvider>
      </MemoryRouter>,
    )
    expect(screen.getByText('Connected')).toBeVisible()
    expect(screen.getByText('local-model · 42 ms')).toBeVisible()
    expect(screen.getByText('Connection issue')).toBeVisible()
    expect(screen.getByText('Connection refused')).toBeVisible()
    expect(screen.getByText('Start LM Studio and load a model.')).toBeVisible()

    // The reachable result renders on the LLM provider's card and the failure
    // result on the embedding provider's card, never crossed between lists.
    const llmCard = screen.getByDisplayValue('Local LLM').closest('article')
    const embedCard = screen
      .getByDisplayValue('Local Embeddings')
      .closest('article')
    expect(within(llmCard as HTMLElement).getByText('Connected')).toBeVisible()
    expect(
      within(llmCard as HTMLElement).queryByText('Connection issue'),
    ).toBeNull()
    expect(
      within(embedCard as HTMLElement).getByText('Connection issue'),
    ).toBeVisible()
    expect(within(embedCard as HTMLElement).queryByText('Connected')).toBeNull()
  })

  test('disables the Test-connection probe for unsaved providers and while AI is off', () => {
    // Provider not yet persisted: probe disabled even with AI on.
    const { rerender } = renderSection({
      persistedProviderIds: new Set(),
      currentSettings: settingsFixture({ enabled: true }),
    })
    expect(
      screen.getAllByRole('button', { name: 'Test connection' })[0],
    ).toBeDisabled()

    // AI off: probe disabled regardless of persistence.
    rerender(
      <MemoryRouter>
        <I18nProvider>
          <AiProvidersSection
            navItem={navItem}
            state={{
              ...baseState(),
              persistedProviderIds: new Set(['llm-1', 'embed-1']),
              currentSettings: settingsFixture({ enabled: false }),
            }}
          />
        </I18nProvider>
      </MemoryRouter>,
    )
    expect(
      screen.getAllByRole('button', { name: 'Test connection' })[0],
    ).toBeDisabled()
  })

  test('relabels the probe button while a probe is in flight', () => {
    renderSection({
      persistedProviderIds: new Set(['llm-1', 'embed-1']),
      currentSettings: settingsFixture({ enabled: true }),
      testingProviderId: 'llm-1',
    })

    // The in-flight provider shows the testing label; the other keeps the idle
    // label but is disabled (single probe at a time).
    expect(screen.getByText('Testing…')).toBeVisible()
    screen
      .getAllByRole('button', { name: /Test connection|Testing/ })
      .forEach((button) => expect(button).toBeDisabled())
  })

  test('drives save/reset from configDirty + saving', () => {
    const handlers = handlerFixture()
    const { rerender } = renderSection({
      ...handlers,
      configDirty: false,
      currentSettings: settingsFixture({ enabled: true }),
      saving: false,
    })

    // Clean draft: save + reset are disabled.
    expect(screen.getByTestId('ai-save-config')).toBeDisabled()
    expect(screen.getByTestId('ai-reset-config')).toBeDisabled()

    // Dirty draft: both enable and route to their handlers.
    rerender(
      <MemoryRouter>
        <I18nProvider>
          <AiProvidersSection
            navItem={navItem}
            state={{
              ...baseState(),
              ...handlers,
              configDirty: true,
              currentSettings: settingsFixture({ enabled: true }),
              saving: false,
            }}
          />
        </I18nProvider>
      </MemoryRouter>,
    )
    const save = screen.getByTestId('ai-save-config')
    const reset = screen.getByTestId('ai-reset-config')
    expect(save).toBeEnabled()
    expect(reset).toBeEnabled()
    fireEvent.click(save)
    expect(handlers.onSaveAiConfig).toHaveBeenCalledTimes(1)
    fireEvent.click(reset)
    expect(handlers.onResetAiConfig).toHaveBeenCalledTimes(1)
  })

  test('surfaces index health when status + meta are present', () => {
    renderSection({
      aiStatus: aiStatusFixture({ indexedItems: 4321 }),
      currentSettings: settingsFixture({ enabled: true }),
      indexMeta: {
        label: 'Ready',
        tone: 'success',
        description: '4,321 rows indexed.',
      },
    })

    expect(screen.getByText('4,321 rows indexed.')).toBeVisible()
    expect(screen.getByText('4,321')).toBeVisible()
  })

  test.each(['success', 'warning', 'blocked', 'info'] as const)(
    'maps the %s index-health tone to a callout',
    (tone) => {
      const { container } = renderSection({
        aiStatus: aiStatusFixture(),
        currentSettings: settingsFixture({ enabled: true }),
        indexMeta: {
          label: tone,
          tone,
          description: `tone-${tone}`,
        },
      })

      expect(screen.getByText(`tone-${tone}`)).toBeVisible()
      // The status-callout tone class is derived from the index-health tone.
      const expectedClass =
        tone === 'success'
          ? 'status-callout--success'
          : tone === 'warning'
            ? 'status-callout--warning'
            : tone === 'blocked'
              ? 'status-callout--blocked'
              : 'status-callout--info'
      expect(container.querySelector(`.${expectedClass}`)).not.toBeNull()
    },
  )

  test('localizes the embedding-missing index warning and passes other warnings through', () => {
    const { rerender } = renderSection({
      aiStatus: aiStatusFixture({
        warning:
          'Select an embedding provider in Settings before enabling semantic retrieval.',
      }),
      currentSettings: settingsFixture({ enabled: true }),
      indexMeta: { label: 'Warning', tone: 'warning', description: 'warn' },
    })

    // The known backend string is mapped through the embedding-missing i18n key
    // (whose English copy happens to read identically), exercising the
    // string-equality branch and the warning header.
    expect(screen.getByText('Current index warning')).toBeVisible()
    expect(
      screen.getByText(
        'Select an embedding provider in Settings before enabling semantic retrieval.',
      ),
    ).toBeVisible()

    // …while any other warning string is shown verbatim (the else branch).
    rerender(
      <MemoryRouter>
        <I18nProvider>
          <AiProvidersSection
            navItem={navItem}
            state={{
              ...baseState(),
              aiStatus: aiStatusFixture({
                warning: 'Indexer paused by policy.',
              }),
              currentSettings: settingsFixture({ enabled: true }),
              indexMeta: {
                label: 'Blocked',
                tone: 'blocked',
                description: 'blocked',
              },
            }}
          />
        </I18nProvider>
      </MemoryRouter>,
    )
    expect(screen.getByText('Indexer paused by policy.')).toBeVisible()
  })

  test('shows the saving label on the config save button while saving', () => {
    renderSection({
      configDirty: true,
      currentSettings: settingsFixture({ enabled: true }),
      saving: true,
    })

    // The save button reflects the in-flight save and stays disabled.
    expect(screen.getByTestId('ai-save-config')).toBeDisabled()
    expect(screen.getByTestId('ai-save-config').textContent).toBe('Saving…')
  })

  test('skips index health when status or meta are missing', () => {
    renderSection({
      aiStatus: null,
      currentSettings: settingsFixture({ enabled: true }),
      indexMeta: null,
    })

    expect(screen.queryByText(/rows indexed/)).toBeNull()
    // The provider editor still renders without the index-health block.
    expect(screen.getByDisplayValue('Local LLM')).toBeVisible()
  })

  test('shows a getting-started callout when no providers exist', () => {
    renderSection({
      currentSettings: settingsFixture({
        embeddingProviders: [],
        embeddingProviderId: null,
        llmProviderId: null,
        llmProviders: [],
        enabled: true,
      }),
      noProviders: true,
    })

    expect(screen.getByText('No AI providers configured yet')).toBeVisible()
  })
})

function baseState(): AiProvidersSectionState {
  return {
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
    providerProbes: {},
    providerTranslations,
    saving: false,
    testingProviderId: null,
    ...handlerFixture(),
  }
}

function renderSection(overrides: Partial<AiProvidersSectionState> = {}) {
  const state: AiProvidersSectionState = {
    ...baseState(),
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
    onProviderProbe: vi.fn().mockResolvedValue(undefined),
    onRemoveProvider: vi.fn(),
    onResetAiConfig: vi.fn(),
    onSaveAiApiKey: vi.fn().mockResolvedValue(undefined),
    onSaveAiConfig: vi.fn().mockResolvedValue(undefined),
    onSelectProvider: vi.fn(),
    onToggleAi: vi.fn(),
    onToggleAssistant: vi.fn(),
    onToggleSemanticIndex: vi.fn(),
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

function probeReportFixture(
  overrides: Partial<AiProviderConnectionTestReport> = {},
): AiProviderConnectionTestReport {
  return {
    providerId: 'llm-1',
    purpose: 'llm',
    model: 'local-model',
    ok: true,
    latencyMs: 42,
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
    ...overrides,
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
