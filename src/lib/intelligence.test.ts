/**
 * This test file protects the front-end helper and contract logic in Intelligence.
 *
 * Why this file exists:
 * - Pure helpers are where we keep UI policy testable without booting the whole shell.
 * - When these tests fail, they usually point at a contract drift that would otherwise show up as subtle route regressions.
 *
 * Main declarations:
 * - This file is mostly internal implementation detail.
 *
 * Source-of-truth notes:
 * - Helper behavior should stay aligned with the same design, feature, and architecture docs that guide the UI surfaces consuming it.
 * - Prefer focused behavioral assertions over snapshotting implementation detail.
 */

import { describe, expect, test } from 'vitest'
import type { AiAssistantResponse, AiIndexStatus, AppConfig } from './types'
import { createNamespaceTranslator } from './i18n/catalog'
import {
  aiStatusMeta,
  assistantHref,
  assistantResponseMeta,
  dedupeEvidence,
  evidenceHref,
  scoreBand,
  selectedAiProvider,
} from './intelligence'

const t = createNamespaceTranslator('en', 'intelligence')

const config: AppConfig = {
  initialized: true,
  archiveMode: 'Plaintext',
  preferredLanguage: 'en',
  dueAfterHours: 72,
  scheduleCheckIntervalHours: 6,
  checkpointDays: 90,
  captureFavicons: true,
  selectedProfileIds: ['chrome:Default'],
  gitEnabled: true,
  rememberDatabaseKeyInKeyring: false,
  appAutostart: false,
  appLock: {
    enabled: false,
    idleTimeoutMinutes: 5,
    biometricEnabled: false,
    passcodeEnabled: true,
    passcodeConfigured: false,
    recoveryHint: null,
  },
  analytics: {
    enabled: false,
    consentGrantedAt: null,
  },
  remoteBackup: {
    enabled: false,
    bucket: '',
    region: 'us-east-1',
    endpoint: null,
    prefix: 'pathkeep',
    pathStyle: true,
    uploadAfterBackup: false,
    credentialsSaved: false,
    lastUploadedAt: null,
    lastUploadedObjectKey: null,
    lastError: null,
  },
  enrichment: {
    plugins: [
      {
        id: 'readable-content-refetch',
        enabled: true,
        version: 'diagnostic',
      },
    ],
  },
  deterministic: {
    modules: [
      { id: 'query-groups', enabled: true, version: 'diagnostic' },
      { id: 'threads', enabled: true, version: 'diagnostic' },
      { id: 'reference-pages', enabled: true, version: 'diagnostic' },
      { id: 'source-effectiveness', enabled: true, version: 'diagnostic' },
      { id: 'template-summaries', enabled: true, version: 'diagnostic' },
    ],
  },
  ai: {
    enabled: true,
    assistantEnabled: true,
    semanticIndexEnabled: true,
    mcpEnabled: false,
    skillEnabled: false,
    autoIndexAfterBackup: false,
    jobQueuePaused: false,
    jobQueueConcurrency: 1,
    enrichmentEnabled: true,
    enrichmentPlugins: [
      { pluginId: 'title-normalization', enabled: true },
      { pluginId: 'readable-content-refetch', enabled: true },
    ],
    llmProviderId: 'llm-primary',
    embeddingProviderId: 'embed-primary',
    retrievalTopK: 8,
    assistantSystemPrompt: 'Use evidence only.',
    llmProviders: [
      {
        id: 'llm-primary',
        name: 'LLM',
        purpose: 'llm',
        requestFormat: 'openai',
        enabled: true,
        apiKeySaved: true,
        defaultModel: 'gpt-4.1-mini',
        modelCatalog: ['gpt-4.1-mini'],
      },
    ],
    embeddingProviders: [
      {
        id: 'embed-primary',
        name: 'Embed',
        purpose: 'embedding',
        requestFormat: 'openai',
        enabled: true,
        apiKeySaved: true,
        defaultModel: 'text-embedding-3-large',
        modelCatalog: ['text-embedding-3-large'],
      },
    ],
  },
}

