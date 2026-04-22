/**
 * This module implements the App.helpers front-end surface.
 *
 * Why this file exists:
 * - It is part of the active `src/` tree and should explain its own role without forcing the next reader to scan unrelated files first.
 * - When this file changes, the surrounding comments should keep the intent, boundaries, and main declarations easy to see at a glance.
 *
 * Main declarations:
 * - `renderWithI18n`
 *
 * Source-of-truth notes:
 * - Keep the implementation aligned with the accepted product, design, and architecture documents.
 * - Prefer explicit structure over cleverness so the codebase stays navigable as the front-end keeps growing.
 */

import type { ReactNode } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, test, vi } from 'vitest'
import { AiProviderEditorList } from './components/ai-provider-editor'
import {
  DataRow,
  FieldBlock,
  Glyph,
  InfoStat,
  StatusTag,
  Surface,
  ToggleRow,
} from './components/ui'
import {
  OperationWorkflow,
  PreviewEntryList,
  type WorkflowStep,
} from './components/review'
import { formatDateTime, formatDuration } from './lib/format'
import {
  createNamespaceTranslator,
  createTranslator,
  type ResolvedLanguage,
} from './lib/i18n'
import { I18nContext, type I18nContextValue } from './lib/i18n/context'
import type { AiProviderConfig } from './lib/types'

/**
 * Explains how render with i18n works.
 *
 * Keeping this declaration named and documented is part of making the front-end codebase navigable without a separate documentation site.
 */
function renderWithI18n(ui: ReactNode, language: ResolvedLanguage = 'en') {
  const namespaceCache = new Map<
    string,
    ReturnType<typeof createNamespaceTranslator>
  >()
  const value: I18nContextValue = {
    language,
    preference: language,
    setLanguagePreference: vi.fn(),
    t: createTranslator(language),
    ns: (namespace) => {
      const cached = namespaceCache.get(namespace)
      if (cached) return cached
      const translator = createNamespaceTranslator(language, namespace)
      namespaceCache.set(namespace, translator)
      return translator
    },
  }

  return render(<I18nContext.Provider value={value}>{ui}</I18nContext.Provider>)
}

