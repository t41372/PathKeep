/**
 * @file ai-provider-editor.test.tsx
 * @description Mutation-focused contract tests for the shared AI provider editor.
 * @module components/ai-provider-editor.test
 *
 * ## Responsibilities
 * - Pin exact field-to-patch behavior for LLM and embedding provider editors.
 * - Cover disabled, selected, empty, and API-key saved states that settings pages rely on.
 * - Keep request-format and model-catalog parsing regressions visible.
 *
 * ## Not responsible for
 * - Testing Settings route state reducers or backend persistence.
 * - Repeating route-level integration review copy covered under `src/pages/settings`.
 * - Owning visual CSS beyond stable class/state contracts required by the shell.
 *
 * ## Dependencies
 * - Uses the real `AiProviderEditorList` plus shared `FieldBlock`/`ToggleRow` wrappers.
 * - Depends on the shipping `AiProviderConfig` type for provider shape.
 *
 * ## Performance notes
 * - Uses direct DOM events for exact input values so the test avoids long per-character user-event loops.
 */

import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import type { AiProviderConfig, AiRequestFormat } from '../lib/types'
import { AiProviderEditorList } from './ai-provider-editor'

const translations = {
  providerName: 'Provider name',
  providerId: 'Provider ID',
  requestFormat: 'Request format',
  baseUrl: 'Base URL',
  baseUrlPlaceholder: 'https://api.example.com/v1',
  defaultModel: 'Default model',
  modelCatalog: 'Model catalog',
  modelCatalogHint: 'model-a, model-b',
  enabled: 'Enabled',
  temperature: 'Temperature',
  maxTokens: 'Max tokens',
  dimensions: 'Dimensions',
  notes: 'Notes',
  apiKey: 'API key',
  apiKeyPlaceholder: 'sk-...',
  keySaved: 'Saved',
  keyNotSaved: 'Not saved',
  saveKey: 'Save key',
  clearKey: 'Clear key',
  remove: 'Remove provider',
  requestFormatLabels: {
    openai: 'OpenAI-compatible',
    anthropic: 'Anthropic-compatible',
    google: 'Google AI Studio',
    ollama: 'Ollama',
    'lm-studio': 'LM Studio',
  } satisfies Record<AiRequestFormat, string>,
}

function providerFixture(
  patch: Partial<AiProviderConfig> = {},
): AiProviderConfig {
  return {
    id: 'llm-primary',
    name: 'Primary LLM',
    purpose: 'llm',
    requestFormat: 'openai',
    enabled: true,
    baseUrl: 'https://api.example.com/v1',
    apiKeySaved: true,
    defaultModel: 'gpt-4.1-mini',
    modelCatalog: ['gpt-4.1-mini', 'gpt-4.1'],
    temperature: 0.3,
    maxTokens: 1200,
    dimensions: null,
    notes: 'Primary provider',
    ...patch,
  }
}

function renderEditor({
  apiKeys = {},
  disabled,
  onAdd = vi.fn(),
  onApiKeyChange = vi.fn(),
  onClearKey = vi.fn(),
  onClearKeyDisabled,
  onRemove = vi.fn(),
  onSaveKey = vi.fn(),
  onSaveKeyDisabled,
  onSelect = vi.fn(),
  onUpdate = vi.fn(),
  providers = [providerFixture()],
  purpose = 'llm',
  selectedProviderId = 'llm-primary',
  title = 'LLM providers',
}: Partial<Parameters<typeof AiProviderEditorList>[0]> = {}) {
  render(
    <AiProviderEditorList
      addLabel="Add provider"
      apiKeys={apiKeys}
      {...(disabled === undefined ? {} : { disabled })}
      onAdd={onAdd}
      onApiKeyChange={onApiKeyChange}
      onClearKey={onClearKey}
      onClearKeyDisabled={onClearKeyDisabled}
      onRemove={onRemove}
      onSaveKey={onSaveKey}
      onSaveKeyDisabled={onSaveKeyDisabled}
      onSelect={onSelect}
      onUpdate={onUpdate}
      providers={providers}
      purpose={purpose}
      selectedProviderId={selectedProviderId}
      title={title}
      translations={translations}
    />,
  )

  return {
    onAdd,
    onApiKeyChange,
    onClearKey,
    onRemove,
    onSaveKey,
    onSelect,
    onUpdate,
  }
}