describe('intelligence helpers', () => {
  test('resolves selected providers by purpose', () => {
    expect(selectedAiProvider(config.ai, 'embedding')?.id).toBe('embed-primary')
    expect(selectedAiProvider(config.ai, 'llm')?.id).toBe('llm-primary')
  })

  test('returns null when the selected provider id is missing', () => {
    expect(
      selectedAiProvider(
        {
          ...config.ai,
          llmProviderId: 'missing-provider',
        },
        'llm',
      ),
    ).toBeNull()
  })

  test('maps index states into user-facing metadata', () => {
    const readyStatus: AiIndexStatus = {
      enabled: true,
      assistantEnabled: true,
      mcpEnabled: false,
      skillEnabled: false,
      state: 'ready',
      ready: true,
      indexedItems: 24,
      lastIndexedAt: null,
      llmProviderId: 'llm-primary',
      embeddingProviderId: 'embed-primary',
      queuePaused: false,
      queueConcurrency: 1,
      queuedJobs: 0,
      runningJobs: 0,
      failedJobs: 0,
      recentJobs: [],
      semanticSidecarBytes: 0,
      semanticMirrorBytes: 0,
      estimatedEmbeddingTokens: 0,
      warning: null,
    }
    expect(aiStatusMeta(readyStatus, t)).toEqual({
      label: t('statusReadyLabel'),
      tone: 'success',
      description: t('statusReadyDescription', { count: 24 }),
    })

    const degraded = aiStatusMeta(
      {
        ...readyStatus,
        state: 'degraded',
        warning: 'store a key',
      },
      t,
    )
    expect(degraded.tone).toBe('blocked')
    expect(degraded.description).toBe('store a key')
    expect(
      aiStatusMeta({ ...readyStatus, state: 'degraded', warning: null }, t),
    ).toEqual({
      label: t('statusDegradedLabel'),
      tone: 'blocked',
      description: t('statusDegradedDescription'),
    })

    expect(
      aiStatusMeta({ ...readyStatus, state: 'rebuilding', warning: null }, t),
    ).toEqual({
      label: t('statusRebuildingLabel'),
      tone: 'warning',
      description: t('statusRebuildingDescription'),
    })
    expect(
      aiStatusMeta({ ...readyStatus, state: 'queued', warning: null }, t),
    ).toEqual({
      label: t('statusQueuedLabel'),
      tone: 'warning',
      description: t('statusQueuedDescription'),
    })
    expect(
      aiStatusMeta({ ...readyStatus, state: 'paused', warning: null }, t),
    ).toEqual({
      label: t('statusPausedLabel'),
      tone: 'warning',
      description: t('statusPausedDescription'),
    })
    expect(
      aiStatusMeta({ ...readyStatus, state: 'failed', warning: null }, t),
    ).toEqual({
      label: t('statusFailedLabel'),
      tone: 'blocked',
      description: t('statusFailedDescription'),
    })
    expect(
      aiStatusMeta({ ...readyStatus, state: 'stale', warning: null }, t),
    ).toEqual({
      label: t('statusStaleLabel'),
      tone: 'warning',
      description: t('statusStaleDescription'),
    })
    expect(
      aiStatusMeta({ ...readyStatus, state: 'blocked', warning: null }, t),
    ).toEqual({
      label: t('statusBlockedLabel'),
      tone: 'blocked',
      description: t('statusBlockedDescription'),
    })
    expect(
      aiStatusMeta({ ...readyStatus, state: 'disabled', warning: null }, t),
    ).toEqual({
      label: t('statusDisabledLabel'),
      tone: 'info',
      description: t('statusDisabledDescription'),
    })
    expect(
      aiStatusMeta({ ...readyStatus, state: 'empty', warning: null }, t),
    ).toEqual({
      label: t('statusEmptyLabel'),
      tone: 'info',
      description: t('statusEmptyDescription'),
    })
  })

  test('assigns evidence score bands', () => {
    expect(scoreBand(0.91, t)).toEqual({
      label: 'High confidence',
      tone: 'success',
    })
    expect(scoreBand(0.72, t)).toEqual({ label: 'Relevant', tone: 'warning' })
    expect(scoreBand(0.4, t)).toEqual({ label: 'Weak match', tone: 'info' })
    expect(scoreBand(undefined, t)).toEqual({ label: 'No score', tone: 'info' })
  })

  test('treats score thresholds as inclusive', () => {
    expect(scoreBand(0.85, t)).toEqual({
      label: 'High confidence',
      tone: 'success',
    })
    expect(scoreBand(0.65, t)).toEqual({
      label: 'Relevant',
      tone: 'warning',
    })
    expect(scoreBand(0.649, t)).toEqual({
      label: 'Weak match',
      tone: 'info',
    })
  })

  test('builds deep links for evidence and assistant prompts', () => {
    expect(
      evidenceHref({
        profileId: 'chrome:Default',
        domain: 'example.com',
        url: 'https://example.com/docs',
      }),
    ).toBe(
      '/explorer?profileId=chrome%3ADefault&domain=example.com&q=https%3A%2F%2Fexample.com%2Fdocs',
    )
    expect(assistantHref('What did I read about SQLite?')).toBe(
      '/assistant?question=What+did+I+read+about+SQLite%3F',
    )
    expect(
      evidenceHref({
        title: 'SQLite history',
      }),
    ).toBe('/explorer?q=SQLite+history')
    expect(evidenceHref({})).toBe('/explorer')
  })

  test('dedupes evidence by history id and url', () => {
    expect(
      dedupeEvidence([
        {
          historyId: 1,
          profileId: 'chrome:Default',
          url: 'https://example.com',
          title: 'Example',
          visitedAt: '2026-04-07T00:00:00Z',
          score: 0.9,
        },
        {
          historyId: 1,
          profileId: 'chrome:Default',
          url: 'https://example.com',
          title: 'Example',
          visitedAt: '2026-04-07T00:00:00Z',
          score: 0.7,
        },
      ]),
    ).toHaveLength(1)
  })

  test('keeps evidence rows when either the history id or url changes', () => {
    expect(
      dedupeEvidence([
        {
          historyId: 1,
          profileId: 'chrome:Default',
          url: 'https://example.com',
          title: 'Example',
          visitedAt: '2026-04-07T00:00:00Z',
          score: 0.9,
        },
        {
          historyId: 2,
          profileId: 'chrome:Default',
          url: 'https://example.com',
          title: 'Example 2',
          visitedAt: '2026-04-07T01:00:00Z',
          score: 0.8,
        },
        {
          historyId: 1,
          profileId: 'chrome:Default',
          url: 'https://example.com/docs',
          title: 'Docs',
          visitedAt: '2026-04-07T02:00:00Z',
          score: 0.7,
        },
      ]),
    ).toHaveLength(3)
  })

  test('maps assistant responses into status chips', () => {
    const response: AiAssistantResponse = {
      state: 'queued',
      answer: '',
      jobId: 7,
      runId: null,
      providerId: 'llm-primary',
      embeddingProviderId: 'embed-primary',
      citations: [],
      notes: [],
    }
    expect(assistantResponseMeta(response, t)).toEqual({
      label: 'Queued',
      tone: 'warning',
    })

    expect(
      assistantResponseMeta({ ...response, state: 'completed' }, t),
    ).toEqual({
      label: t('answerReady'),
      tone: 'success',
    })
    expect(
      assistantResponseMeta({ ...response, state: 'insufficient-evidence' }, t),
    ).toEqual({
      label: t('evidenceMissing'),
      tone: 'blocked',
    })
    expect(assistantResponseMeta({ ...response, state: 'failed' }, t)).toEqual({
      label: t('assistantFailed'),
      tone: 'blocked',
    })
    expect(
      assistantResponseMeta({ ...response, state: 'cancelled' }, t),
    ).toEqual({
      label: t('cancelled'),
      tone: 'info',
    })
    expect(assistantResponseMeta({ ...response, state: 'running' }, t)).toEqual(
      {
        label: t('inProgress'),
        tone: 'info',
      },
    )
  })
})
