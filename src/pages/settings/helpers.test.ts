/**
 * This test file protects the shipped behavior of the Settings front-end surface.
 *
 * Why this file exists:
 * - These assertions keep route-level trust, loading, and degraded-state promises from quietly regressing.
 * - If a design or product contract changes, the corresponding test should move with it instead of letting the route drift.
 *
 * Main declarations:
 * - This file is mostly internal implementation detail.
 *
 * Source-of-truth notes:
 * - Route behavior is defined jointly by `docs/design/screens-and-nav.md`, `docs/design/ux-principles.md`, and the relevant feature docs.
 * - Tests should verify real user-facing promises such as deep links, scoped callouts, loading grammar, and repair entry points.
 */

import { describe, expect, it } from 'vitest'
import type { AiSettings, RetentionPreview } from '../../lib/types'
import {
  appendAiProviderDraft,
  browserIcon,
  browserIconClass,
  buildRetentionSelection,
  cloneAiSettings,
  mergeAiProviderSecretState,
  patchAiProviderDraft,
  removeAiProviderDraft,
  selectAiProviderDraft,
  serializeAiSettings,
} from './helpers'

const aiSettingsFixture: AiSettings = {
  enabled: true,
  assistantEnabled: true,
  semanticIndexEnabled: true,
  mcpEnabled: false,
  skillEnabled: false,
  autoIndexAfterBackup: true,
  jobQueuePaused: false,
  jobQueueConcurrency: 2,
  enrichmentEnabled: true,
  enrichmentPlugins: [{ pluginId: 'title-normalization', enabled: true }],
  llmProviderId: 'llm-1',
  embeddingProviderId: 'embed-1',
  retrievalTopK: 8,
  assistantSystemPrompt: 'Use the evidence first.',
  llmProviders: [
    {
      id: 'llm-1',
      name: 'LLM',
      enabled: true,
      baseUrl: 'http://localhost:11434/v1',
      apiKeySaved: false,
      defaultModel: 'gpt-test',
      modelCatalog: ['gpt-test', 'gpt-fallback'],
      requestFormat: 'openai',
      purpose: 'llm',
    },
  ],
  embeddingProviders: [
    {
      id: 'embed-1',
      name: 'Embed',
      enabled: true,
      baseUrl: 'http://localhost:11434/v1',
      apiKeySaved: false,
      defaultModel: 'text-embedding',
      modelCatalog: ['text-embedding'],
      requestFormat: 'openai',
      purpose: 'embedding',
    },
  ],
}

const retentionPreviewFixture: RetentionPreview = {
  buckets: [
    {
      id: 'exports',
      bytes: 2048,
      itemCount: 3,
      paths: ['/tmp/exports/report.csv'],
    },
    {
      id: 'staging',
      bytes: 0,
      itemCount: 0,
      paths: ['/tmp/staging/tmp.sqlite'],
    },
    {
      id: 'snapshots',
      bytes: 4096,
      itemCount: 4,
      paths: ['/tmp/snapshots/run-1.sqlite'],
    },
  ],
  warnings: [],
}

describe('settings helpers', () => {
  it('clones AI settings without sharing model catalog arrays', () => {
    const clone = cloneAiSettings(aiSettingsFixture)

    clone.llmProviders[0].modelCatalog.push('mutated')

    expect(aiSettingsFixture.llmProviders[0].modelCatalog).toEqual([
      'gpt-test',
      'gpt-fallback',
    ])
  })

  it('serializes AI settings for dirty-check comparisons', () => {
    expect(serializeAiSettings(aiSettingsFixture)).toBe(
      JSON.stringify(aiSettingsFixture),
    )
    expect(serializeAiSettings(null)).toBeNull()
  })

  it('keeps explicit retention selections while defaulting new buckets from byte size', () => {
    expect(
      buildRetentionSelection(retentionPreviewFixture, {
        exports: false,
        snapshots: true,
      }),
    ).toEqual({
      exports: false,
      staging: false,
      snapshots: true,
    })
  })

  it('maps browser profile ids to stable glyphs and CSS classes', () => {
    expect(browserIcon('chrome:Default')).toBe('C')
    expect(browserIcon('edge:Default')).toBe('E')
    expect(browserIconClass('safari:default')).toBe('browser-icon safari')
  })

  it('updates saved-secret state across both provider lists by provider id', () => {
    expect(
      mergeAiProviderSecretState(aiSettingsFixture, 'embed-1', true)
        .embeddingProviders[0].apiKeySaved,
    ).toBe(true)
    expect(
      mergeAiProviderSecretState(aiSettingsFixture, 'llm-1', true)
        .llmProviders[0].apiKeySaved,
    ).toBe(true)
  })

  it('adds, patches, selects, and removes provider drafts in a type-safe way', () => {
    const newProvider = {
      id: 'llm-2',
      name: 'Second LLM',
      purpose: 'llm' as const,
      requestFormat: 'ollama' as const,
      enabled: true,
      baseUrl: 'http://localhost:11434',
      apiKeySaved: false,
      defaultModel: 'llama3.2:8b',
      modelCatalog: [],
      temperature: 0.7,
      maxTokens: 1200,
      dimensions: null,
      notes: null,
    }

    const withAdded = appendAiProviderDraft(
      aiSettingsFixture,
      'llm',
      newProvider,
    )
    expect(withAdded.llmProviders).toHaveLength(2)

    const withPatched = patchAiProviderDraft(withAdded, 'llm', 'llm-2', {
      defaultModel: 'patched-model',
    })
    expect(withPatched.llmProviders[1].defaultModel).toBe('patched-model')

    const withSelected = selectAiProviderDraft(withPatched, 'llm', 'llm-2')
    expect(withSelected.llmProviderId).toBe('llm-2')

    const withRemoved = removeAiProviderDraft(withSelected, 'llm', 'llm-2')
    expect(withRemoved.llmProviders).toHaveLength(1)
    expect(withRemoved.llmProviderId).toBeNull()
  })
})