describe('App helpers', () => {
  test('formats date and duration edge cases', () => {
    expect(formatDateTime(null, 'en')).toBeNull()
    expect(formatDateTime('2026-04-03T12:00:00.000Z', 'en')).toContain('2026')
    expect(formatDuration(0)).toBe('0s')
    expect(formatDuration(59_000)).toBe('59s')
    expect(formatDuration(125_000)).toBe('2m 5s')
  })

  test('renders generic surface primitives', async () => {
    const user = userEvent.setup()
    const toggleSpy = vi.fn()

    renderWithI18n(
      <div>
        <Surface
          actions={<button type="button">Act</button>}
          eyebrow="Eyebrow"
          icon="history"
          title="Panel"
        >
          <FieldBlock
            label="Label"
            control={<input aria-label="Label input" type="text" />}
          />
          <ToggleRow checked={false} label="Enabled" onChange={toggleSpy} />
          <dl>
            <DataRow label="Data" value="Value" />
          </dl>
          <InfoStat label="Count" value="42" />
          <StatusTag tone="success">Done</StatusTag>
          <Glyph filled icon="check" />
        </Surface>
      </div>,
    )

    expect(screen.getByText('Panel')).toBeInTheDocument()
    expect(screen.getByText('42')).toBeInTheDocument()
    expect(screen.getByText('Done')).toBeInTheDocument()
    await user.click(screen.getByLabelText('Enabled'))
    expect(toggleSpy).toHaveBeenCalledWith(true)
  })

  test('keeps glyphs decorative by default and supports explicit labels', () => {
    renderWithI18n(
      <div>
        <Glyph icon="check" />
        <Glyph icon="warning" label="Warning state" />
      </div>,
    )

    expect(
      screen.queryByRole('img', { name: /check/i }),
    ).not.toBeInTheDocument()
    expect(
      screen.getByRole('img', { name: 'Warning state' }),
    ).toBeInTheDocument()
  })

  test('renders workflow states and supports command copy', async () => {
    const user = userEvent.setup()
    const copySpy = vi.fn().mockResolvedValue(undefined)
    const actionSpy = vi.fn()
    const steps: WorkflowStep[] = [
      {
        id: 'complete',
        title: 'Complete',
        status: 'complete',
        summary: 'Already done',
        reason: 'Covered before.',
        files: ['/tmp/one'],
      },
      {
        id: 'current',
        title: 'Current',
        status: 'pending',
        summary: 'Do this now',
        reason: 'Needed for safety.',
        commands: ['echo test'],
        checklist: ['Read the file', 'Confirm the change'],
        actions: (
          <button type="button" onClick={actionSpy}>
            Continue
          </button>
        ),
      },
      {
        id: 'later',
        title: 'Later',
        status: 'pending',
        summary: 'Wait for the earlier step',
        reason: 'Dependent on current step.',
      },
    ]

    renderWithI18n(
      <OperationWorkflow
        actionLabel="Workflow"
        labels={{
          why: 'Why',
          files: 'Files',
          commands: 'Commands',
          checklist: 'Checklist',
          copy: 'Copy',
          current: 'Current',
          complete: 'Complete',
          pending: 'Pending',
          command: (index) => `Command ${index}`,
        }}
        onCopy={copySpy}
        steps={steps}
      />,
    )

    expect(screen.getAllByText('Complete').length).toBeGreaterThanOrEqual(2)
    expect(screen.getAllByText('Current').length).toBeGreaterThanOrEqual(2)
    expect(screen.getAllByText('Pending').length).toBeGreaterThanOrEqual(2)
    expect(screen.getByText('/tmp/one')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Copy' }))
    expect(copySpy).toHaveBeenCalledWith('echo test')
    await user.click(screen.getByRole('button', { name: 'Continue' }))
    expect(actionSpy).toHaveBeenCalledTimes(1)
  })

  test('renders AI provider editors for llm and embedding providers', async () => {
    const user = userEvent.setup()
    const addSpy = vi.fn()
    const apiKeySpy = vi.fn()
    const clearSpy = vi.fn()
    const removeSpy = vi.fn()
    const saveSpy = vi.fn()
    const selectSpy = vi.fn()
    const updateSpy = vi.fn()

    const llmProvider: AiProviderConfig = {
      id: 'llm-preview',
      name: 'Preview LLM',
      purpose: 'llm',
      requestFormat: 'openai',
      enabled: true,
      baseUrl: 'https://api.example.com/v1',
      apiKeySaved: true,
      defaultModel: 'gpt-4.1-mini',
      modelCatalog: ['gpt-4.1-mini', 'gpt-4.1'],
      temperature: 0.3,
      maxTokens: 1400,
      dimensions: null,
      notes: 'LLM notes',
    }
    const embeddingProvider: AiProviderConfig = {
      id: 'embedding-preview',
      name: 'Preview Embedding',
      purpose: 'embedding',
      requestFormat: 'openai',
      enabled: false,
      baseUrl: null,
      apiKeySaved: false,
      defaultModel: 'text-embedding-3-large',
      modelCatalog: ['text-embedding-3-large'],
      temperature: null,
      maxTokens: null,
      dimensions: 1536,
      notes: null,
    }

    const translations = {
      providerName: 'Provider name',
      providerId: 'Provider id',
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
      remove: 'Remove',
      requestFormatLabels: {
        openai: 'OpenAI-compatible',
        anthropic: 'Anthropic-compatible',
        google: 'Google AI Studio',
        ollama: 'Ollama',
        'lm-studio': 'LM Studio',
      },
    }

    const { rerender } = renderWithI18n(
      <AiProviderEditorList
        addLabel="Add provider"
        apiKeys={{ 'llm-preview': 'secret', 'embedding-preview': '' }}
        onAdd={addSpy}
        onApiKeyChange={apiKeySpy}
        onClearKey={clearSpy}
        onRemove={removeSpy}
        onSaveKey={saveSpy}
        onSelect={selectSpy}
        onUpdate={updateSpy}
        providers={[llmProvider]}
        purpose="llm"
        selectedProviderId="llm-preview"
        title="LLM providers"
        translations={translations}
      />,
    )

    await user.click(screen.getByRole('button', { name: 'Add provider' }))
    expect(addSpy).toHaveBeenCalledTimes(1)
    await user.click(screen.getByRole('button', { name: 'Remove' }))
    expect(removeSpy).toHaveBeenCalledWith('llm-preview')
    await user.type(screen.getByDisplayValue('Preview LLM'), ' updated')
    expect(updateSpy).toHaveBeenCalled()
    await user.selectOptions(
      screen.getByRole('combobox', { name: 'Request format' }),
      'google',
    )
    await user.clear(screen.getByDisplayValue('https://api.example.com/v1'))
    await user.type(
      screen.getByPlaceholderText('https://api.example.com/v1'),
      'https://override.example.com/v1',
    )
    const defaultModelInput = screen.getByDisplayValue('gpt-4.1-mini')
    await user.clear(defaultModelInput)
    await user.type(defaultModelInput, 'gpt-5-mini')
    const modelCatalogInput = screen.getByDisplayValue('gpt-4.1-mini, gpt-4.1')
    await user.clear(modelCatalogInput)
    await user.type(modelCatalogInput, 'gpt-5-mini, gpt-5')
    const temperatureInput = screen.getByRole('spinbutton', {
      name: 'Temperature',
    })
    await user.clear(temperatureInput)
    await user.type(temperatureInput, '1')
    const maxTokensInput = screen.getByRole('spinbutton', {
      name: 'Max tokens',
    })
    await user.clear(maxTokensInput)
    await user.type(maxTokensInput, '1600')
    const notesInput = screen.getByRole('textbox', { name: 'Notes' })
    await user.clear(notesInput)
    await user.type(notesInput, 'Updated notes')
    await user.click(screen.getByLabelText('Enabled'))
    await user.clear(screen.getByPlaceholderText('sk-...'))
    await user.type(screen.getByPlaceholderText('sk-...'), 'next-secret')
    expect(apiKeySpy).toHaveBeenCalled()
    await user.click(screen.getByRole('button', { name: 'Save key' }))
    expect(saveSpy).toHaveBeenCalledWith('llm-preview')
    await user.click(screen.getByRole('button', { name: 'Clear key' }))
    expect(clearSpy).toHaveBeenCalledWith('llm-preview')

    rerender(
      <AiProviderEditorList
        addLabel="Add embedding"
        apiKeys={{ 'embedding-preview': '' }}
        onAdd={addSpy}
        onApiKeyChange={apiKeySpy}
        onClearKey={clearSpy}
        onRemove={removeSpy}
        onSaveKey={saveSpy}
        onSelect={selectSpy}
        onUpdate={updateSpy}
        providers={[embeddingProvider]}
        purpose="embedding"
        selectedProviderId={null}
        title="Embedding providers"
        translations={translations}
      />,
    )

    await user.click(screen.getByRole('radio'))
    expect(selectSpy).toHaveBeenCalledWith('embedding-preview')
    await user.clear(screen.getByDisplayValue('1536'))
    await user.type(
      screen.getByRole('spinbutton', { name: 'Dimensions' }),
      '3072',
    )
    expect(updateSpy).toHaveBeenCalledWith(
      'embedding-preview',
      expect.any(Object),
    )
  })

  test('renders preview entries with imported and preview statuses', () => {
    render(
      <PreviewEntryList
        entries={[
          {
            sourcePath: '/tmp/import.jsonl',
            url: 'https://example.com',
            title: 'Example',
            visitedAt: '2026-04-03T12:00:00.000Z',
            sourceVisitId: 1,
            status: 'imported',
          },
          {
            sourcePath: '/tmp/import.jsonl',
            url: 'https://example.org',
            title: null,
            visitedAt: '2026-04-03T12:05:00.000Z',
            sourceVisitId: 2,
            status: 'preview',
          },
        ]}
        language="en"
      />,
    )

    expect(screen.getByText('Example')).toBeInTheDocument()
    expect(screen.getAllByText('https://example.org')).toHaveLength(2)
    expect(screen.getByText('imported')).toBeInTheDocument()
    expect(screen.getByText('preview')).toBeInTheDocument()
  })

  test('covers provider empty states, id fallbacks, and default numeric values', () => {
    const noop = vi.fn()

    const sparseLlmProvider: AiProviderConfig = {
      id: 'llm-fallback',
      name: '',
      purpose: 'llm',
      requestFormat: 'openai',
      enabled: false,
      baseUrl: null,
      apiKeySaved: false,
      defaultModel: '',
      modelCatalog: [],
      temperature: null,
      maxTokens: null,
      dimensions: null,
      notes: null,
    }
    const sparseEmbeddingProvider: AiProviderConfig = {
      id: 'embedding-fallback',
      name: '',
      purpose: 'embedding',
      requestFormat: 'openai',
      enabled: false,
      baseUrl: null,
      apiKeySaved: false,
      defaultModel: '',
      modelCatalog: [],
      temperature: null,
      maxTokens: null,
      dimensions: null,
      notes: null,
    }

    const translations = {
      providerName: 'Provider name',
      providerId: 'Provider ID',
      requestFormat: 'Request format',
      baseUrl: 'Base URL',
      baseUrlPlaceholder: 'https://api.example.com/v1',
      defaultModel: 'Default model',
      modelCatalog: 'Available models',
      modelCatalogHint: 'Comma-separated model names',
      enabled: 'Provider enabled',
      temperature: 'Temperature',
      maxTokens: 'Max tokens',
      dimensions: 'Embedding dimensions',
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
      },
    }

    renderWithI18n(
      <div>
        <AiProviderEditorList
          addLabel="Add empty LLM"
          apiKeys={{}}
          onAdd={noop}
          onApiKeyChange={noop}
          onClearKey={noop}
          onRemove={noop}
          onSaveKey={noop}
          onSelect={noop}
          onUpdate={noop}
          providers={[]}
          purpose="llm"
          selectedProviderId={null}
          title="No providers yet"
          translations={translations}
        />
        <AiProviderEditorList
          addLabel="Add sparse LLM"
          apiKeys={{ 'llm-fallback': '' }}
          onAdd={noop}
          onApiKeyChange={noop}
          onClearKey={noop}
          onRemove={noop}
          onSaveKey={noop}
          onSelect={noop}
          onUpdate={noop}
          providers={[sparseLlmProvider]}
          purpose="llm"
          selectedProviderId={null}
          title="Sparse LLM"
          translations={translations}
        />
        <AiProviderEditorList
          addLabel="Add sparse embedding"
          apiKeys={{ 'embedding-fallback': '' }}
          onAdd={noop}
          onApiKeyChange={noop}
          onClearKey={noop}
          onRemove={noop}
          onSaveKey={noop}
          onSelect={noop}
          onUpdate={noop}
          providers={[sparseEmbeddingProvider]}
          purpose="embedding"
          selectedProviderId={null}
          title="Sparse embedding"
          translations={translations}
        />
      </div>,
    )

    expect(screen.getAllByText('No providers yet').length).toBeGreaterThan(0)
    expect(screen.getAllByText('llm-fallback').length).toBeGreaterThan(0)
    expect(screen.getAllByText('embedding-fallback').length).toBeGreaterThan(0)
    expect(screen.getByRole('spinbutton', { name: 'Temperature' })).toHaveValue(
      0,
    )
    expect(screen.getByRole('spinbutton', { name: 'Max tokens' })).toHaveValue(
      1200,
    )
    expect(
      screen.getByRole('spinbutton', { name: 'Embedding dimensions' }),
    ).toHaveValue(1536)
  })
})
