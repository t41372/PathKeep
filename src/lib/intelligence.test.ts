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
  ai: {
    enabled: true,
    assistantEnabled: true,
    semanticIndexEnabled: true,
    mcpEnabled: false,
    skillEnabled: false,
    autoIndexAfterBackup: false,
    jobQueuePaused: false,
    jobQueueConcurrency: 1,
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
      warning: null,
    }
    expect(aiStatusMeta(readyStatus, t)).toEqual({
      label: 'Semantic index ready',
      tone: 'success',
      description: 'Indexed 24 records and ready for evidence-first retrieval.',
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
  })
})
