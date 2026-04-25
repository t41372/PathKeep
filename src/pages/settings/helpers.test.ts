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
import { createNamespaceTranslator } from '../../lib/i18n'
import type { IntelligenceLocalHostPreview } from '../../lib/core-intelligence/types'
import type {
  AiIntegrationPreview,
  AiSettings,
  RetentionPreview,
} from '../../lib/types'
import {
  appendAiProviderDraft,
  buildRetentionSelection,
  cloneAiSettings,
  localizeAiIntegrationPreview,
  localizeIntelligenceLocalHostPreview,
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

const aiIntegrationPreviewFixture: AiIntegrationPreview = {
  mcpCommand: '/Applications/PathKeep.app --worker mcp-server',
  consentSummary:
    'External AI integrations stay local-first and explicit. PathKeep only exposes localhost MCP tools after you turn on AI + MCP in Settings, and the current app session must stay unlocked.',
  manualSteps: [
    'Enable MCP or Skill integration in Settings first. Both are off by default.',
    'Store the database key in the native keyring if the archive is encrypted, so background and MCP lookups can unlock the archive.',
  ],
  capabilityNotes: [
    'MCP server toggle is currently disabled in saved Settings.',
    'Skill integration toggle is currently disabled in saved Settings.',
    'No embedding provider is selected right now, so MCP and external assistants fall back to lexical recall only. They still respect archive visibility and App Lock.',
  ],
  scopeBoundary: [
    'Queries only see currently visible archive facts. Reverted visits stay hidden even if an old embedding row still exists.',
  ],
  auditTrace: [
    'Every MCP request is recorded as a dedicated `mcp_query` run in the unified archive ledger.',
    'Derived AI state lives beside the archive at /tmp/pathkeep and can be cleared/rebuilt without touching canonical visits.',
  ],
  generatedFiles: [
    {
      relativePath: 'integrations/pathkeep-mcp.json',
      absolutePath: '/tmp/pathkeep/integrations/pathkeep-mcp.json',
      purpose: 'Local MCP client configuration snippet for PathKeep.',
      contents: '{\n  "mcpServers": {}\n}',
    },
  ],
  warnings: [
    'MCP and skill integration are both disabled in Settings right now.',
  ],
}

const localHostPreviewFixture: IntelligenceLocalHostPreview = {
  artifactRoot: '/tmp/pathkeep/browser-snippet-v1',
  entryFilePath: '/tmp/pathkeep/browser-snippet-v1/index.html',
  generatedFiles: [
    {
      relativePath:
        'integrations/core-intelligence/browser-snippet-v1/index.html',
      absolutePath: '/tmp/pathkeep/browser-snippet-v1/index.html',
      purpose:
        'Core Intelligence snippet that can be opened directly in a local browser.',
      contents: '<!doctype html>',
    },
  ],
  bundle: {
    bundleVersion: 'pathkeep.core-intelligence.local-host.v1',
    hostId: 'browser-snippet-v1',
    generatedAt: '2026-04-18T10:15:00Z',
    locale: 'en',
    dateRange: { start: '2026-04-01', end: '2026-04-18' },
    profileId: 'chrome:Default',
    embedCards: [],
    widgetSnapshot: {
      generatedAt: '2026-04-18T10:15:00Z',
      dateRange: { start: '2026-04-01', end: '2026-04-18' },
      digestSummary: {
        dateRange: { start: '2026-04-01', end: '2026-04-18' },
        totalVisits: { value: 0, trend: 'flat' },
        totalSearches: { value: 0, trend: 'flat' },
        newDomains: { value: 0, trend: 'flat' },
        deepReadPages: { value: 0, trend: 'flat' },
        refindPages: { value: 0, trend: 'flat' },
      },
      highlights: [],
      notes: [],
    },
    publicSnapshot: {
      generatedAt: '2026-04-18T10:15:00Z',
      dateRange: { start: '2026-04-01', end: '2026-04-18' },
      digestSummary: {
        dateRange: { start: '2026-04-01', end: '2026-04-18' },
        totalVisits: { value: 0, trend: 'flat' },
        totalSearches: { value: 0, trend: 'flat' },
        newDomains: { value: 0, trend: 'flat' },
        deepReadPages: { value: 0, trend: 'flat' },
        refindPages: { value: 0, trend: 'flat' },
      },
      topDomains: [],
      searchEngines: [],
      discoveryTrend: { availableYears: [], points: [] },
      notes: [],
    },
    trustedOnlyCardIds: [],
    trustedOnlyCardCount: 0,
    boundaryNotes: [],
  },
  boundaryNotes: [
    'This local host only uses deterministic Core Intelligence read models.',
  ],
  manualSteps: [
    'Review index.html and bundle.json before handing this folder to another trusted local tool.',
    'Open index.html from this folder inside a trusted local browser surface.',
  ],
  warnings: [
    'This local snippet includes trusted-only cards and should not be treated like a public export.',
  ],
  installedHost: null,
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

  it('localizes AI integration preview review copy through settings-owned strings', () => {
    const settingsT = createNamespaceTranslator('zh-TW', 'settings')
    const localized = localizeAiIntegrationPreview(
      aiIntegrationPreviewFixture,
      settingsT,
    )

    expect(localized.consentSummary).toBe(
      settingsT('aiIntegrationConsentSummary'),
    )
    expect(localized.manualSteps).toContain(
      settingsT('aiIntegrationManualEnable'),
    )
    expect(localized.auditTrace).toContain(
      settingsT('aiIntegrationAuditDerivedPath', { path: '/tmp/pathkeep' }),
    )
    expect(localized.generatedFiles[0].purpose).toBe(
      settingsT('aiIntegrationGeneratedFileMcpPurpose'),
    )
    expect(localized.warnings).toContain(
      settingsT('aiIntegrationWarningDisabled'),
    )
  })

  it('localizes trusted local-host preview fallback strings before settings renders them', () => {
    const settingsT = createNamespaceTranslator('zh-TW', 'settings')
    const localized = localizeIntelligenceLocalHostPreview(
      localHostPreviewFixture,
      settingsT,
    )

    expect(localized.boundaryNotes).toContain(
      settingsT('externalOutputsLocalHostBoundaryDeterministic'),
    )
    expect(localized.manualSteps).toContain(
      settingsT('externalOutputsLocalHostManualReview'),
    )
    expect(localized.generatedFiles[0].purpose).toBe(
      settingsT('externalOutputsLocalHostPurposeEntry'),
    )
    expect(localized.warnings).toContain(
      settingsT('externalOutputsLocalHostWarningTrusted'),
    )
  })
})