function changeField(label: string, value: string) {
  fireEvent.change(screen.getByLabelText(label), { target: { value } })
}

describe('AiProviderEditorList', () => {
  test('renders LLM provider controls and emits exact field patches', () => {
    const callbacks = renderEditor({
      apiKeys: { 'llm-primary': 'pending-secret' },
      selectedProviderId: null,
    })

    const card = screen.getByText('Primary LLM').closest('article')
    expect(card).toBeInstanceOf(HTMLElement)
    expect(card).not.toHaveClass('selected')
    expect(card).toHaveClass('providerCard')
    expect(card?.className.trim()).toBe('providerCard')
    expect(screen.getByLabelText('Primary LLM')).not.toBeChecked()
    expect(screen.getByLabelText('Primary LLM')).toHaveAttribute(
      'name',
      'llm-provider',
    )
    expect(screen.getByRole('button', { name: 'Add provider' })).toBeEnabled()
    expect(
      screen.getByRole('button', { name: 'Remove provider' }),
    ).toBeEnabled()
    expect(screen.getByText('API key · Saved')).toBeInTheDocument()
    expect(screen.getByPlaceholderText('sk-...')).toHaveValue('pending-secret')
    expect(screen.getAllByRole('option')).toHaveLength(5)
    expect(
      within(screen.getByLabelText('Request format')).getAllByRole('option'),
    ).toHaveLength(5)
    expect(screen.getByRole('option', { name: 'LM Studio' })).toHaveValue(
      'lm-studio',
    )
    expect(screen.getByLabelText('Model catalog')).toHaveValue(
      'gpt-4.1-mini, gpt-4.1',
    )

    fireEvent.click(screen.getByRole('button', { name: 'Add provider' }))
    expect(callbacks.onAdd).toHaveBeenCalledTimes(1)

    fireEvent.click(screen.getByRole('button', { name: 'Remove provider' }))
    expect(callbacks.onRemove).toHaveBeenCalledWith('llm-primary')

    fireEvent.click(screen.getByLabelText('Primary LLM'))
    expect(callbacks.onSelect).toHaveBeenCalledWith('llm-primary')

    changeField('Provider name', 'Renamed LLM')
    expect(callbacks.onUpdate).toHaveBeenLastCalledWith('llm-primary', {
      name: 'Renamed LLM',
    })

    fireEvent.change(screen.getByLabelText('Request format'), {
      target: { value: 'google' },
    })
    expect(callbacks.onUpdate).toHaveBeenLastCalledWith('llm-primary', {
      requestFormat: 'google',
    })

    changeField('Base URL', '')
    expect(callbacks.onUpdate).toHaveBeenLastCalledWith('llm-primary', {
      baseUrl: null,
    })
    changeField('Base URL', 'https://override.example.com/v1')
    expect(callbacks.onUpdate).toHaveBeenLastCalledWith('llm-primary', {
      baseUrl: 'https://override.example.com/v1',
    })

    changeField('Default model', 'gpt-5-mini')
    expect(callbacks.onUpdate).toHaveBeenLastCalledWith('llm-primary', {
      defaultModel: 'gpt-5-mini',
    })

    changeField('Model catalog', ' gpt-5-mini, ,gpt-5, ')
    expect(callbacks.onUpdate).toHaveBeenLastCalledWith('llm-primary', {
      modelCatalog: ['gpt-5-mini', 'gpt-5'],
    })

    changeField('Temperature', '1.2')
    expect(callbacks.onUpdate).toHaveBeenLastCalledWith('llm-primary', {
      temperature: 1.2,
    })

    changeField('Max tokens', '4096')
    expect(callbacks.onUpdate).toHaveBeenLastCalledWith('llm-primary', {
      maxTokens: 4096,
    })

    changeField('Notes', '')
    expect(callbacks.onUpdate).toHaveBeenLastCalledWith('llm-primary', {
      notes: null,
    })
    changeField('Notes', 'Keep local')
    expect(callbacks.onUpdate).toHaveBeenLastCalledWith('llm-primary', {
      notes: 'Keep local',
    })

    fireEvent.click(screen.getByLabelText('Enabled'))
    expect(callbacks.onUpdate).toHaveBeenLastCalledWith('llm-primary', {
      enabled: false,
    })

    changeField('API key · Saved', 'next-secret')
    expect(callbacks.onApiKeyChange).toHaveBeenLastCalledWith(
      'llm-primary',
      'next-secret',
    )

    fireEvent.click(screen.getByRole('button', { name: 'Save key' }))
    expect(callbacks.onSaveKey).toHaveBeenCalledWith('llm-primary')
    fireEvent.click(screen.getByRole('button', { name: 'Clear key' }))
    expect(callbacks.onClearKey).toHaveBeenCalledWith('llm-primary')
  })

  test('renders embedding, empty, disabled, and API-key fallback states', () => {
    const embeddingProvider = providerFixture({
      id: 'embedding-local',
      name: '',
      purpose: 'embedding',
      requestFormat: 'ollama',
      enabled: false,
      baseUrl: null,
      apiKeySaved: false,
      defaultModel: 'nomic-embed-text',
      modelCatalog: [],
      temperature: null,
      maxTokens: null,
      dimensions: null,
      notes: null,
    })
    renderEditor({
      apiKeys: {},
      disabled: true,
      onClearKeyDisabled: (providerId) => providerId === 'embedding-local',
      onSaveKeyDisabled: (providerId) => providerId === 'embedding-local',
      providers: [embeddingProvider],
      purpose: 'embedding',
      selectedProviderId: null,
      title: 'Embedding providers',
    })

    expect(screen.getAllByText('embedding-local')).toHaveLength(2)
    expect(screen.getByLabelText('embedding-local')).not.toBeChecked()
    expect(screen.getByLabelText('embedding-local')).toBeDisabled()
    expect(screen.getByLabelText('embedding-local')).toHaveAttribute(
      'name',
      'embedding-provider',
    )
    expect(screen.getByText('API key · Not saved')).toBeInTheDocument()
    expect(screen.getByLabelText('API key · Not saved')).toHaveValue('')
    expect(screen.getByLabelText('Base URL')).toHaveValue('')
    expect(screen.getByLabelText('Notes')).toHaveValue('')
    expect(screen.queryByLabelText('Temperature')).not.toBeInTheDocument()
    expect(screen.queryByLabelText('Max tokens')).not.toBeInTheDocument()
    expect(screen.getByLabelText('Dimensions')).toHaveValue(1536)
    expect(screen.getByRole('button', { name: 'Add provider' })).toBeDisabled()
    expect(
      screen.getByRole('button', { name: 'Remove provider' }),
    ).toBeDisabled()
    expect(screen.getByLabelText('Provider name')).toBeDisabled()
    expect(screen.getByLabelText('Enabled')).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Save key' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Clear key' })).toBeDisabled()

    renderEditor({
      providers: [],
      selectedProviderId: null,
      title: 'No providers yet',
    })
    expect(
      screen
        .getAllByText('No providers yet')
        .find((element) => element.classList.contains('emptyState')),
    ).toBeDefined()
  })

  test('keeps enabled embedding controls interactive when only key actions are disabled', () => {
    const embeddingProvider = providerFixture({
      id: 'embedding-live',
      name: 'Embedding live',
      purpose: 'embedding',
      requestFormat: 'lm-studio',
      enabled: false,
      apiKeySaved: false,
      dimensions: 384,
      temperature: null,
      maxTokens: null,
    })
    const callbacks = renderEditor({
      onClearKeyDisabled: () => false,
      onSaveKeyDisabled: () => true,
      providers: [embeddingProvider],
      purpose: 'embedding',
      selectedProviderId: 'embedding-live',
    })

    const card = screen.getByText('Embedding live').closest('article')
    expect(card).toBeInstanceOf(HTMLElement)
    expect(card).toHaveClass('selected')
    expect(screen.getByLabelText('Embedding live')).toBeChecked()

    changeField('Dimensions', '768')
    expect(callbacks.onUpdate).toHaveBeenLastCalledWith('embedding-live', {
      dimensions: 768,
    })
    expect(screen.getByRole('button', { name: 'Save key' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Clear key' })).toBeEnabled()
    fireEvent.click(screen.getByRole('button', { name: 'Clear key' }))
    expect(callbacks.onClearKey).toHaveBeenCalledWith('embedding-live')
  })
})
